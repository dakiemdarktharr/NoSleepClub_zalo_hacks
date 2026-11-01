"""Offline build tooling, not part of NORA's resident Rust runtime.

Export the Windows nora-app normal/build dependency graph and observed license
texts. This is a partial build SBOM, not a legal clearance or binary analysis.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tomllib
from urllib.parse import parse_qs, urlsplit

TARGET = "x86_64-pc-windows-msvc"
MAX_TEXT = 1_000_000
MAX_TOTAL = 20_000_000
NOTICE_NAME = re.compile(r"^(licen[cs]e|copying|copyright|notice|unlicense)([._-].*)?$", re.I)


def linked(path: Path) -> bool:
    information = path.lstat()
    return stat.S_ISLNK(information.st_mode) or bool(getattr(information, "st_file_attributes", 0) & 0x400)


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_bytes((json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode())


def public_url(value: str | None) -> str | None:
    if not value:
        return None
    parsed = urlsplit(value)
    if (not any(c.isspace() for c in value) and parsed.scheme == "https" and parsed.hostname and not parsed.username
            and not parsed.password and not parsed.query and not parsed.fragment):
        return value
    return None


def check_source(source: str | None) -> None:
    if source is None:
        return
    kind, separator, value = source.partition("+")
    parsed = urlsplit(value)
    if (not separator or kind not in {"registry", "git"} or parsed.scheme != "https"
            or not parsed.hostname or parsed.username or parsed.password
            or any(c.isspace() for c in value)):
        raise ValueError("public inventory requires credential-free HTTPS Cargo sources")
    query = parse_qs(parsed.query, keep_blank_values=True)
    if (kind == "registry" and parsed.query) or set(query) - {"rev", "branch", "tag"}:
        raise ValueError("unsupported source query in public inventory")


def dependency_graph(metadata: dict) -> tuple[str, dict, dict]:
    packages = {p["id"]: p for p in metadata["packages"]}
    roots = [p["id"] for p in metadata["packages"]
             if p["name"] == "nora-app" and p["id"] in metadata["workspace_members"]]
    if len(roots) != 1:
        raise ValueError("expected one workspace nora-app")
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    edges, pending = {}, [roots[0]]
    while pending:
        package_id = pending.pop()
        if package_id in edges:
            continue
        selected = set()
        for dep in nodes[package_id]["deps"]:
            kinds = dep["dep_kinds"]
            if any(k["kind"] not in {None, "normal", "build", "dev"} for k in kinds):
                raise ValueError("unknown Cargo dependency kind")
            if any(k["kind"] in {None, "normal", "build"} for k in kinds):
                selected.add(dep["pkg"])
        edges[package_id] = sorted(selected)
        pending.extend(selected)
    return roots[0], {k: packages[k] for k in edges}, edges


def reference(package: dict) -> str:
    source = package["source"]
    suffix = "workspace" if source is None else sha(source.encode())[:16]
    return f"cargo:{package['name']}@{package['version']}:{suffix}"


def license_files(package_root: Path, declared: str | None) -> list[Path]:
    """Never follow links or a metadata license_file outside its package."""
    base = package_root.resolve(strict=True)
    selected = set()
    if declared:
        candidate = Path(declared)
        if not candidate.is_absolute():
            candidate = base / candidate
        if ".." in candidate.parts or not NOTICE_NAME.fullmatch(candidate.name):
            raise ValueError("unsafe declared license file")
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(base)
        cursor = candidate
        while cursor != base:
            if linked(cursor):
                raise ValueError("linked declared license file")
            cursor = cursor.parent
        selected.add(resolved)
    visited = 0
    for folder, directories, files in os.walk(base, followlinks=False):
        directories[:] = sorted(d for d in directories if d not in {".git", "target", "node_modules"}
                                 and not linked(Path(folder) / d))
        for name in sorted(files):
            visited += 1
            if visited > 100_000:
                raise ValueError("package file limit")
            path = Path(folder) / name
            if NOTICE_NAME.fullmatch(name) and not linked(path):
                path.resolve(strict=True).relative_to(base)
                selected.add(path)
        if len(selected) > 256:
            raise ValueError("package notice count limit")
    return sorted(selected, key=lambda p: p.relative_to(base).as_posix())


def export(metadata: dict, lock_bytes: bytes, output: Path) -> dict:
    lock_bytes = lock_bytes.replace(b"\r\n", b"\n")
    root_id, packages, edges = dependency_graph(metadata)
    lock = tomllib.loads(lock_bytes.decode())
    locked = {(p["name"], p["version"], p.get("source")): p for p in lock["package"]}
    if output.exists():
        raise ValueError("output must be new")
    output.mkdir(parents=True)
    (output / "texts").mkdir()
    rows, components, gaps, total = [], [], [], 0
    for package_id in sorted(packages, key=lambda i: reference(packages[i])):
        package = packages[package_id]
        source = package["source"]
        check_source(source)
        if source is None and package_id not in metadata["workspace_members"]:
            raise ValueError("external path dependency requires explicit notice review")
        key = (package["name"], package["version"], source)
        if key not in locked:
            raise ValueError("metadata package is absent from lockfile")
        row = {"name": package["name"], "version": package["version"],
               "source": source or "workspace", "declared_license": package["license"],
               "registry_checksum": locked[key].get("checksum"), "texts": []}
        component = {"type": "application" if package_id == root_id else "library",
                     "bom-ref": reference(package), "name": package["name"], "version": package["version"]}
        if package["license"]:
            component["licenses"] = [{"expression": package["license"]}]
        if source and source.startswith("registry+"):
            component["purl"] = f"pkg:cargo/{package['name']}@{package['version']}"
        repository = public_url(package.get("repository"))
        if repository:
            component["externalReferences"] = [{"type": "vcs", "url": repository}]
        properties = [{"name": "nora:cargo:source", "value": source or "workspace"}]
        if row["registry_checksum"]:
            # This hashes the source .crate archive, NOT the compiled library.
            properties.append({"name": "nora:cargo:source-archive-sha256", "value": row["registry_checksum"]})
        component["properties"] = properties
        if source is not None:
            base = Path(package["manifest_path"]).parent.resolve(strict=True)
            for path in license_files(base, package.get("license_file")):
                size = path.stat().st_size
                if not 0 < size <= MAX_TEXT:
                    raise ValueError("notice byte limit")
                content = path.read_bytes()
                if b"\0" in content or len(content) != size:
                    raise ValueError("notice changed or is not text")
                digest = sha(content)
                destination = output / "texts" / (digest + ".txt")
                if not destination.exists():
                    total += size
                    if total > MAX_TOTAL:
                        raise ValueError("total notice byte limit")
                    destination.write_bytes(content)
                row["texts"].append({"original_path": path.relative_to(base).as_posix(),
                                     "path": "texts/" + digest + ".txt", "bytes": size, "sha256": digest})
            if not row["texts"] or not row["declared_license"]:
                gaps.append({"name": row["name"], "version": row["version"],
                             "license_expression_missing": not bool(row["declared_license"]),
                             "package_notice_files_missing": not bool(row["texts"])})
        else:
            row["notice_scope"] = "first-party notices are packaged separately"
        rows.append(row)
        components.append(component)
    root_component = next(c for c in components if c["bom-ref"] == reference(packages[root_id]))
    inventory = {"schema_version": 1, "target": TARGET, "lock_sha256": sha(lock_bytes),
                 "lock_hash_basis": "Cargo.lock UTF-8 bytes with CRLF normalized to LF",
                 "scope": "nora-app Windows all-features normal and build dependencies, excluding dev-only edges; Cargo resolved graph, not binary linkage analysis",
                 "text_collection": "Observed license/notice files within each package. Alternative SPDX licenses are not selected or legally cleared.",
                 "native_and_model_coverage": "Incomplete: compiler/std/CRT, vendored native subcomponents, ORT binary, model/data/voice rights require separate review.",
                 "redistribution_cleared": False, "packages": rows, "gaps": gaps}
    bom = {"$schema": "http://cyclonedx.org/schema/bom-1.6.schema.json", "bomFormat": "CycloneDX",
           "specVersion": "1.6", "version": 1,
           "metadata": {"lifecycles": [{"phase": "build"}], "component": root_component,
                        "properties": [{"name": "nora:inventory:scope", "value": inventory["scope"]},
                                       {"name": "nora:inventory:coverage", "value": inventory["native_and_model_coverage"]},
                                       {"name": "nora:cargo:lock-sha256", "value": sha(lock_bytes)}]},
           "components": [c for c in components if c is not root_component],
           "dependencies": [{"ref": reference(packages[k]), "dependsOn": sorted(reference(packages[d]) for d in edges[k])}
                            for k in sorted(edges, key=lambda i: reference(packages[i]))],
           "compositions": [{"aggregate": "incomplete", "assemblies": [root_component["bom-ref"]]}]}
    write_json(output / "inventory.json", inventory)
    write_json(output / "bom.cdx.json", bom)
    return {"packages": len(rows), "notice_texts": len(list((output / "texts").glob("*.txt"))),
            "notice_bytes": total, "packages_with_metadata_or_text_gaps": len(gaps), "redistribution_cleared": False}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    environment = os.environ.copy()
    local = root / ".tools/cargo/bin/cargo.exe"
    if local.is_file():
        environment.update(CARGO_HOME=str(root / ".tools/cargo"), RUSTUP_HOME=str(root / ".tools/rustup"))
        environment["PATH"] = str(local.parent) + os.pathsep + environment["PATH"]
    process = subprocess.run([str(local) if local.is_file() else "cargo", "metadata", "--locked", "--offline",
                              "--all-features", "--filter-platform", TARGET, "--format-version", "1"],
                             cwd=root, env=environment, capture_output=True, check=True)
    if len(process.stdout) > 16_000_000:
        raise ValueError("metadata byte limit")
    print(json.dumps(export(json.loads(process.stdout), (root / "Cargo.lock").read_bytes(), args.output)))


if __name__ == "__main__":
    main()
