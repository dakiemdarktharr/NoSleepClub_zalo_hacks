"""Stage two caller-supplied, hash-pinned models; never downloads data or keys.

The default staging directory is ignored models/ in this repository. Hard links
avoid duplicating 3.315 GB on the same filesystem. Treat both original assets
and staged links as immutable; use --copy for an independent byte copy.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

REPOSITORY = Path(__file__).resolve().parents[1]
MANIFEST = REPOSITORY / "assets/manifests/lifemate-offline.v1.json"


def verify(path: Path, asset: dict) -> None:
    if not path.is_file() or path.stat().st_size != asset["bytes"]:
        raise ValueError(f"wrong file size for {asset['filename']}")
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    if digest.hexdigest() != asset["sha256"]:
        raise ValueError(f"wrong checksum for {asset['filename']}")


def stage(source: Path, destination: Path, asset: dict, copy: bool = False) -> str:
    source = source.resolve(strict=True)
    verify(source, asset)
    if destination.is_symlink():
        raise ValueError("refusing a symlink in asset staging")
    if destination.exists():
        verify(destination, asset)
        return "already-verified"
    if not copy:
        try:
            os.link(source, destination)
            return "hardlink"
        except FileExistsError:
            raise ValueError("asset staging destination changed") from None
        except OSError:
            pass  # Cross-device or unsupported hard links: bounded byte copy.
    # Exclusive creation never overwrites another file. A failed copy removes
    # only the exact file this invocation created, without recursive cleanup.
    with destination.open("xb") as output:
        try:
            with source.open("rb") as input_file:
                shutil.copyfileobj(input_file, output, length=1024 * 1024)
            output.flush()
            os.fsync(output.fileno())
        except BaseException:
            output.close()
            destination.unlink()
            raise
    verify(destination, asset)
    return "copy"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--llm", type=Path, required=True)
    parser.add_argument("--asr", type=Path, required=True)
    parser.add_argument("--destination", type=Path, default=REPOSITORY / "models")
    parser.add_argument("--copy", action="store_true")
    args = parser.parse_args()
    assets = json.loads(MANIFEST.read_text(encoding="utf-8"))["models"]
    directory = args.destination.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    report = []
    for asset in assets:
        filename = asset["filename"]
        if Path(filename).name != filename or "/" in filename or "\\" in filename:
            raise ValueError("invalid manifest filename")
        source = {"llm": args.llm, "asr": args.asr}[asset["role"]]
        mode = stage(source, directory / filename, asset, args.copy)
        report.append({"filename": filename, "bytes": asset["bytes"], "sha256": asset["sha256"], "staging": mode})
    print(json.dumps({"scope": "verified local asset staging; no inference or accuracy claim", "models": report}, indent=2))


if __name__ == "__main__":
    main()
