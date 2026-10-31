# Third-party notices and distribution scope

Original NORA code and documents use the root [MIT license](LICENSE). That grant does not replace third-party terms.

`nora-vieneu` includes modified Rust translations/adaptations of Apache-2.0 VieNeu reference algorithms. Its SPDX expression is `MIT AND Apache-2.0`; preserve its [NOTICE](crates/nora-vieneu/NOTICE) and [Apache license](crates/nora-vieneu/LICENSE-APACHE) with distributions. The NOTICE identifies reviewed upstream Git blobs and modifications. No upstream affiliation or endorsement is implied.

Cargo dependencies keep their own licenses. `Cargo.lock` pins the build graph; it is not a complete redistribution license bundle. Final binary packaging must include the licenses/notices for the exact selected dependencies, bundled SQLite, whisper.cpp/ggml, ONNX Runtime and their bundled components. First-party `unsafe_code=forbid` does not apply to those libraries.

Native `sea-g2p-rs` is pinned to commit `e825173f235d08ea19315b2b279fb11153b44cea` with Python features disabled. Its [root source LICENSE](https://github.com/pnnbao97/sea-g2p/blob/e825173f235d08ea19315b2b279fb11153b44cea/LICENSE) declares Apache-2.0; its Cargo package omits a license metadata field. Dictionary/data and voice rights still require separate redistribution review. This source repository does not redistribute the dictionary.

Model graphs, tokenizer assets, preset voices, ONNX external data and dictionaries are downloaded separately for local validation. Their declared model-repository licenses, upstream attribution, training-data/voice rights and redistribution conditions require separate review. Hash manifests record integrity and revisions; they do not grant redistribution rights or authenticate a publisher.

No weights, runtime DLLs, real recordings, contest datasets or generated voice audio are included in this commit. `scripts/compare_vieneu_reference.py` is optional offline validation tooling and requires the caller to supply hash-matching original reference files; it is not part of the shipped Rust runtime.

`New-NoraPortable.ps1` assembles an unsigned local validation package with a blank credential template and these first-party/ported-code notices. It is not a complete third-party license bundle or authorization to redistribute every optional asset. Complete the exact-build notice inventory, model/data/voice review, signing and clean-machine release gates before public binary distribution.
