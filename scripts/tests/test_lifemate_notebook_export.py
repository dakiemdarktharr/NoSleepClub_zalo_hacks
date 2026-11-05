"""Transport tests use synthetic rows, never task labels or model inference."""
import base64
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[1] / "receive_lifemate_notebook_export.py"
SPEC = importlib.util.spec_from_file_location("notebook_export", MODULE)
export = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(export)


def fixture():
    predictions = [{"id": "public_0002", "tool_calls": [], "state_hash": "a" * 64}]
    metrics = [{"id": "public_0002", "elapsed_ms": 1.5, "status": "complete",
                "error": None, "executed_calls": 0}]
    raw = (json.dumps(predictions[0], ensure_ascii=False) + "\n").encode()
    metric_raw = (json.dumps(metrics[0]) + "\n").encode()
    payload = {
        "source_snapshot_sha256": "c" * 64,
        "native_ui_hardening": {"embedded_asset_arrays": 0, "hf_ui_disabled": True, "npm_ui_disabled": True},
        "sha256": hashlib.sha256(raw).hexdigest(),
        "metrics_sha256": hashlib.sha256(metric_raw).hexdigest(),
        "gzip_base64": base64.b64encode(gzip.compress(raw)).decode(),
        "metrics_gzip_base64": base64.b64encode(gzip.compress(metric_raw)).decode(),
        "preflight": {"public_samples": 1, "complete_coverage": True,
                      "runtime_gate_passed": True, "official_sdk_replay_passed": True,
                      "failed_or_timeout_samples": 0, "submission_sha256": hashlib.sha256(raw).hexdigest(),
                      "nora_accuracy_percent": None},
        "replay": {"split": "public_test", "samples": 1, "replay_mismatches": 0, "passed": True},
    }
    return payload, raw


class ExportTests(unittest.TestCase):
    def test_valid_transport_preserves_exact_bytes(self):
        payload, raw = fixture()
        result = export.validate(payload, {"public_0002"}, "c" * 64)
        self.assertEqual(result["predictions"], raw)
        self.assertIsNone(result["receipt"]["official_accuracy_percent"])

    def test_corruption_and_wrong_source_are_rejected(self):
        payload, _ = fixture()
        with self.assertRaises(ValueError):
            export.validate(payload, {"public_0002"}, "d" * 64)
        payload["sha256"] = "e" * 64
        with self.assertRaises(ValueError):
            export.validate(payload, {"public_0002"}, "c" * 64)

    def test_missing_ids_and_inconsistent_receipts_are_rejected(self):
        payload, _ = fixture()
        with self.assertRaises(ValueError):
            export.validate(payload, {"public_0002", "public_0003"}, "c" * 64)
        for section, field, value in [("replay", "passed", False), ("preflight", "runtime_gate_passed", False),
                                       ("preflight", "nora_accuracy_percent", 100)]:
            changed = copy.deepcopy(payload)
            changed[section][field] = value
            with self.assertRaises(ValueError):
                export.validate(changed, {"public_0002"}, "c" * 64)

    def test_duplicate_records_and_json_fields_are_rejected(self):
        _, raw = fixture()
        with self.assertRaises(ValueError):
            export.rows(raw + raw)
        with self.assertRaises(ValueError):
            export.parse('{"id":"a","id":"b"}')
        with self.assertRaises(ValueError):
            export.parse('{"elapsed_ms":NaN}')

    def test_decompression_is_bounded(self):
        raw = b"x" * 1000
        encoded = base64.b64encode(gzip.compress(raw)).decode()
        with patch.object(export, "MAX_BYTES", 128):
            with self.assertRaises(ValueError):
                export.decoded_bytes(encoded, hashlib.sha256(raw).hexdigest())

    def test_export_must_come_from_exactly_one_stream_result(self):
        payload, _ = fixture()
        output = {"output_type": "stream", "text": [export.MARKER + json.dumps(payload) + "\n"]}
        self.assertEqual(export.notebook_payload({"cells": [{"outputs": [output]}]}), payload)
        with self.assertRaises(ValueError):
            export.notebook_payload({"cells": [{"outputs": [output, output]}]})


if __name__ == "__main__":
    unittest.main()
