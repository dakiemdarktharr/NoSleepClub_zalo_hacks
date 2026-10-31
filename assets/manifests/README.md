# Local validation asset catalog

The adjacent JSON files contain only file sizes and SHA-256 digests of the local assets used in the 01/11/2026 Rust validation. They are not signed publisher metadata. Obtain the assets from their upstreams, review licenses and provenance, and verify before loading. The runtime never downloads or executes hub code. The explicit setup script below downloads only pinned, bounded data files and verifies SHA-256 before publishing them locally.

| Manifest | Expected root contents | Source pin |
|---|---|---|
| `vieneu-v3-turbo-fp32.json` | VieNeu files plus `codec/` subdirectory and `voices_v3_turbo.json` | [pnnbao-ump/VieNeu-TTS-v3-Turbo](https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Turbo/tree/61b85e3d937fbbacb387714180e8182823512523), revision `61b85e3d937fbbacb387714180e8182823512523`; [OpenMOSS codec](https://huggingface.co/OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX/tree/ceff0d0749bfb3fa2d61149794ec6feef0d1e1ae), revision `ceff0d0749bfb3fa2d61149794ec6feef0d1e1ae` |
| `silero-vad.json` | `silero_vad.onnx` | [snakers4/silero-vad](https://github.com/snakers4/silero-vad); reviewed model Git blob `80c5592ef1f4c9ede3e357bbd02eb863358a6a9d` |
| `sea-g2p.json` | `sea_g2p.bin` | [sea-g2p](https://github.com/pnnbao97/sea-g2p/tree/e825173f235d08ea19315b2b279fb11153b44cea), `python/sea_g2p/sea_g2p.bin` |
| `onnxruntime-win-x64-cpu.json` | `onnxruntime.dll` | [Microsoft ONNX Runtime v1.31.1](https://github.com/microsoft/onnxruntime/releases/tag/v1.31.1), Windows x64 CPU release |
| `whisper-large-v3-turbo-q5_0.json` | `ggml-large-v3-turbo-q5_0.bin` | [ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1), revision `5359861c739e955e79d9a303bcbc70fb988958b1` |

The original ORT release ZIP SHA-256 checked during setup was `5869e26bb25314375f2d5ef1877e3ea06b1543d6c710e6e4a1ec2f7177ab4b2c`. The manifest verifies the extracted DLL, not every component of a redistributable runtime package. CPU runtime ABI is pinned to ORT 1.31.1 / Rust `ort 2.0.0-rc.15` API31. Do not mix DLLs from another ambient installation.

The selected VieNeu/codec repositories declare Apache-2.0; this is not an independent data/voice-rights clearance. Exact TTS assets are FP32; this catalog does not describe an INT8 conversion. Whisper Q5 was exercised in the integrated synthetic WAV journey; see [validation](../../docs/validation.md). All five packs contain 16 files totaling 1,177,282,839 logical bytes.

```powershell
./scripts/Prepare-VoiceAssets.ps1 -Destination D:/nora-assets
./scripts/Prepare-VoiceAssets.ps1 -Destination D:/nora-assets -VerifyOnly
```

PowerShell 7 setup uses streaming transfers, per-file size limits, a transfer deadline and create-new/atomic publication. It does not replace an existing file with a different hash. The ORT archive is pinned and only its single expected DLL entry is extracted. VieNeu graphs come from the pinned `onnx_update/` directory; the voice catalog is pinned separately to VieNeu-TTS commit `eecd615d04e3d0e4b9c8330af66306da789b8f8d`. Silero uses commit `bfdc0193023f121ea5b3cc7b176dbed570a68a59`.

```powershell
& $nora verify-pack 'D:/nora-assets/vieneu-fp32' assets/manifests/vieneu-v3-turbo-fp32.json
```

Success reports `integrity_verified=true` and `publisher_authenticated=false`. Keep catalogs in trusted application storage. Verification followed by loading is not protected against an authorized OS account modifying the files; production needs immutable staging/ownership policy and parser isolation. No extraction API is used for the heads NPZ: bounded NPY entries are read as data in Rust.
