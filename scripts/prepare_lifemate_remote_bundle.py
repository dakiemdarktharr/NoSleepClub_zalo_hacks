"""Create a private, bounded source bundle and Colab notebook; no model runs.

Only explicit repository source paths enter the zip. Dataset, weights, .env,
Git metadata, user folders and development artifacts are never bundled.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import json
from pathlib import Path
import stat
import zipfile

ROOT=Path(__file__).resolve().parents[1]


def notebook(bundle_sha: str, embedded_bundle: bytes | None = None) -> dict:
    def cell(kind,text):
        value={"cell_type":kind,"metadata":{},"source":text.splitlines(keepends=True)}
        if kind=="code": value.update(execution_count=None,outputs=[])
        return value
    transport = '''uploaded = files.upload()
bundle = Path("NORA-LifeMate-source.zip")
assert bundle.name in uploaded, "Upload NORA-LifeMate-source.zip"'''
    if embedded_bundle is not None:
        encoded = base64.b64encode(embedded_bundle).decode("ascii")
        transport = ('import base64\nbundle = Path("NORA-LifeMate-source.zip")\n'
                     f'bundle.write_bytes(base64.b64decode("{encoded}", validate=True))')
    cells=[
        cell("markdown","""# NORA LifeMate: tạo Public Submission trên GPU ngoài laptop

Candidate chưa được chạy trên Linux/GPU; notebook này không chứng nhận 100%.
Zalo Public Submissions chỉ nhận kết quả JSONL, không chạy code đã upload.
Chạy notebook **private**, dùng runtime GPU của tài khoản bạn; không mount Drive,
không thêm API key, không bật chia sẻ công khai và không mua thêm compute tự động.
Colab không bảo đảm GPU/quota. Đây chưa phải benchmark A30/64GB của BTC.

Chuẩn bị: Runtime → Change runtime type → GPU. Sau đó upload file source bundle
được cung cấp cùng notebook. Chỉ code hiện tại của NORA được upload, không có
credentials, dữ liệu thi hay model weights trong bundle. Bước prepare sẽ tải
dataset chính thức và hai model đã pin vào VM riêng của tài khoản bạn.
"""),
        cell("code",f'''from google.colab import files
from pathlib import Path, PurePosixPath
import hashlib, json, stat, zipfile, subprocess, sys
{transport}
assert hashlib.sha256(bundle.read_bytes()).hexdigest() == "{bundle_sha}", "Source bundle checksum mismatch"
repo = Path("/content/nora-source")
assert not repo.exists(), "Use a fresh runtime/source directory"
with zipfile.ZipFile(bundle) as archive:
    members = archive.infolist()
    assert len(members) < 2000 and sum(m.file_size for m in members) < 20000000
    for m in members:
        p=PurePosixPath(m.filename)
        assert not p.is_absolute() and ".." not in p.parts and "\\\\" not in m.filename and ":" not in m.filename and not stat.S_ISLNK(m.external_attr >> 16)
    repo.mkdir()
    archive.extractall(repo)
manifest=json.loads((repo/"remote-source-manifest.json").read_text())
for item in manifest["files"]:
    assert hashlib.sha256((repo/item["path"]).read_bytes()).hexdigest() == item["sha256"]
print("Verified private source bundle:",len(manifest["files"]),"files; no inference run")
'''),
        cell("code",'''import os, signal

def run_logged(command, env=None):
    # The disposable Linux VM owns this new process group and its descendants.
    task = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, bufsize=1, start_new_session=True)
    try:
        for line in task.stdout:
            print(line.rstrip(), flush=True)
        code = task.wait()
        if code:
            raise subprocess.CalledProcessError(code, command)
    finally:
        try:
            os.killpg(task.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            task.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        try:
            os.killpg(task.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        task.wait()

workspace = Path("/content/nora-gpu-workspace")
helper = repo / "scripts/lifemate_remote.py"
print(subprocess.check_output(["nvidia-smi"], text=True))
'''),
        cell("markdown","""## Prepare trên VM GPU

Bước này cài build dependencies trong VM tạm, build Rust/native servers từ SHA
đã pin, tải model và devkit theo size/SHA256. Không gọi LLM API, không đọc app
state để làm reasoning, không sinh nhãn public test. Cần Internet trong bước
chuẩn bị; inference của NORA chỉ gọi các tiến trình model qua loopback.
Notebook không chứng minh OS network isolation; Docker/A30 vẫn là gate riêng.
"""),
        cell("code",'''run_logged([sys.executable,str(helper),"prepare","--workspace",str(workspace),"--install-build-deps"])
print((workspace/"ready.json").read_text())
'''),
        cell("markdown","""## Chạy đủ public test

Giữ đúng 30 giây/mẫu, context 16.384, SDK nguyên trạng. Không dùng gold
transcript/labels, API model hoặc lookup đáp án public. Logs riêng nằm trong VM;
không công khai notebook outputs. Nếu runtime bị ngắt, giữ artifacts chẩn đoán;
không coi file thiếu mẫu hoặc có timeout là kết quả hoàn thiện.
"""),
        cell("code",'''configuration = json.loads((workspace / "ready.json").read_text())
assert configuration["native_ui_hardening"] == {"embedded_asset_arrays": 0, "hf_ui_disabled": True, "npm_ui_disabled": True}
smoke = workspace / "public-smoke-001"
smoke.mkdir(exist_ok=False)
command = [configuration["executable"], "--root", configuration["root"], "--split", "public_test", "--python", sys.executable, "--bridge", str(repo / "crates/nora-lifemate/python/bridge.py"), "--server", configuration["native"]["llama-server"], "--model", configuration["models"]["llm"], "--device", "CUDA0", "--asr-server", configuration["native"]["whisper-server"], "--asr-model", configuration["models"]["asr"], "--context", "16384", "--deadline-ms", "30000", "--partition", "all", "--limit", "3", "--output", str(smoke / "submission.jsonl"), "--metrics", str(smoke / "samples.jsonl"), "--model-log", str(smoke / "model.log")]
allowed = ["PATH", "LD_LIBRARY_PATH", "LANG", "LC_ALL", "CUDA_VISIBLE_DEVICES", "NVIDIA_VISIBLE_DEVICES", "NVIDIA_DRIVER_CAPABILITIES"]
environment = {key: os.environ[key] for key in allowed if key in os.environ}
cache = workspace / "cuda-cache"
cache.mkdir(exist_ok=True)
environment["CUDA_CACHE_PATH"] = str(cache)
print("Starting three-sample runtime smoke; no accuracy claim", flush=True)
run_logged(command, env=environment)
run_logged([sys.executable, str(repo / "scripts/verify_lifemate_replay.py"), "--root", configuration["root"], "--split", "public_test", "--predictions", str(smoke / "submission.jsonl"), "--report", str(smoke / "replay.json")])
smoke_metrics = [json.loads(line) for line in (smoke / "samples.jsonl").read_text().splitlines() if line.strip()]
print(json.dumps(smoke_metrics, indent=2), flush=True)
assert len(smoke_metrics) == 3 and all(item["status"] == "complete" and item["elapsed_ms"] <= 30000 for item in smoke_metrics), "Smoke failed or timed out; preserve logs and repair before full run"
print("Runtime smoke and official SDK replay passed; starting all 634 public samples", flush=True)
output = workspace / "public-run-001"
run_logged([sys.executable, str(helper), "run", "--workspace", str(workspace), "--output", str(output)])
report = json.loads((output / "preflight.json").read_text())
print(json.dumps(report, indent=2))
'''),
        cell("markdown","""## Lấy file về để nộp Zalo

Preflight/replay chỉ xác nhận format, coverage và execution receipts, không
biết accuracy public. Chỉ sau khi có file đầy đủ từ NORA mới nộp qua Public
Submissions; chọn SLM cho cấu hình model này. Không upload file ZIP input hoặc
sample_submission.jsonl 3 dòng của BTC. Điểm chính thức chỉ có sau khi Zalo chấm.
"""),
        cell("code",'''assert report["complete_coverage"] and report["official_sdk_replay_passed"]
assert report["runtime_gate_passed"], "Có mẫu lỗi/timeout: kiểm tra và sửa trước khi nộp"
import base64, gzip
prediction_bytes = (output / "submission.jsonl").read_bytes()
metric_bytes = (output / "samples.jsonl").read_bytes()
assert len(prediction_bytes) <= 10000000, "Export exceeds the notebook transport bound"
assert len(metric_bytes) <= 10000000, "Metrics export exceeds the notebook transport bound"
assert hashlib.sha256(prediction_bytes).hexdigest() == report["submission_sha256"]
payload = {"scope": "NORA public result export; keep this notebook private",
           "sha256": report["submission_sha256"], "preflight": report,
           "gzip_base64": base64.b64encode(gzip.compress(prediction_bytes, mtime=0)).decode("ascii"),
           "metrics_sha256": hashlib.sha256(metric_bytes).hexdigest(),
           "metrics_gzip_base64": base64.b64encode(gzip.compress(metric_bytes, mtime=0)).decode("ascii"),
           "replay": json.loads((output / "replay.json").read_text()),
           "source_snapshot_sha256": hashlib.sha256((repo / "remote-source-manifest.json").read_bytes()).hexdigest(),
           "native_ui_hardening": json.loads((workspace / "ready.json").read_text()).get("native_ui_hardening")}
print("NORA_PUBLIC_EXPORT_V1=" + json.dumps(payload, separators=(",", ":")))
''')
    ]
    if embedded_bundle is not None:
        cells[1]["metadata"]["cellView"] = "form"
        cells[0]["source"] = [line.replace("Sau đó upload file source bundle", "Source bundle được nhúng trong notebook và")
                              .replace("được cung cấp cùng notebook.", "được kiểm tra SHA256 trước khi giải nén.")
                              for line in cells[0]["source"]]
    for index,item in enumerate(cells): item["id"]=f"nora-{index:03d}"
    return {"nbformat":4,"nbformat_minor":5,"metadata":{"accelerator":"GPU","colab":{"name":"NORA-LifeMate-Public-GPU.ipynb","provenance":[],"gpuType":"T4"},"kernelspec":{"display_name":"Python 3","name":"python3"},"language_info":{"name":"python"}},"cells":cells}


def prepare(output: Path, embed_source: bool = False) -> dict:
    output.mkdir(parents=True,exist_ok=False)
    selected=[ROOT/name for name in ["Cargo.toml","Cargo.lock","rust-toolchain.toml","LICENSE","THIRD_PARTY_NOTICES.md",".cargo/config.toml",
        "assets/manifests/lifemate-offline.v1.json","scripts/lifemate_remote.py","scripts/verify_lifemate_replay.py"]]
    selected += [p for p in (ROOT/"crates").rglob("*") if p.is_file() and (p.suffix in {".rs",".toml",".py",".md"} or p.name.startswith(("LICENSE","NOTICE")))]
    rows=[]
    for path in sorted(set(selected)):
        relative=path.relative_to(ROOT)
        if path.is_symlink() or path.resolve().is_relative_to(ROOT.resolve()) is False or any(part in {"target","__pycache__",".git",".artifacts",".tools"} for part in relative.parts) or any(part.startswith(".env") for part in relative.parts):
            raise ValueError("disallowed source path")
        if path.stat().st_size>2000000: raise ValueError("source file exceeds bound")
        data=path.read_bytes()
        rows.append({"path":relative.as_posix(),"bytes":len(data),"sha256":hashlib.sha256(data).hexdigest()})
    if sum(row["bytes"] for row in rows)>20000000: raise ValueError("source bundle exceeds bound")
    bundle=output/"NORA-LifeMate-source.zip"
    with zipfile.ZipFile(bundle,"x",compression=zipfile.ZIP_DEFLATED) as archive:
        for row in rows: archive.write(ROOT/row["path"],row["path"])
        archive.writestr("remote-source-manifest.json",json.dumps({"scope":"private uncommitted source snapshot; not a score or release certification","files":rows},indent=2))
    digest=hashlib.sha256(bundle.read_bytes()).hexdigest()
    (output/"NORA-LifeMate-Public-GPU.ipynb").write_text(json.dumps(notebook(digest, bundle.read_bytes() if embed_source else None),ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    receipt={"source_files":len(rows),"source_bytes":sum(row["bytes"] for row in rows),"bundle_bytes":bundle.stat().st_size,"bundle_sha256":digest,
             "source_embedded_in_notebook":embed_source,"dataset_included":False,"model_weights_included":False,"api_key_included":False,"uploaded_to_any_service":False,"linux_gpu_execution_verified":False}
    (output/"receipt.json").write_text(json.dumps(receipt,indent=2)+"\n")
    return receipt


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--embed-source",action="store_true",help="Avoid browser file chooser dependency; embed only the verified source ZIP")
    args = parser.parse_args()
    print(json.dumps(prepare(args.output.resolve(), args.embed_source)))


if __name__=="__main__": main()
