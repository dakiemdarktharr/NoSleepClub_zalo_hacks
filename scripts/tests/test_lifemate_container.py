"""Test input-layout handling without Docker, CUDA, models or contest data."""
import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("lifemate_container", Path(__file__).parents[2] / "crates/nora-lifemate/python/container_entry.py")
entry = importlib.util.module_from_spec(spec)
spec.loader.exec_module(entry)


class ContainerLayoutTests(unittest.TestCase):
    def fixture(self, root, split):
        (root / f"{split}.jsonl").touch()
        (root / f"{split}_tools_catalog.json").touch()
        (root / "zaic_tools").mkdir()
        (root / "audio").mkdir()
        # No environment directory is needed or accessed for layout detection.

    def test_accepts_both_official_mount_shapes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.fixture(root, "private_test")
            self.assertEqual(entry.detect_layout(root), ("private_test", root))
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            nested = root / "public_test"
            nested.mkdir()
            self.fixture(nested, "public_test")
            self.assertEqual(entry.detect_layout(root), ("public_test", nested))

    def test_rejects_traversal_or_incomplete_layout(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with self.assertRaises(ValueError):
                entry.detect_layout(root, "../private_test")
            (root / "private_test.jsonl").touch()
            with self.assertRaises(ValueError):
                entry.detect_layout(root)

    def test_model_hash_is_streamed_and_rejects_wrong_size_or_content(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/"toy-model"
            path.write_bytes(b"abc")
            digest=hashlib.sha256(b"abc").hexdigest()
            entry.verify_model(path,3,digest)
            with self.assertRaises(ValueError):entry.verify_model(path,4,digest)
            with self.assertRaises(ValueError):entry.verify_model(path,3,"0"*64)

    def test_runtime_model_pins_match_the_reviewable_manifest(self):
        manifest = Path(__file__).parents[2] / "assets/manifests/lifemate-offline.v1.json"
        assets = json.loads(manifest.read_text(encoding="utf-8"))
        self.assertEqual([(a["filename"],a["bytes"],a["sha256"]) for a in assets["models"]], list(entry.MODELS))
        self.assertLessEqual(assets["configured_context_tokens"],16384)


if __name__ == "__main__":
    unittest.main()
