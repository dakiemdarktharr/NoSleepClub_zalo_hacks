"""Offline Docker bootstrap; never opens a dataset environment or gold label."""
from __future__ import annotations
import hashlib
import os
from pathlib import Path
import sys
import tempfile

SPLITS = ("private_test", "public_test", "train")
MODELS = (
    ("Qwen3.5-4B-Q4_K_M.gguf", 2740937888, "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4"),
    ("ggml-large-v3-turbo-q5_0.bin", 574041195, "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2"),
)


def detect_layout(mount: Path, selected: str | None = None) -> tuple[str, Path]:
    if selected is not None and selected not in SPLITS:
        raise ValueError("unsupported split")
    for split in ([selected] if selected else SPLITS):
        for directory in (mount / split, mount):
            # Only inspect public input metadata; environment files are opened
            # exclusively by the provided ToolEnv, never by this bootstrap.
            if (directory / f"{split}.jsonl").is_file() and (directory / f"{split}_tools_catalog.json").is_file() and (directory / "zaic_tools").is_dir() and (directory / "audio").is_dir():
                return split, directory
    raise ValueError("mounted data does not match the official split layout")


def verify_model(path: Path, expected_bytes: int, expected_hash: str) -> None:
    if path.stat().st_size != expected_bytes:
        raise ValueError("model size does not match frozen assets")
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    if digest.hexdigest() != expected_hash:
        raise ValueError("model checksum does not match frozen assets")


def main() -> None:
    split, directory = detect_layout(Path("/data").resolve(strict=True), os.environ.get("NORA_SPLIT"))
    model_root = Path("/models")
    for filename, size, digest in MODELS:
        verify_model(model_root / filename, size, digest)
    scratch = Path(tempfile.mkdtemp(prefix="nora-contest-"))
    root = directory.parent
    if directory.name != split:
        root = scratch / "input"
        root.mkdir()
        # Link the supplied split as a whole for the official ToolEnv root
        # convention. No environment content is copied, parsed or inspected.
        (root / split).symlink_to(directory, target_is_directory=True)
    result = Path("/app/result")
    result.mkdir(exist_ok=True)
    executable = "/app/bin/nora-lifemate"
    arguments = [executable, "--root", str(root), "--split", split,
        "--python", sys.executable, "--bridge", "/app/bridge.py",
        "--server", "/opt/llama/bin/llama-server", "--device", "CUDA0",
        "--model", str(model_root / MODELS[0][0]),
        "--asr-server", "/opt/whisper/bin/whisper-server", "--asr-model", str(model_root / MODELS[1][0]),
        "--context", "16384", "--deadline-ms", "30000", "--partition", "all",
        "--output", str(result / "submission.jsonl"), "--metrics", str(result / "samples.jsonl"),
        "--model-log", str(result / "model.log")]
    allowed = ("PATH", "LANG", "LC_ALL", "CUDA_VISIBLE_DEVICES", "CUDA_CACHE_PATH",
               "NVIDIA_VISIBLE_DEVICES", "NVIDIA_DRIVER_CAPABILITIES", "LD_LIBRARY_PATH")
    environment = {key: value for key in allowed if (value := os.environ.get(key)) is not None}
    environment["CUDA_CACHE_PATH"] = str(scratch / "cuda-cache")
    Path(environment["CUDA_CACHE_PATH"]).mkdir()
    os.execve(executable, arguments, environment)


if __name__ == "__main__":
    main()
