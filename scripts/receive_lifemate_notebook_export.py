"""Receive bounded private Colab output without mounting Drive or changing bytes.

This verifies transport, coverage, timing and SDK replay receipts. It cannot
measure public task accuracy; that result comes from the organiser.
"""
from __future__ import annotations

import argparse
import base64
import gzip
import hashlib
import io
import json
import math
from pathlib import Path
import re

MARKER = "NORA_PUBLIC_EXPORT_V1="
MAX_BYTES = 10_000_000
HASH = re.compile(r"[0-9a-f]{64}")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON field")
        result[key] = value
    return result


def parse(text: str):
    def invalid_constant(_):
        raise ValueError("non-finite JSON number")
    return json.loads(text, object_pairs_hook=unique_object,
                      parse_constant=invalid_constant)


def bounded_read(path: Path, limit: int = MAX_BYTES) -> bytes:
    with path.open("rb") as source:
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError("input exceeds transport bound")
    return data


def decoded_bytes(encoded: str, digest: str) -> bytes:
    if not isinstance(encoded, str) or len(encoded) > 4 * MAX_BYTES // 3 + 4:
        raise ValueError("encoded export exceeds bound")
    if not isinstance(digest, str) or HASH.fullmatch(digest) is None:
        raise ValueError("invalid expected digest")
    compressed = base64.b64decode(encoded, validate=True)
    if len(compressed) > MAX_BYTES:
        raise ValueError("compressed export exceeds bound")
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as source:
        raw = source.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError("decompressed export exceeds bound")
    if hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("export checksum mismatch")
    return raw


def rows(raw: bytes) -> dict:
    result = {}
    for line in raw.decode("utf-8").splitlines():
        if not line.strip():
            continue
        row = parse(line)
        sample_id = row.get("id")
        if (not isinstance(sample_id, str)
                or re.fullmatch(r"[A-Za-z0-9_-]{1,128}", sample_id) is None
                or sample_id in result):
            raise ValueError("invalid or duplicate sample id")
        result[sample_id] = row
    return result


def notebook_payload(notebook: dict) -> dict:
    exports = []
    for cell in notebook["cells"]:
        for output in cell.get("outputs", []):
            if output.get("output_type") != "stream":
                continue
            text = output.get("text", "")
            if isinstance(text, list):
                text = "".join(text)
            exports.extend(line[len(MARKER):] for line in text.splitlines()
                           if line.startswith(MARKER))
    if len(exports) != 1:
        raise ValueError("expected exactly one completed public export")
    return parse(exports[0])


def validate(payload: dict, expected_ids: set[str], source_sha256: str) -> dict:
    if not expected_ids or HASH.fullmatch(source_sha256) is None:
        raise ValueError("expected coverage/source must be explicit")
    if payload.get("source_snapshot_sha256") != source_sha256:
        raise ValueError("source snapshot does not match the intended candidate")
    if payload.get("native_ui_hardening") != {
        "embedded_asset_arrays": 0, "hf_ui_disabled": True, "npm_ui_disabled": True
    }:
        raise ValueError("native UI verification receipt missing")
    raw = decoded_bytes(payload["gzip_base64"], payload["sha256"])
    metric_raw = decoded_bytes(payload["metrics_gzip_base64"], payload["metrics_sha256"])
    predictions, metrics = rows(raw), rows(metric_raw)
    if set(predictions) != expected_ids or set(metrics) != expected_ids:
        raise ValueError("public prediction/metric coverage mismatch")
    for sample_id, prediction in predictions.items():
        if (set(prediction) != {"id", "tool_calls", "state_hash"}
                or not isinstance(prediction["state_hash"], str)
                or HASH.fullmatch(prediction["state_hash"]) is None
                or not isinstance(prediction["tool_calls"], list)
                or len(prediction["tool_calls"]) > 32):
            raise ValueError("invalid prediction envelope")
        for call in prediction["tool_calls"]:
            if (not isinstance(call, dict) or set(call) != {"name", "arguments"}
                    or not isinstance(call["name"], str)
                    or not isinstance(call["arguments"], dict)):
                raise ValueError("invalid tool call envelope")
        metric = metrics[sample_id]
        elapsed = metric.get("elapsed_ms")
        if (isinstance(elapsed, bool) or not isinstance(elapsed, (float, int))
                or not math.isfinite(elapsed) or not 0 <= elapsed <= 30_000
                or metric.get("status") != "complete" or metric.get("error") is not None
                or metric.get("executed_calls") != len(prediction["tool_calls"])):
            raise ValueError("failed, inconsistent or timed-out sample")
    preflight, replay = payload["preflight"], payload["replay"]
    if (preflight.get("public_samples") != len(expected_ids)
            or preflight.get("complete_coverage") is not True
            or preflight.get("runtime_gate_passed") is not True
            or preflight.get("official_sdk_replay_passed") is not True
            or preflight.get("failed_or_timeout_samples") != 0
            or preflight.get("submission_sha256") != payload["sha256"]
            or preflight.get("nora_accuracy_percent") is not None
            or replay.get("split") != "public_test"
            or replay.get("samples") != len(expected_ids)
            or replay.get("replay_mismatches") != 0
            or replay.get("passed") is not True):
        raise ValueError("incomplete or inconsistent public preflight/replay")
    elapsed = sorted(m["elapsed_ms"] for m in metrics.values())
    receipt = {
        "scope": "private result transport and runtime verification; NOT task accuracy",
        "samples": len(predictions), "submission_sha256": payload["sha256"],
        "metrics_sha256": payload["metrics_sha256"],
        "source_snapshot_sha256": source_sha256,
        "latency_ms": {"median": elapsed[len(elapsed) // 2],
                       "p95": elapsed[math.ceil(len(elapsed) * .95) - 1], "max": elapsed[-1]},
        "official_sdk_replay": replay, "native_ui_hardening": payload["native_ui_hardening"],
        "official_accuracy_percent": None, "uploaded_to_zalo": False,
    }
    return {"predictions": raw, "metrics": metric_raw, "receipt": receipt}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--notebook", type=Path, required=True)
    parser.add_argument("--public-input", type=Path, required=True)
    parser.add_argument("--expected-source-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    expected = set(rows(bounded_read(args.public_input)))
    if len(expected) != 634:
        raise ValueError("this public release requires exactly634 official input IDs")
    result = validate(notebook_payload(parse(bounded_read(args.notebook).decode("utf-8"))),
                      expected, args.expected_source_sha256)
    args.output.mkdir(parents=True, exist_ok=False)
    for name, data in (("submission.jsonl", result["predictions"]),
                       ("samples.jsonl", result["metrics"])):
        with (args.output / name).open("xb") as target:
            target.write(data)
    with (args.output / "transport-receipt.json").open("x", encoding="utf-8") as target:
        json.dump(result["receipt"], target, ensure_ascii=False, indent=2)
        target.write("\n")
    print(json.dumps(result["receipt"], ensure_ascii=False))


if __name__ == "__main__":
    main()
