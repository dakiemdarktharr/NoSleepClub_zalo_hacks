"""Optional offline numerical validator; never part of the Rust runtime.

Requires NumPy, ORT 1.31.1 and tokenizers 0.22.2 in a separate test environment.
The three original Apache-2.0 source files must be supplied locally and match
the reviewed Git blob IDs below. Nothing is downloaded or played by this tool.
"""

import argparse
import hashlib
import importlib
import json
import os
from pathlib import Path
import sys
import time
import types


SOURCE_BLOBS = {
    "vieneu/_v3_turbo_engine/onnx_runtime_lite.py": "e7f1db482348df446640a6ea4f85fd2fbb6ad1fc",
    "vieneu/_v3_turbo_engine/rep_history.py": "2b9078e711bdd462888ec09f880d987730b78c06",
    "vieneu_utils/core_utils.py": "b534786b33a1c3d5b9a37531f297e4f09a66b3ec",
}


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("rust-result", "rust-wav", "model-dir", "codec-dir", "reference-source-dir", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists; choose a fresh result path")
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["HF_DATASETS_OFFLINE"] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    os.environ["OPENBLAS_NUM_THREADS"] = "2"
    os.environ["OMP_NUM_THREADS"] = "2"
    for relative, expected in SOURCE_BLOBS.items():
        payload = (args.reference_source_dir / relative).read_bytes()
        actual = hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        if actual != expected:
            raise ValueError("reference source differs from reviewed version: " + relative)

    import numpy as np
    import onnxruntime as ort
    if ort.__version__ != "1.31.1":
        raise ValueError("reference must use the same ORT1.31.1 as the measured Rust runtime")
    ort.disable_telemetry_events()
    # Bypass SDK package factories, not model computations. The audited files
    # import only these namespaces plus their normal NumPy/stdlib dependencies.
    for name, directory in (
        ("vieneu", args.reference_source_dir / "vieneu"),
        ("vieneu._v3_turbo_engine", args.reference_source_dir / "vieneu/_v3_turbo_engine"),
        ("vieneu_utils", args.reference_source_dir / "vieneu_utils"),
    ):
        module = types.ModuleType(name)
        module.__path__ = [str(directory)]
        module.__package__ = name
        sys.modules[name] = module
    engine_module = importlib.import_module("vieneu._v3_turbo_engine.onnx_runtime_lite")
    core_utils = importlib.import_module("vieneu_utils.core_utils")

    class OfflinePresetEngine(engine_module.OnnxV3LiteEngine):
        def _load_denoiser(self):
            # Unused by preset synthesis. No fresh-reference/cloning path runs.
            return None

    record = json.loads(args.rust_result.read_text(encoding="utf-8-sig"))
    if "stdout" in record:
        record = json.loads(record["stdout"].strip())
    options = record["options"]
    if options["sampling"]["temperature"] != 0.0:
        raise ValueError("use the Rust --greedy diagnostic for deterministic comparison")
    if not 1 <= options["max_new_frames"] <= 300:
        raise ValueError("reference validation frame bound exceeded")
    catalog = json.loads((args.model_dir / "voices_v3_turbo.json").read_text(encoding="utf-8"))
    preset = catalog["presets"][record["voice"]]
    speaker = np.asarray(preset["speaker_emb"], dtype=np.float32)
    codes = core_utils.strip_encoder_pad_frame(np.asarray(preset["codes"], dtype=np.int64))
    started = time.perf_counter()
    engine = OfflinePresetEngine(
        checkpoint_path=str(args.model_dir),
        onnx_dir=str(args.model_dir),
        codec_dir=str(args.codec_dir),
        threads=2,
    )
    loaded = time.perf_counter() - started
    started = time.perf_counter()
    chunks = list(engine.infer_stream(
        phonemes=record["phonemes"], speaker_emb=speaker, ref_codes=codes,
        use_ref_codes=True, temperature=0.0,
        top_k=options["sampling"]["top_k"], top_p=options["sampling"]["top_p"],
        repetition_penalty=options["sampling"]["repetition_penalty"],
        repetition_window=options["repetition_window"], frame_cap=options["frame_cap"],
        max_new_frames=options["max_new_frames"], chunk_frames=options["chunk_frames"],
    ))
    reference = np.concatenate(chunks).astype(np.float32)
    native_bytes = args.rust_wav.read_bytes()
    if native_bytes[:4] != b"RIFF" or native_bytes[8:12] != b"WAVE" or native_bytes[36:40] != b"data":
        raise ValueError("expected the diagnostic float32 WAV layout")
    native = np.frombuffer(native_bytes[44:], dtype="<f4")
    same_length = len(native) == len(reference)
    finite = bool(np.isfinite(native).all() and np.isfinite(reference).all())
    difference = native.astype(np.float64) - reference.astype(np.float64) if same_length else None
    maximum = float(np.max(np.abs(difference))) if same_length and finite else None
    rmse = float(np.sqrt(np.mean(difference ** 2))) if same_length and finite else None
    passed = bool(same_length and finite and maximum <= 1e-3 and rmse <= 1e-4)
    result = {
        "scope": "One preset phrase, greedy FP32 ONNX host/codec numerical check; not human voice quality or full SDK feature parity",
        "source_blobs": SOURCE_BLOBS,
        "ort_version": ort.__version__, "numpy_version": np.__version__,
        "voice": record["voice"], "source_phonemes": "Rust frontend output; G2P is not independently compared here",
        "unused_denoiser_initialization_disabled": True,
        "rust_samples": len(native), "reference_samples": len(reference),
        "reference_chunk_lengths": [len(chunk) for chunk in chunks],
        "same_length": same_length, "all_finite": finite,
        "max_abs_difference": maximum, "rmse": rmse,
        "max_abs_tolerance": 1e-3, "rmse_tolerance": 1e-4,
        "numerical_check_passed": passed,
        "reference_load_seconds": loaded,
        "reference_synthesis_seconds": time.perf_counter() - started,
    }
    encoded = json.dumps(result, ensure_ascii=False, indent=2)
    args.output.write_text(encoded + "\n", encoding="utf-8")
    print(encoded)
    return 0 if passed else 2


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    raise SystemExit(run())
