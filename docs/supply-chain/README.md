# Windows dependency inventory and notice export

Updated 2026-11-01. This is a **partial build inventory**, not a complete binary SBOM, license clearance or security certification. The Rust application and its tested executable are unchanged by this tooling update.

Working-tree refresh 2026-11-02: the two inventory files have been regenerated against the lockfile that adds `nora-lifemate`; the selected desktop `nora-app` dependency graph remains 357 packages, 251 observed texts and 18 gaps. The historical validation/package receipt below belongs to its original lockfile and executable. This desktop inventory does **not** cover the Linux LifeMate image, its CUDA runtime or its separately built model servers.

The [CycloneDX 1.6 build BOM](windows-build.cdx.json) records the locked `nora-app` Windows all-features normal/build dependency graph, excluding dev-only edges. Its composition is explicitly `incomplete`: compiler/std/CRT, vendored native subcomponents, the separate ORT DLL, models, dictionaries and voice rights still need separate coverage/review. Inclusion in this graph does not prove that code is linked into the executable. Package archive checksums are labelled as **source archive hashes**, not binary hashes.

The [license inventory](windows-licenses.json) records 357 packages and the observed paths, byte lengths and SHA-256 hashes of their license/notice files. The local export contains 251 unique text blobs, 827,719 bytes. Text bytes are preserved; identical files are deduplicated by hash. SPDX expressions are recorded as declared, without silently selecting an alternative license.

There are **18 metadata or packaged-text gaps**: AccessKit packages, egui-related packages, `clipboard-win`, `dasp_sample`, `gl_generator`, `khronos_api`, `profiling`, and the missing Cargo license field in `sea-g2p-rs`. The JSON lists exact names/versions and the kind of gap. This does not establish a license violation: some monorepo crates omit root license texts from their crate archive. Pinned repository-level attribution must be reviewed before closing those gaps. The root Apache-2.0 declaration for sea-g2p is already linked in [third-party notices](../../THIRD_PARTY_NOTICES.md).

## Reproduce without compilation

Developer tooling requires Python 3.11+ and the pinned Cargo toolchain/cache. Python is not added to the NORA runtime package. Cargo runs with `--locked --offline`; the exporter does not build dependencies, run their build scripts or call an LLM API.

```powershell
python scripts/export_dependency_notices.py --output .artifacts/dependency-notices
./scripts/Copy-DependencyNotices.ps1 -Source .artifacts/dependency-notices -ValidateOnly
python -m unittest discover -s scripts/tests -v
./scripts/tests/Test-DependencyNotices.ps1
./scripts/New-NoraPortable.ps1 -Executable ./target/x86_64-pc-windows-msvc/release/nora-app.exe -AssetSource D:/nora-assets -Destination D:/NORA -NoticesDirectory .artifacts/dependency-notices
```

Choose new export/package directories. The collector refuses declared license paths outside their package or with a secret filename, and skips symlink/reparse entries. The copier checks a canonical LF hash of Cargo.lock, bounded manifests/text sizes, exact `texts/<sha256>.txt` paths and file hashes before writing. Tampered text, path escapes and stale lockfiles are rejected before destination creation. Hash checks detect corruption; they do not authenticate the publisher.

The new local package includes this Cargo notice bundle and remains an **unsigned validation candidate** with `redistribution_cleared=false`. Its logical size is 1,203,779,356 bytes; the EXE SHA-256 remains `d65584f0a69807b577f0d10925cd7b0af6d73bc909c9d286899dbc1b7086e519`. Earlier [voice/resource measurements](../benchmarks/2026-11-01-integrated.json) apply to that unchanged binary; their historical package-size receipt is retained rather than overwritten.

Local checks passed: official [CycloneDX 1.6 JSON schema](https://cyclonedx.org/schema/bom-1.6.schema.json) and formats, five Python tests plus one symlink test skipped because Windows did not grant symlink creation, four PowerShell copy/tamper/path/lock controls, and real portable assembly. [Validation receipt](validation.json). Linux CI exercises the symlink test; Windows CI regenerates and compares the committed build BOM and publishes the observed notice bundle with its validation artifact. These checks do not close live-microphone, physical-device, clean-machine, signing or redistribution release gates.
