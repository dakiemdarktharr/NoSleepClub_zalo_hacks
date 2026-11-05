"""Verify downloaded release assets without loading any model or pickle payload."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile


def digest(stream):
    return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    manifest = json.loads((args.directory / "manifest.json").read_bytes())
    checked = 0
    for asset in manifest["assets"]:
        name = asset["name"]
        if Path(name).name != name or "/" in name or "\\" in name:
            raise ValueError("invalid asset name")
        path = args.directory / name
        with path.open("rb") as stream:
            if digest(stream) != asset["sha256"] or path.stat().st_size != asset["bytes"]:
                raise ValueError(f"asset mismatch: {name}")
        with zipfile.ZipFile(path) as archive:
            bad = archive.testzip()
            if bad:
                raise ValueError(f"archive CRC error: {bad}")
            files = asset.get("files")
            if files is not None:
                if sorted(archive.namelist()) != sorted(row["path"] for row in files):
                    raise ValueError(f"archive inventory mismatch: {name}")
                for row in files:
                    with archive.open(row["path"]) as stream:
                        if digest(stream) != row["sha256"]:
                            raise ValueError(f"member hash mismatch: {row['path']}")
                    if archive.getinfo(row["path"]).file_size != row["bytes"]:
                        raise ValueError(f"member size mismatch: {row['path']}")
            checked += 1
    print(json.dumps({"verified_assets": checked, "release": manifest["release_tag"],
                      "model_loaded": False, "quality_evaluated": False}))


if __name__ == "__main__":
    main()
