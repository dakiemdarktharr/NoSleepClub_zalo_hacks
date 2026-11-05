"""Prepare and run the Rust LifeMate candidate on an operator-owned Linux GPU.

Run only in an explicitly selected remote notebook/host. This helper never
reads .env.local, calls a hosted LLM, publishes a notebook, or submits to Zalo.
Linux/CUDA execution is unverified until the remote run produces receipts.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

REPO = Path(__file__).resolve().parents[1]
DATA_URL = "https://dl-challenge.zalo.ai/2026/lifemate_argfzdgert/train+public_test.zip"
DATA_BYTES = 876430231
DATA_SHA = "57d371518996ec70d3af86402b7eba61208093c82955b9807a7ab39604b47320"


def call(arguments: list[str], *, env=None, cwd=None, capture=False):
    return subprocess.run(arguments, env=env, cwd=cwd, check=True, text=True,
                          capture_output=capture)


def checksum(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024): digest.update(block)
    return digest.hexdigest()


def download(url: str, path: Path, expected_bytes: int, expected_sha: str) -> None:
    if path.is_file():
        if path.stat().st_size == expected_bytes and checksum(path) == expected_sha: return
        raise ValueError("existing downloaded asset does not match its pin")
    temporary = path.with_suffix(path.suffix + ".partial")
    with urllib.request.urlopen(url, timeout=90) as source, temporary.open("xb") as target:
        size = 0
        while block := source.read(1024 * 1024):
            size += len(block)
            if size > expected_bytes: raise ValueError("download exceeds pinned size")
            target.write(block)
    if size != expected_bytes or checksum(temporary) != expected_sha:
        raise ValueError("download integrity check failed")
    temporary.rename(path)


def unpack_official(archive: Path, destination: Path) -> Path:
    """Transport/setup only: restore the official archive, never inspect state."""
    if destination.exists():
        raise ValueError("choose a new dataset extraction directory")
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        if len(members) > 40000 or sum(m.file_size for m in members) > 2_000_000_000:
            raise ValueError("archive bounds")
        for member in members:
            path = PurePosixPath(member.filename)
            if path.is_absolute() or not path.parts or any(p in {"..", "."} for p in path.parts) or "\\" in member.filename or ":" in member.filename or stat.S_ISLNK(member.external_attr >> 16):
                raise ValueError("unsafe archive entry")
        destination.mkdir(parents=True)
        source.extractall(destination)
    candidates = list(destination.glob("*/public_test/public_test.jsonl"))
    if (destination / "public_test/public_test.jsonl").is_file():
        candidates.append(destination / "public_test/public_test.jsonl")
    if len(candidates) != 1: raise ValueError("official data layout not found")
    return candidates[0].parent.parent


def gpu_info() -> tuple[str, str]:
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise RuntimeError("this remote helper requires Linux x86_64; no local Windows inference")
    text = call(["nvidia-smi", "--query-gpu=name,compute_cap", "--format=csv,noheader"], capture=True).stdout
    first = text.splitlines()[0]
    name, capability = [part.strip() for part in first.rsplit(",", 1)]
    architecture = capability.replace(".", "")
    if not architecture.isdigit() or not 70 <= int(architecture) <= 150:
        raise RuntimeError("unsupported CUDA architecture metadata")
    return name, architecture


def verify_ui_disabled(native_source: Path) -> dict:
    cache = (native_source / "build/CMakeCache.txt").read_text(encoding="utf-8")
    if not all(f"{name}:BOOL=OFF" in cache for name in ("LLAMA_BUILD_UI", "LLAMA_USE_PREBUILT_UI")):
        raise RuntimeError("native UI provisioning is not disabled")
    if any((native_source / relative).exists() for relative in ("tools/ui/dist/index.html", "build/tools/ui/dist/index.html")):
        raise RuntimeError("unexpected cached native UI assets")
    generated = (native_source / "build/tools/ui/ui.cpp").read_text(encoding="utf-8")
    count = len(re.findall(r"(?m)^\s*static\s+const\s+unsigned\s+char\s+asset_\d+\[", generated))
    if count:
        raise RuntimeError("native build still contains unpinned UI asset arrays")
    return {"embedded_asset_arrays": count, "hf_ui_disabled": True, "npm_ui_disabled": True}


def prepare(workspace: Path, install_deps: bool, build_jobs: int = 2) -> None:
    if not 1 <= build_jobs <= 8:
        raise ValueError("native build jobs must be between 1 and 8")
    name, architecture = gpu_info()
    workspace.mkdir(parents=True, exist_ok=True)
    ready = workspace / "ready.json"
    if ready.exists(): raise ValueError("workspace already prepared; use its run command")
    if install_deps:
        if os.geteuid() != 0: raise RuntimeError("install build dependencies yourself or use a disposable root notebook VM")
        call(["apt-get", "update", "-qq"])
        call(["apt-get", "install", "-y", "--no-install-recommends", "ca-certificates", "git", "cmake", "ninja-build", "build-essential", "pkg-config", "libclang-dev", "libgomp1", "curl"])
    for executable in ["git", "cmake", "ninja", "nvcc"]:
        if not shutil.which(executable): raise RuntimeError(f"remote GPU build image is missing {executable}")
    env = os.environ.copy()
    env.update(CARGO_HOME=str(workspace / "cargo"), RUSTUP_HOME=str(workspace / "rustup"), CARGO_BUILD_JOBS="2")
    env["PATH"] = str(workspace / "cargo/bin") + os.pathsep + env.get("PATH", "")
    if not (workspace / "cargo/bin/rustup").is_file():
        installer = workspace / "rustup-init.sh"
        with urllib.request.urlopen("https://sh.rustup.rs", timeout=60) as source:
            data = source.read(2_000_001)
        if len(data) > 2_000_000: raise ValueError("rustup bootstrap exceeds bound")
        installer.write_bytes(data)
        call(["sh", str(installer), "-y", "--no-modify-path", "--profile", "minimal", "--default-toolchain", "1.99.1"], env=env)
    call([str(workspace / "cargo/bin/rustup"), "toolchain", "install", "1.99.1", "--profile", "minimal", "--component", "rustfmt", "--component", "clippy"], env=env)
    manifest = json.loads((REPO / "assets/manifests/lifemate-offline.v1.json").read_text())
    native = {}
    for item in manifest["native_sources"]:
        directory = workspace / item["name"]
        if not directory.exists():
            call(["git", "init", str(directory)])
            call(["git", "-C", str(directory), "remote", "add", "origin", item["repository"]])
            call(["git", "-C", str(directory), "fetch", "--depth", "1", "origin", item["revision"]])
            call(["git", "-C", str(directory), "checkout", "--detach", item["revision"]])
        if call(["git", "-C", str(directory), "rev-parse", "HEAD"], capture=True).stdout.strip() != item["revision"]:
            raise ValueError("native source revision mismatch")
        options = ["-DGGML_NATIVE=OFF", "-DGGML_CUDA=ON", f"-DCMAKE_CUDA_ARCHITECTURES={architecture}", "-DBUILD_SHARED_LIBS=OFF", "-DGGML_BACKEND_DL=OFF"]
        if item["name"] == "llama.cpp":
            target = "llama-server"
            # Disabling HF provisioning alone still embeds an old build/dist.
            # Preserve any earlier unpinned download outside the build tree.
            if (directory / "tools/ui/dist/index.html").exists():
                raise RuntimeError("unexpected source UI assets; use a clean pinned checkout")
            cached_ui = directory / "build/tools/ui/dist"
            if cached_ui.exists():
                if cached_ui.is_symlink() or not cached_ui.resolve().is_relative_to(workspace.resolve()):
                    raise ValueError("cached UI path escapes the owned workspace")
                quarantine = Path(tempfile.mkdtemp(prefix="nora-unused-ui-", dir=workspace))
                cached_ui.rename(quarantine / "dist")
            options += ["-DLLAMA_CURL=OFF", "-DLLAMA_OPENSSL=OFF", "-DLLAMA_BUILD_TESTS=OFF", "-DLLAMA_BUILD_SERVER=ON", "-DLLAMA_BUILD_APP=OFF", "-DLLAMA_BUILD_EXAMPLES=OFF", "-DLLAMA_SUBPROCESS=OFF", "-DLLAMA_BUILD_UI=OFF", "-DLLAMA_USE_PREBUILT_UI=OFF"]
        else:
            target = "whisper-server"
            options += ["-DWHISPER_BUILD_TESTS=OFF", "-DWHISPER_BUILD_SERVER=ON"]
        call(["cmake", "-S", str(directory), "-B", str(directory / "build"), "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release", *options], env=env)
        call(["cmake", "--build", str(directory / "build"), "--target", target, "--parallel", str(build_jobs)], env=env)
        if item["name"] == "llama.cpp":
            verify_ui_disabled(directory)
        native[target] = str(directory / "build/bin" / target)
    call([str(workspace / "cargo/bin/cargo"), "build", "--locked", "--release", "-p", "nora-lifemate", "--features", "whisper"], env=env, cwd=REPO)
    models = workspace / "models"
    models.mkdir(exist_ok=True)
    for item in manifest["models"]:
        repository = item["repository"].removeprefix("https://huggingface.co/")
        url = f"https://huggingface.co/{repository}/resolve/{item['revision']}/{item['filename']}"
        download(url, models / item["filename"], item["bytes"], item["sha256"])
    archive = workspace / "official.zip"
    download(DATA_URL, archive, DATA_BYTES, DATA_SHA)
    root = unpack_official(archive, workspace / "official")
    configuration = {"root":str(root), "gpu":name, "cuda_architecture":architecture, "native":native,
                     "models":{m["role"]:str(models / m["filename"]) for m in manifest["models"]},
                     "executable":str(REPO / "target/release/nora-lifemate"), "source_manifest_sha256":checksum(REPO / "assets/manifests/lifemate-offline.v1.json"),
                     "executable_sha256":checksum(REPO / "target/release/nora-lifemate"),
                     "native_sha256":{name:checksum(Path(path)) for name,path in native.items()},
                     "native_ui_hardening":verify_ui_disabled(workspace / "llama.cpp"),
                     "a30_benchmark_certified":False,"hosted_llm_api_used":False}
    ready.write_text(json.dumps(configuration,indent=2)+"\n")
    print("Prepared remote candidate. No inference has run and no score is claimed.")


def validate(configuration: dict, directory: Path) -> dict:
    prediction = directory / "submission.jsonl"
    expected = set()
    with (Path(configuration["root"]) / "public_test/public_test.jsonl").open() as source:
        for line in source:
            if line.strip(): expected.add(json.loads(line)["id"])
    seen = set()
    with prediction.open() as source:
        for line in source:
            row = json.loads(line)
            if set(row) != {"id", "tool_calls", "state_hash"} or row["id"] in seen or row["id"] not in expected:
                raise ValueError("invalid prediction coverage/envelope")
            seen.add(row["id"])
            if len(row["state_hash"]) != 64 or any(c not in "0123456789abcdefABCDEF" for c in row["state_hash"]) or not isinstance(row["tool_calls"],list) or len(row["tool_calls"])>32:
                raise ValueError("invalid state hash or calls")
    if seen != expected: raise ValueError("missing public predictions; do not upload a partial file")
    call([sys.executable, str(REPO / "scripts/verify_lifemate_replay.py"), "--root", configuration["root"], "--split", "public_test", "--predictions", str(prediction), "--report", str(directory / "replay.json")])
    metrics = [json.loads(line) for line in (directory / "samples.jsonl").read_text().splitlines() if line.strip()]
    if len(metrics) != len(seen) or {m["id"] for m in metrics} != seen: raise ValueError("metric coverage mismatch")
    failed = sum(m["status"] != "complete" or m["elapsed_ms"] > 30000 for m in metrics)
    report = {"scope":"public submission preflight; NOT an accuracy score", "public_samples":len(seen),"complete_coverage":True,
              "official_sdk_replay_passed":True,"failed_or_timeout_samples":failed,"runtime_gate_passed":failed==0,
              "submission_sha256":checksum(prediction),"nora_accuracy_percent":None,"submission_uploaded":False}
    (directory / "preflight.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report))
    return report


def run(workspace: Path, output: Path) -> None:
    gpu_info()
    configuration = json.loads((workspace / "ready.json").read_text())
    if configuration.get("native_ui_hardening") != verify_ui_disabled(workspace / "llama.cpp"):
        raise RuntimeError("native UI verification receipt is missing or changed")
    for name, path in configuration["native"].items():
        if checksum(Path(path)) != configuration["native_sha256"][name]:
            raise RuntimeError("native executable differs from its prepared receipt")
    output.mkdir(parents=True,exist_ok=False)
    executable=configuration["executable"]
    arguments=[executable,"--root",configuration["root"],"--split","public_test","--python",sys.executable,
               "--bridge",str(REPO / "crates/nora-lifemate/python/bridge.py"),"--server",configuration["native"]["llama-server"],
               "--model",configuration["models"]["llm"],"--device","CUDA0","--asr-server",configuration["native"]["whisper-server"],
               "--asr-model",configuration["models"]["asr"],"--context","16384","--deadline-ms","30000","--partition","all",
               "--output",str(output/"submission.jsonl"),"--metrics",str(output/"samples.jsonl"),"--model-log",str(output/"model.log")]
    allowed=["PATH","LD_LIBRARY_PATH","LANG","LC_ALL","CUDA_VISIBLE_DEVICES","NVIDIA_VISIBLE_DEVICES","NVIDIA_DRIVER_CAPABILITIES"]
    environment={key:os.environ[key] for key in allowed if key in os.environ}
    cache=workspace/"cuda-cache";cache.mkdir(exist_ok=True)
    environment["CUDA_CACHE_PATH"]=str(cache)
    with (output/"supervisor.log").open("x") as errors:
        process=subprocess.Popen(arguments,env=environment,stderr=subprocess.PIPE,text=True)
        try:
            assert process.stderr is not None
            for line in process.stderr:
                errors.write(line);errors.flush()
                if "models ready" in line or "NORA LifeMate " in line:
                    print(line.rstrip(),flush=True)
            if process.wait()!=0: raise RuntimeError("remote supervisor failed; inspect the private supervisor.log")
        finally:
            if process.poll() is None:
                process.terminate()
                try: process.wait(timeout=5)
                except subprocess.TimeoutExpired: process.kill();process.wait()
    validate(configuration,output)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action",choices=["prepare","run","validate"])
    parser.add_argument("--workspace",type=Path,required=True)
    parser.add_argument("--output",type=Path)
    parser.add_argument("--install-build-deps",action="store_true")
    parser.add_argument("--build-jobs",type=int,default=2)
    args=parser.parse_args()
    workspace=args.workspace.resolve()
    if args.action=="prepare": prepare(workspace,args.install_build_deps,args.build_jobs)
    elif args.action=="run":
        if args.output is None: parser.error("--output is required")
        run(workspace,args.output.resolve())
    else:
        if args.output is None: parser.error("--output is required")
        validate(json.loads((workspace/"ready.json").read_text()),args.output.resolve())


if __name__=="__main__": main()
