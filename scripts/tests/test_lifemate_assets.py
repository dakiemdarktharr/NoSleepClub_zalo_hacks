"""Staging checks use tiny original fixtures, never model assets or data."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("lifemate_assets", Path(__file__).parents[1] / "prepare_lifemate_assets.py")
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class AssetStagingTests(unittest.TestCase):
    def test_staging_verifies_integrity_and_never_overwrites_a_different_file(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source, destination = root / "source", root / "destination"
            source.write_bytes(b"toy weights")
            asset = {"filename":"destination", "bytes":11, "sha256":hashlib.sha256(b"toy weights").hexdigest()}
            self.assertEqual(assets.stage(source,destination,asset,copy=True), "copy")
            self.assertEqual(assets.stage(source,destination,asset), "already-verified")
            destination.write_bytes(b"do not overwrite")
            with self.assertRaises(ValueError): assets.stage(source,destination,asset)
            self.assertEqual(destination.read_bytes(),b"do not overwrite")

    def test_wrong_source_is_rejected_before_destination_creation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source, destination = root / "source", root / "destination"
            source.write_bytes(b"abc")
            with self.assertRaises(ValueError):
                assets.stage(source,destination,{"filename":"destination","bytes":3,"sha256":"0"*64})
            self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
