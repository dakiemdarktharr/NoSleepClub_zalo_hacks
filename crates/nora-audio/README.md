# nora-audio

Original Rust audio frontend with bounded PCM queues, framing, hysteresis
endpointing, generation-bound TTS playback and band-limited Rubato resampling.
First-party code forbids unsafe Rust. It does not implement speaker authentication,
acoustic echo cancellation, a permission broker, or a complete voice assistant.

## Features

- Default: pure Rust frontend; no microphone/model loading.
- onnx: Silero 16kHz CPU ONNX host using a shared ORT Environment.
- whisper: bounded CPU whisper.cpp host via maintained whisper-rs.
- capture: explicit CPAL default-device capture; caller obtains microphone consent.

Models are external and must pass the application's asset/trust policy before
loading. No hub code, network download or Python interpreter is used.

The frontend never grows a PCM queue to absorb slow processing. Ring writes reject
overflow atomically; capture reports dropped frames/backend errors. A caller must
invalidate/reset an utterance after discontinuity. PlaybackQueue::interrupt
flushes audio and rejects stale producer generations. It does not cancel a goal.

MonoResampler processes exact input chunks into caller-preallocated buffers and
reports filter delay. Use FrameAssembler around arbitrary device chunks.
Endpoint timestamps refer to mono samples; account for resampler delay and session
boundaries before mapping them to wall-clock time.

## Native runtime

Create one ONNX Runtime environment per process and pass it to VAD and TTS:

~~~ignore
let environment = ort::init_from(approved_ort_dll)?.with_telemetry(false).build()?;
let mut vad = nora_audio::SileroVad::load(&environment, approved_silero_model, 1)?;
let probability = vad.probability(&[0.0; 512])?;
~~~

The supported Silero signature is input/state/sr -> output/stateN with 512 new
samples, 64 context samples and a [2,1,128] recurrent state at 16 kHz. The protocol
follows snakers4/silero-vad/src/silero_vad/utils_vad.py (MIT). No upstream Python
source or model weight is copied into this crate.

WhisperEngine::load(path, max_samples, threads) uses one CPU model context and
reuses a state. transcribe_16khz(samples, "vi") disables cross-request context,
translation and progress printing. Model memory is separate from the bounded
request samples; CPU build success is not a 4 GB GPU or Vietnamese quality benchmark.

The tested x86 Whisper profile requires AVX2/F16C/FMA and checks those features
before native initialization. GGML_NATIVE=OFF prevents host-specific tuning but
does not disable all upstream SIMD defaults. Older CPUs need a separately built
and tested profile. With MinGW, whisper-rs-sys0.15's Windows-only MSVC UTF-8 flag
needs the GNU build setting CMAKE_CXX_FLAGS=-finput-charset=UTF-8; this setting is
not required for MSVC. GNU cargo check passed, but linking native Whisper tests
also exposed ggml archive prefix incompatibility in upstream Win32 CMake logic.
Use the separately verified MSVC/Linux build path for native test/release gates;
no generated-cache aliases or upstream source patches are part of this crate.

## Verification

Unit tests cover ring wrapping/atomic overflow, non-finite inputs, frame chunk
invariance/padding, endpoint hysteresis/durations/pause, playback interruption,
resampler reset and attenuation above the destination Nyquist frequency.
Feature compile checks require native build dependencies; actual model/runtime
integration is a separate gate. No test opens a microphone by default.

Endpoint maximum duration is checked at frame boundaries (less than one frame
of resolution); ASR request limits remain separately enforced by WhisperEngine.
