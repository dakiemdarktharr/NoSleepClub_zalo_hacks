import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("notices", Path(__file__).parents[1] / "export_dependency_notices.py")
notices = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(notices)


def fixture(root):
    packages, nodes = [], []
    for name in ("nora-app", "normal", "build", "dev"):
        directory = root / name
        directory.mkdir()
        (directory / "Cargo.toml").write_text("")
        packages.append({"id": name, "name": name, "version": "1.0.0", "source": None if name == "nora-app" else "registry+https://github.com/rust-lang/crates.io-index",
                         "manifest_path": str(directory / "Cargo.toml"), "license": "MIT", "license_file": None, "repository": "https://example.com/project"})
        nodes.append({"id": name, "deps": []})
        (directory / "LICENSE").write_bytes(b"A synthetic license fixture.\n")
    nodes[0]["deps"] = [{"pkg": name, "dep_kinds": [{"kind": kind, "target": None}]} for name, kind in (("normal", None), ("build", "build"), ("dev", "dev"))]
    metadata = {"packages": packages, "workspace_members": ["nora-app"], "resolve": {"nodes": nodes}}
    lock = "\n".join(f'[[package]]\nname = "{p["name"]}"\nversion = "1.0.0"\n' + (f'source = "{p["source"]}"\nchecksum = "' + 'a'*64 + '"\n' if p["source"] else "") for p in packages)
    return metadata, lock.encode()


class NoticeTests(unittest.TestCase):
    def test_public_source_metadata_cannot_include_url_credentials_or_tokens(self):
        for source in ("git+https://user:secret@example.com/repo#abcd", "registry+https://example.com/index?token=secret", "git+https://example.com/repo?access_token=secret#abcd", "git+file:///private/repo"):
            with self.assertRaises(ValueError):
                notices.check_source(source)
        notices.check_source("git+https://github.com/example/repo?rev=abcd#abcd")

    def test_build_dependencies_included_and_dev_only_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            metadata, _ = fixture(Path(tmp))
            _, packages, edges = notices.dependency_graph(metadata)
            self.assertEqual(set(packages), {"nora-app", "normal", "build"})
            self.assertEqual(set(edges["nora-app"]), {"normal", "build"})
            metadata["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] = "unrecognized"
            with self.assertRaises(ValueError):
                notices.dependency_graph(metadata)

    def test_declared_license_cannot_escape_or_select_secret_filename(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            package = root / "package"
            package.mkdir()
            (root / "LICENSE").write_text("outside")
            (package / ".env.local").write_text("synthetic secret")
            for path in ("../LICENSE", str(root / "LICENSE"), ".env.local"):
                with self.assertRaises(ValueError):
                    notices.license_files(package, path)

    def test_linked_license_is_not_copied(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            package = root / "package"
            package.mkdir()
            outside = root / "LICENSE"
            outside.write_text("outside")
            try:
                (package / "NOTICE").symlink_to(outside)
            except OSError:
                self.skipTest("OS does not permit test symlinks")
            self.assertEqual(notices.license_files(package, None), [])
            with self.assertRaises(ValueError):
                notices.license_files(package, "NOTICE")

    def test_deduplicated_export_has_stable_graph_and_honest_gaps(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, lock = fixture(root)
            metadata["packages"][2]["license"] = None
            result = notices.export(metadata, lock, root / "out")
            self.assertEqual(result["packages"], 3)
            self.assertEqual(result["notice_texts"], 1)
            self.assertEqual(result["packages_with_metadata_or_text_gaps"], 1)
            self.assertFalse(result["redistribution_cleared"])
            bom = json.loads((root / "out/bom.cdx.json").read_text())
            references = {bom["metadata"]["component"]["bom-ref"]} | {c["bom-ref"] for c in bom["components"]}
            self.assertEqual(references, {d["ref"] for d in bom["dependencies"]})
            self.assertTrue(all(set(d["dependsOn"]) <= references for d in bom["dependencies"]))
            self.assertEqual(bom["compositions"][0]["aggregate"], "incomplete")
            second = notices.export(metadata, lock, root / "second")
            self.assertEqual(result, second)
            self.assertEqual((root / "out/bom.cdx.json").read_bytes(), (root / "second/bom.cdx.json").read_bytes())
            with self.assertRaises(ValueError):
                notices.export(metadata, lock, root / "out")

    def test_stale_metadata_and_credentials_in_repository_url_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            metadata, lock = fixture(root)
            metadata["packages"][1]["version"] = "2.0.0"
            with self.assertRaises(ValueError):
                notices.export(metadata, lock, root / "out")
            self.assertIsNone(notices.public_url("https://secret@example.com/project"))
            self.assertIsNone(notices.public_url("https://example.com/project?token=secret"))
            self.assertIsNone(notices.public_url("file:///private/LICENSE"))


if __name__ == "__main__":
    unittest.main()
