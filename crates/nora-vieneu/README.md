# nora-vieneu

Rust CPU host for a **local, verified VieNeu v3 Turbo update model pack**.
No Python, remote-code loading, model download, voice cloning or service runs.
Code derived from the upstream reference retains Apache attribution in NOTICE.

Implemented: bounded config and NPZ float32 loading; phoneme tokenizer/prompt;
speaker projection and tied heads; top-k/top-p/repetition sampling with bounded
history; codec state layout; native ORT graph execution/KV ownership; streaming
or full codec (one loaded, not both); per-call cancellation/PCM limits.
Optional `g2p` uses pinned sea-g2p native Rust with Python features disabled.

`onnx` requires a trusted ONNX Runtime 1.31 CPU library supplied by the caller.
Create one ORT environment per process, pass it to
`onnx::OnnxEngine::load(&pack, &environment, threads)` and call
`synthesize_phonemes` with a `VoicePreset`, `SynthesisOptions`, cancellation token
and bounded audio sink. The sink must own playback/backpressure and propagate
errors. Native kernels are not preempted in the middle of a call.

Model directories must contain the upstream update export filenames, config,
tokenizer, heads NPZ and external-data files. Codec directory contains full
decoder graph/data; metadata + decode_step enable streaming. Voice JSON uses
`speaker_embedding` and `ref_codes`, with dimensions verified against config.
No fake audio is produced if graphs/voices are missing. Host-loader tests use
explicit synthetic placeholders only; those are not runnable model fixtures.

FP32 is the supported profile. The trusted manifest must identify FP32 graph
artifacts; this loader does not parse ONNX to certify quantization/precision.
There is no automatic INT8 conversion/CPU-feature fallback. Sampling is
distribution-equivalent, not NumPy RNG bit-identical. Layernorm/matvec floating
point order can differ. Streaming frame-cap rules are ported; offline babble
retry selection, fresh-reference cloning/denoiser, automatic text chunking and
emotion-tag text frontend still require separate validation/integration.

File bounds do not sandbox ONNX/tokenizer/native parsers. Asset hashes/signatures,
external-data paths, runtime patching, process/egress isolation and whole-process
RAM/VRAM accounting remain application responsibilities. Shared ORT prepacked
weights may help eligible buffers; duplicate graph weights are not claimed to
be eliminated. Human Vietnamese voice-quality/latency/resource parity and the
full production security matrix remain release gates.

Diagnostic render (writes a new WAV, no playback/network):

```text
cargo run -p nora-vieneu --example vieneu-render --features onnx,g2p -- \
  ORT_DLL MODEL_DIR CODEC_DIR DICTIONARY OUTPUT_WAV TEXT [MAX_FRAMES] [GREEDY]
```

`GREEDY`/`--greedy` sets only temperature to zero for reference parity; default
chunk_frames25, top_k25, top_p0.95, repetition_penalty1.2/window64 and frame_cap
remain unchanged. JSON diagnostics include the exact phonemes/options; this is
test metadata, not a production telemetry policy. The WAV writer buffers64KiB.
