# Third-party notices and distribution scope

Original NORA code and documents use the root [MIT license](LICENSE). That grant does not replace third-party terms.

`nora-vieneu` includes modified Rust translations/adaptations of Apache-2.0 VieNeu reference algorithms. Its SPDX expression is `MIT AND Apache-2.0`; preserve its [NOTICE](crates/nora-vieneu/NOTICE) and [Apache license](crates/nora-vieneu/LICENSE-APACHE) with distributions. The NOTICE identifies reviewed upstream Git blobs and modifications. No upstream affiliation or endorsement is implied.

Cargo dependencies keep their own licenses. `Cargo.lock` pins the build graph; it is not a complete redistribution license bundle. Final binary packaging must include the licenses/notices for the exact selected dependencies, bundled SQLite, whisper.cpp/ggml, ONNX Runtime and their bundled components. First-party `unsafe_code=forbid` does not apply to those libraries.

Optional native `sea-g2p-rs` is pinned to commit `e825173f235d08ea19315b2b279fb11153b44cea` with Python features disabled. Audit its upstream LICENSE and the dictionary/data rights before redistributing the final voice pack; its Cargo package does not provide a license metadata field. This source foundation does not redistribute the dictionary.

Model graphs, tokenizer assets, preset voices, ONNX external data and dictionaries are downloaded separately for local validation. Their declared model-repository licenses, upstream attribution, training-data/voice rights and redistribution conditions require separate review. Hash manifests record integrity and revisions; they do not grant redistribution rights or authenticate a publisher.

No weights, runtime DLLs, real recordings, contest datasets or generated voice audio are included in this commit. `scripts/compare_vieneu_reference.py` is optional offline validation tooling and requires the caller to supply hash-matching original reference files; it is not part of the shipped Rust runtime.
