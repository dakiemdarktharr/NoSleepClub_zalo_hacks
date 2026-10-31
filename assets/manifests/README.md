# Local validation asset catalog

The adjacent JSON files contain only file sizes and SHA-256 digests of the local assets used in the 01/11/2026 Rust validation. They are not signed publisher metadata. Obtain the assets from their upstreams, review licenses and provenance, and verify before loading. The CLI never downloads or executes hub code.

| Manifest | Expected root contents | Source pin |
|---|---|---|
| `vieneu-v3-turbo-fp32.json` | VieNeu files plus `codec/` subdirectory and `voices_v3_turbo.json` | [pnnbao-ump/VieNeu-TTS-v3-Turbo](https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Turbo/tree/61b85e3d937fbbacb387714180e8182823512523), revision `61b85e3d937fbbacb387714180e8182823512523`; [OpenMOSS codec](https://huggingface.co/OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX/tree/ceff0d0749bfb3fa2d61149794ec6feef0d1e1ae), revision `ceff0d0749bfb3fa2d61149794ec6feef0d1e1ae` |
| `silero-vad.json` | `silero_vad.onnx` | [snakers4/silero-vad](https://github.com/snakers4/silero-vad); reviewed model Git blob `80c5592ef1f4c9ede3e357bbd02eb863358a6a9d` |
| `sea-g2p.json` | `sea_g2p.bin` | [sea-g2p](https://github.com/pnnbao97/sea-g2p/tree/e825173f235d08ea19315b2b279fb11153b44cea), `python/sea_g2p/sea_g2p.bin` |
| `onnxruntime-win-x64-cpu.json` | `onnxruntime.dll` | [Microsoft ONNX Runtime v1.31.1](https://github.com/microsoft/onnxruntime/releases/tag/v1.31.1), Windows x64 CPU release |

The original ORT release ZIP SHA-256 checked during setup was `5869e26bb25314375f2d5ef1877e3ea06b1543d6c710e6e4a1ec2f7177ab4b2c`. The manifest verifies the extracted DLL, not every component of a redistributable runtime package. CPU runtime ABI is pinned to ORT 1.31.1 / Rust `ort 2.0.0-rc.15` API31. Do not mix DLLs from another ambient installation.

The selected VieNeu/codec repositories declare Apache-2.0; this is not an independent data/voice-rights clearance. Exact assets are FP32; this catalog does not describe an INT8 conversion. No Whisper model was downloaded/benchmarked in this handoff, so there is no untested ASR digest here.

```powershell
& $nora verify-pack 'D:/nora-assets/vieneu-fp32' assets/manifests/vieneu-v3-turbo-fp32.json
```

Success reports `integrity_verified=true` and `publisher_authenticated=false`. Keep catalogs in trusted application storage. Verification followed by loading is not protected against an authorized OS account modifying the files; production needs immutable staging/ownership policy and parser isolation. No extraction API is used for the heads NPZ: bounded NPY entries are read as data in Rust.
