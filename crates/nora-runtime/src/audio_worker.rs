//! One isolated native inference job. No API key, UI, microphone, speaker or
//! environment configuration is read here. The launcher must verify isolation
//! before entering this module and kill the child on cancellation/deadline.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(feature = "native-voice")]
use cap_std::{ambient_authority, fs::Dir};
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "native-voice", test))]
use std::path::Component;

use crate::Result;
#[cfg(feature = "native-voice")]
use crate::assets::VoiceAssets;

pub const MAX_REQUEST_BYTES: usize = 32_768;
pub const MAX_METADATA_BYTES: usize = 262_144;
pub const MAX_ASR_SAMPLES: usize = 16_000 * 60;
pub const MAX_TTS_PCM_BYTES: usize = 16_000_000;
pub const MAX_TTS_TEXT_BYTES: usize = 8_192;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerRequest {
    Asr {
        assets_root: PathBuf,
        input_pcm: PathBuf,
        output_json: PathBuf,
    },
    Tts {
        assets_root: PathBuf,
        text: String,
        output_pcm: PathBuf,
        output_json: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerMetadata {
    Asr(AsrMetadata),
    Tts(TtsMetadata),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsrMetadata {
    pub sample_rate: u32,
    pub input_samples: usize,
    pub text: String,
    pub language: String,
    pub segments: Vec<AsrSegment>,
    pub verification_ms: f64,
    pub loaded_ms: f64,
    pub inference_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsStopReason {
    Eos,
    FrameLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TtsMetadata {
    pub sample_rate: u32,
    pub samples: usize,
    pub generated_frames: usize,
    pub stop_reason: TtsStopReason,
    pub native_streaming: bool,
    pub voice: String,
    pub verification_ms: f64,
    pub loaded_ms: f64,
    /// PCM ready in the writer callback after load; not audible playback.
    pub first_chunk_ms: Option<f64>,
    pub synthesis_ms: f64,
}

pub fn read_worker_metadata(path: &Path) -> Result<WorkerMetadata> {
    let bytes = read_bounded_file(path, MAX_METADATA_BYTES)?;
    let metadata: WorkerMetadata = serde_json::from_slice(&bytes)?;
    metadata.validate()?;
    Ok(metadata)
}

impl WorkerMetadata {
    pub fn validate(&self) -> Result<()> {
        let finite_ms = |value: f64| value.is_finite() && value >= 0.0;
        match self {
            Self::Asr(value) => {
                if value.sample_rate != 16_000
                    || value.input_samples == 0
                    || value.input_samples > MAX_ASR_SAMPLES
                    || value.language != "vi"
                    || value.text.trim().is_empty()
                    || value.text.len() > 65_536
                    || value.segments.is_empty()
                    || value.segments.len() > 4096
                    || ![value.verification_ms, value.loaded_ms, value.inference_ms]
                        .into_iter()
                        .all(finite_ms)
                    || value.segments.iter().any(|segment| {
                        segment.end_ms < segment.start_ms || segment.end_ms > 180_000
                    })
                {
                    return Err("invalid ASR worker metadata".into());
                }
                let mut joined = String::new();
                for segment in &value.segments {
                    if joined
                        .len()
                        .checked_add(segment.text.len())
                        .is_none_or(|n| n > 65_536)
                    {
                        return Err("ASR metadata text exceeds limit".into());
                    }
                    joined.push_str(&segment.text);
                }
                if joined != value.text {
                    return Err("ASR metadata text is inconsistent with its segments".into());
                }
            }
            Self::Tts(value) => {
                if value.sample_rate != 48_000
                    || value.samples == 0
                    || value.samples > MAX_TTS_PCM_BYTES / 4
                    || value.generated_frames == 0
                    || value.generated_frames > 300
                    || value.voice.is_empty()
                    || value.voice.len() > 1024
                    || ![value.verification_ms, value.loaded_ms, value.synthesis_ms]
                        .into_iter()
                        .all(finite_ms)
                    || value
                        .first_chunk_ms
                        .is_none_or(|first| !finite_ms(first) || first > value.synthesis_ms)
                {
                    return Err("invalid TTS worker metadata".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(feature = "native-voice"))]
pub fn run_audio_worker(_request_path: &Path) -> Result<()> {
    Err("native voice support is not included in this build".into())
}

#[cfg(feature = "native-voice")]
pub fn run_audio_worker(request_path: &Path) -> Result<()> {
    use std::{io::BufWriter, time::Instant};
    let bytes = read_bounded_file(request_path, MAX_REQUEST_BYTES)?;
    let request: WorkerRequest = serde_json::from_slice(&bytes)?;
    let scratch = request_path
        .parent()
        .ok_or("worker request has no scratch directory")?
        .canonicalize()?;
    let directory = Dir::open_ambient_dir(&scratch, ambient_authority())?;
    let request_name = request_path
        .file_name()
        .ok_or("invalid worker request path")?;
    let (name, metadata) = match request {
        WorkerRequest::Asr {
            assets_root,
            input_pcm,
            output_json,
        } => {
            let input_name = scratch_name(&scratch, &input_pcm)?;
            let output_name = scratch_name(&scratch, &output_json)?;
            distinct_names(&[
                input_name.as_os_str(),
                output_name.as_os_str(),
                request_name,
            ])?;
            let pcm = read_pcm(directory.open(&input_name)?, MAX_ASR_SAMPLES)?;
            let assets = VoiceAssets::new(assets_root.canonicalize()?);
            let begin = Instant::now();
            assets.verify_asr()?;
            let verification_ms = begin.elapsed().as_secs_f64() * 1000.0;
            let loading = Instant::now();
            let mut engine = nora_audio::WhisperEngine::load(assets.asr(), MAX_ASR_SAMPLES, 2)?;
            let loaded_ms = loading.elapsed().as_secs_f64() * 1000.0;
            let inference = Instant::now();
            let transcript = engine.transcribe_16khz(&pcm, "vi")?;
            let inference_ms = inference.elapsed().as_secs_f64() * 1000.0;
            if transcript.text.trim().is_empty() {
                return Err("ASR returned no speech text".into());
            }
            let metadata = AsrMetadata {
                sample_rate: 16_000,
                input_samples: pcm.len(),
                text: transcript.text,
                language: transcript.language,
                segments: transcript
                    .segments
                    .into_iter()
                    .map(|s| AsrSegment {
                        start_ms: s.start_ms,
                        end_ms: s.end_ms,
                        text: s.text,
                    })
                    .collect(),
                verification_ms,
                loaded_ms,
                inference_ms,
            };
            (output_name, WorkerMetadata::Asr(metadata))
        }
        WorkerRequest::Tts {
            assets_root,
            text,
            output_pcm,
            output_json,
        } => {
            use nora_vieneu::{
                CancellationToken, Limits, ModelPack,
                g2p::VietnameseFrontend,
                onnx::{OnnxEngine, StopReason, SynthesisOptions},
            };
            if text.trim().is_empty() || text.len() > MAX_TTS_TEXT_BYTES {
                return Err("TTS text is empty or exceeds its limit".into());
            }
            let pcm_name = scratch_name(&scratch, &output_pcm)?;
            let output_name = scratch_name(&scratch, &output_json)?;
            distinct_names(&[pcm_name.as_os_str(), output_name.as_os_str(), request_name])?;
            let assets = VoiceAssets::new(assets_root.canonicalize()?);
            let begin = Instant::now();
            assets.verify_tts()?;
            let verification_ms = begin.elapsed().as_secs_f64() * 1000.0;
            let loading = Instant::now();
            nora_vieneu::configure_single_user_runtime();
            let environment = model_environment(&assets.ort())?;
            let pack = ModelPack::open(assets.tts(), assets.codec(), Limits::default())?;
            let (voice_name, voice) =
                pack.load_voice_catalog(Path::new("voices_v3_turbo.json"), None)?;
            let frontend = VietnameseFrontend::load(&assets.g2p(), pack.limits().max_text_bytes)?;
            let phonemes = frontend.phonemize(&text)?;
            let mut engine = OnnxEngine::load(&pack, &environment, 2)?;
            let loaded_ms = loading.elapsed().as_secs_f64() * 1000.0;
            let mut options = cap_std::fs::OpenOptions::new();
            options.create_new(true).write(true);
            let output = directory.open_with(&pcm_name, &options)?;
            let mut writer = BufWriter::with_capacity(65_536, output);
            let mut written_bytes = 0usize;
            let mut first_chunk_ms = None;
            let synthesis = Instant::now();
            let stats = engine.synthesize_phonemes(
                &phonemes,
                &voice,
                &SynthesisOptions::default(),
                &CancellationToken::default(),
                |chunk| {
                    let bytes = chunk
                        .len()
                        .checked_mul(4)
                        .ok_or_else(|| nora_vieneu::Error::Limit("PCM size overflow".into()))?;
                    let next = written_bytes
                        .checked_add(bytes)
                        .ok_or_else(|| nora_vieneu::Error::Limit("PCM size overflow".into()))?;
                    if next > MAX_TTS_PCM_BYTES || chunk.iter().any(|v| !v.is_finite()) {
                        return Err(nora_vieneu::Error::Limit(
                            "PCM output exceeded bound or was non-finite".into(),
                        ));
                    }
                    first_chunk_ms
                        .get_or_insert_with(|| synthesis.elapsed().as_secs_f64() * 1000.0);
                    for sample in chunk {
                        writer.write_all(&sample.to_le_bytes()).map_err(|_| {
                            nora_vieneu::Error::Runtime("cannot write PCM output".into())
                        })?;
                    }
                    written_bytes = next;
                    Ok(())
                },
            )?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
            let synthesis_ms = synthesis.elapsed().as_secs_f64() * 1000.0;
            if written_bytes == 0 || written_bytes / 4 != stats.emitted_samples {
                return Err("TTS emitted no audio or inconsistent sample count".into());
            }
            let metadata = TtsMetadata {
                sample_rate: 48_000,
                samples: stats.emitted_samples,
                generated_frames: stats.generated_frames,
                stop_reason: match stats.stop_reason {
                    StopReason::Eos => TtsStopReason::Eos,
                    StopReason::FrameLimit => TtsStopReason::FrameLimit,
                },
                native_streaming: stats.native_streaming,
                voice: voice_name,
                verification_ms,
                loaded_ms,
                first_chunk_ms,
                synthesis_ms,
            };
            (output_name, WorkerMetadata::Tts(metadata))
        }
    };
    metadata.validate()?;
    let encoded = serde_json::to_vec(&metadata)?;
    if encoded.len() > MAX_METADATA_BYTES {
        return Err("worker metadata exceeds limit".into());
    }
    let mut options = cap_std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    let mut output = directory.open_with(name, &options)?;
    output.write_all(&encoded)?;
    output.sync_all()?;
    Ok(())
}

#[cfg(feature = "native-voice")]
pub(crate) fn model_environment(dll: &Path) -> Result<ort::environment::Environment> {
    // init_from must precede even GlobalThreadPoolOptions::default().
    let environment = ort::init_from(dll)?.with_telemetry(false);
    let pool = ort::environment::GlobalThreadPoolOptions::default()
        .with_intra_threads(2)?
        .with_inter_threads(1)?
        .with_spin_control(false)?;
    Ok(environment.with_global_thread_pool(pool).build()?)
}

fn read_bounded_file(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > max as u64 {
        return Err("worker file is not bounded regular data".into());
    }
    let mut bytes = Vec::new();
    file.take(max.checked_add(1).ok_or("invalid file limit")? as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err("worker file grew beyond limit".into());
    }
    Ok(bytes)
}

#[cfg(any(feature = "native-voice", test))]
fn scratch_name(scratch: &Path, path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or("worker path has no scratch parent")?
        .canonicalize()?;
    let name = path.file_name().ok_or("worker filename missing")?;
    let text = name.to_str().ok_or("worker filename must be UTF-8")?;
    if parent != scratch
        || text.is_empty()
        || text.len() > 128
        || !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || text.ends_with(['.', ' '])
        || !Path::new(text)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("worker path escaped scratch directory".into());
    }
    let stem = text.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("reserved worker filename".into());
    }
    Ok(PathBuf::from(name))
}

#[cfg(any(feature = "native-voice", test))]
fn distinct_names(names: &[&std::ffi::OsStr]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        if !seen.insert(name.to_string_lossy().to_ascii_lowercase()) {
            return Err("worker paths alias one another".into());
        }
    }
    Ok(())
}

pub fn read_pcm(mut file: impl Read, max_samples: usize) -> Result<Vec<f32>> {
    if max_samples == 0 || max_samples > MAX_TTS_PCM_BYTES / 4 {
        return Err("invalid PCM sample budget".into());
    }
    let mut bytes = [0u8; 65_536];
    let mut pcm = Vec::new();
    loop {
        let mut used = 0;
        while used < bytes.len() {
            let n = file.read(&mut bytes[used..])?;
            if n == 0 {
                break;
            }
            used += n;
        }
        if used == 0 {
            break;
        }
        if !used.is_multiple_of(4) {
            return Err("PCM byte count is not float32 aligned".into());
        }
        if pcm
            .len()
            .checked_add(used / 4)
            .is_none_or(|next| next > max_samples)
        {
            return Err("PCM duration budget exceeded".into());
        }
        pcm.try_reserve_exact(used / 4)?;
        for b in bytes[..used].as_chunks::<4>().0 {
            let value = f32::from_le_bytes(*b);
            if !value.is_finite() {
                return Err("non-finite PCM sample".into());
            }
            pcm.push(value);
        }
    }
    if pcm.is_empty() {
        return Err("empty PCM input".into());
    }
    Ok(pcm)
}

pub fn write_pcm(path: &Path, pcm: &[f32]) -> Result<()> {
    if pcm.is_empty() || pcm.len() > MAX_TTS_PCM_BYTES / 4 || pcm.iter().any(|v| !v.is_finite()) {
        return Err("invalid PCM output".into());
    }
    let mut output = std::io::BufWriter::with_capacity(
        65_536,
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?,
    );
    for sample in pcm {
        output.write_all(&sample.to_le_bytes())?;
    }
    output.flush()?;
    output.get_ref().sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_rejects_partial_nonfinite_empty_and_oversized() {
        assert!(read_pcm(&[0u8; 3][..], 10).is_err());
        assert!(read_pcm(&[][..], 10).is_err());
        assert!(read_pcm(&f32::NAN.to_le_bytes()[..], 10).is_err());
        assert!(read_pcm(&[0u8; 12][..], 2).is_err());
        assert_eq!(read_pcm(&1.0f32.to_le_bytes()[..], 1).unwrap(), vec![1.0]);
    }
    #[test]
    fn scratch_paths_cannot_alias_escape_or_use_devices() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        assert_eq!(
            scratch_name(&root, &root.join("output.pcm")).unwrap(),
            PathBuf::from("output.pcm")
        );
        assert!(scratch_name(&root, &root.join("../outside.pcm")).is_err());
        assert!(scratch_name(&root, &root.join("CON.pcm")).is_err());
        assert!(distinct_names(&["input.pcm".as_ref(), "INPUT.PCM".as_ref()]).is_err());
    }
    #[test]
    fn request_schema_refuses_extra_fields_and_output_is_create_new() {
        let request = r#"{"kind":"tts","assets_root":"x","text":"a","output_pcm":"b","output_json":"c","api_key":"forbidden"}"#;
        assert!(serde_json::from_str::<WorkerRequest>(request).is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.pcm");
        write_pcm(&path, &[0.1, 0.2]).unwrap();
        assert!(write_pcm(&path, &[0.3]).is_err());
        assert_eq!(
            read_pcm(File::open(path).unwrap(), 2).unwrap(),
            vec![0.1, 0.2]
        );
    }

    #[test]
    fn worker_metadata_must_have_consistent_bounded_rates_and_text() {
        let mut value = AsrMetadata {
            sample_rate: 16_000,
            input_samples: 1000,
            text: "Xin chào".into(),
            language: "vi".into(),
            segments: vec![AsrSegment {
                start_ms: 0,
                end_ms: 100,
                text: "Xin chào".into(),
            }],
            verification_ms: 1.0,
            loaded_ms: 2.0,
            inference_ms: 3.0,
        };
        assert!(WorkerMetadata::Asr(value.clone()).validate().is_ok());
        value.text = "Different".into();
        assert!(WorkerMetadata::Asr(value).validate().is_err());
        let value = TtsMetadata {
            sample_rate: 48_000,
            samples: 960,
            generated_frames: 1,
            stop_reason: TtsStopReason::Eos,
            native_streaming: true,
            voice: "Test".into(),
            verification_ms: 1.0,
            loaded_ms: 1.0,
            first_chunk_ms: Some(3.0),
            synthesis_ms: 2.0,
        };
        assert!(WorkerMetadata::Tts(value).validate().is_err());
    }
}
