//! CPU graph execution for the v3 Turbo update export.
//!
//! The caller supplies a process-wide ORT environment initialized from a trusted
//! runtime. Preset graph weights are local; no network/Python/cloning is used.
//! Cancellation is observed between graph calls and before emitting a chunk;
//! it does not preempt a native kernel already executing.
use std::{borrow::Cow, collections::BTreeMap, time::Instant};

use ort::{
    environment::Environment,
    session::{
        Session, SessionInputValue, SessionOutputs,
        builder::{GraphOptimizationLevel, PrepackedWeights},
    },
    value::{DynValue, Tensor, TensorRef},
};
use serde::{Deserialize, Serialize};

use crate::{
    CancellationToken, Error, ModelConfig, ModelPack, RepetitionHistory, Result, Sampler,
    SamplingConfig, StateDtype, StateSpec, VoicePreset, checked_elements, invalid,
    max_expected_frames,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisOptions {
    pub sampling: SamplingConfig,
    pub max_new_frames: usize,
    pub chunk_frames: usize,
    pub repetition_window: usize,
    pub frame_cap: bool,
    pub seed: u64,
}

impl Default for SynthesisOptions {
    fn default() -> Self {
        Self {
            sampling: Default::default(),
            max_new_frames: 300,
            chunk_frames: 25,
            repetition_window: 64,
            frame_cap: true,
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Eos,
    FrameLimit,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SynthesisStats {
    pub sample_rate: usize,
    pub generated_frames: usize,
    /// Accepted by the callback, not an acknowledgement of speaker playback.
    pub emitted_samples: usize,
    pub stop_reason: StopReason,
    pub native_streaming: bool,
}

struct Cache {
    keys: Vec<DynValue>,
    values: Vec<DynValue>,
}

enum Codec {
    Streaming {
        session: Session,
        specs: Vec<StateSpec>,
    },
    Full(Session),
}

/// A serialized synthesis session. Mutable access makes ownership/concurrency
/// explicit; the application should admit one voice synthesis at a time.
pub struct OnnxEngine<'p> {
    pack: &'p ModelPack,
    prefill: Session,
    decode: Session,
    acoustic: Session,
    codec: Codec,
    backbone_names: Vec<(String, String)>,
    acoustic_names: Vec<(String, String)>,
    // ORT may share eligible prepacked buffers; this is not a guarantee that
    // all graph weights are deduplicated. Actual memory must be profiled.
    _prepacked: PrepackedWeights,
}

impl<'p> OnnxEngine<'p> {
    pub fn load(pack: &'p ModelPack, environment: &Environment, threads: usize) -> Result<Self> {
        if !(1..=8).contains(&threads) {
            return Err(invalid("CPU threads must be 1..=8"));
        }
        let prepacked = PrepackedWeights::new();
        let load = |path: std::path::PathBuf| -> Result<Session> {
            Session::builder(environment)
                .map_err(runtime)?
                .with_intra_threads(threads)
                .map_err(runtime)?
                .with_inter_threads(1)
                .map_err(runtime)?
                .with_parallel_execution(false)
                .map_err(runtime)?
                .with_intra_op_spinning(false)
                .map_err(runtime)?
                .with_inter_op_spinning(false)
                .map_err(runtime)?
                .with_optimization_level(GraphOptimizationLevel::All)
                .map_err(runtime)?
                .with_prepacked_weights(&prepacked)
                .map_err(runtime)?
                .commit_from_file(path)
                .map_err(runtime)
        };
        let prefill = load(pack.model_file("vieneu_prefill.onnx")?)?;
        let decode = load(pack.model_file("vieneu_decode_step.onnx")?)?;
        let acoustic = load(pack.model_file("vieneu_acoustic_cached.onnx")?)?;
        // Do not eagerly load both full and streaming codec graphs. They can
        // duplicate native model state. Cloning encoder/denoiser stay unloaded.
        let codec = if let Some(layout) = &pack.stream {
            Codec::Streaming {
                session: load(pack.codec_file("moss_audio_tokenizer_decode_step.onnx")?)?,
                specs: layout.specs(),
            }
        } else {
            Codec::Full(load(
                pack.codec_file("moss_audio_tokenizer_decode_full.onnx")?,
            )?)
        };
        Ok(Self {
            pack,
            prefill,
            decode,
            acoustic,
            codec,
            backbone_names: cache_names(pack.config.num_hidden_layers),
            acoustic_names: cache_names(pack.config.local_num_hidden_layers),
            _prepacked: prepacked,
        })
    }

    pub fn config(&self) -> &ModelConfig {
        &self.pack.config
    }

    pub fn synthesize_phonemes<F>(
        &mut self,
        phonemes: &str,
        voice: &VoicePreset,
        options: &SynthesisOptions,
        cancel: &CancellationToken,
        mut on_chunk: F,
    ) -> Result<SynthesisStats>
    where
        F: FnMut(&[f32]) -> Result<()>,
    {
        cancel.check()?;
        options.sampling.validate()?;
        let cfg = &self.pack.config;
        let limits = &self.pack.limits;
        if phonemes.trim().is_empty() || phonemes.len() > limits.max_text_bytes {
            return Err(Error::Limit("empty/oversized phoneme input".into()));
        }
        if options.max_new_frames == 0
            || options.max_new_frames > limits.max_new_frames
            || options.chunk_frames == 0
            || options.chunk_frames > limits.max_chunk_frames
            || options.repetition_window > 4096
        {
            return Err(Error::Limit("synthesis options".into()));
        }
        let frame_limit = if options.frame_cap {
            options.max_new_frames.min(max_expected_frames(phonemes))
        } else {
            options.max_new_frames
        };
        let prompt = self.pack.prompt(phonemes, voice)?;
        cfg.check_backbone_state(
            prompt
                .row_count()
                .checked_add(frame_limit)
                .ok_or_else(|| Error::Limit("context size".into()))?,
            limits,
        )?;
        let anchor = self
            .pack
            .heads
            .speaker_anchor(voice.speaker_embedding.as_deref())?;
        let prompt_embeddings =
            self.pack
                .heads
                .embed_prompt(&prompt, cfg.audio_pad_token_id, anchor.as_deref())?;
        let outputs=self.prefill.run(ort::inputs!{
            "inputs_embeds"=>TensorRef::from_array_view(([1,prompt.row_count(),cfg.hidden_size],prompt_embeddings.as_slice())).map_err(runtime)?
        }).map_err(runtime)?;
        let (prefill_hidden, mut backbone) = split_cache(
            outputs,
            cfg.num_hidden_layers,
            cfg.num_key_value_heads,
            cfg.head_size(),
            prompt.row_count(),
            limits.max_state_bytes,
        )?;
        let mut hidden = hidden_row(&prefill_hidden, cfg.hidden_size, None)?;
        drop(prefill_hidden);
        drop(prompt_embeddings);
        cancel.check()?;
        let mut sampler = Sampler::new(cfg.audio_vocab_size, options.seed)?;
        let mut history =
            RepetitionHistory::new(cfg.n_vq, cfg.audio_vocab_size, options.repetition_window)?;
        let mut audio_logits = vec![0.0; cfg.audio_vocab_size];
        let mut text_logits = vec![0.0; cfg.text_vocab_size];
        let mut embeddings = vec![0.0; cfg.hidden_size];
        let mut row = vec![cfg.audio_pad_token_id; cfg.n_vq + 1];
        row[0] = cfg.speech_generation_start_token_id;
        let native_streaming = matches!(self.codec, Codec::Streaming { .. });
        let buffer_cap = if native_streaming {
            options.chunk_frames
        } else {
            frame_limit
        };
        let mut buffered = Vec::<i32>::with_capacity(checked_elements(&[buffer_cap, cfg.n_vq])?);
        let mut states = match &self.codec {
            Codec::Streaming { specs, .. } => initial_codec_state(specs)?,
            Codec::Full(_) => BTreeMap::new(),
        };
        let mut emitted = 0usize;
        let mut first_audio = None::<Instant>;
        let mut generated = 0usize;
        let mut stopped = StopReason::FrameLimit;
        for frame in 0..frame_limit {
            cancel.check()?;
            let (codes, eos) = acoustic_frame(
                &mut self.acoustic,
                &self.acoustic_names,
                &self.pack.heads,
                cfg,
                limits.max_state_bytes,
                &hidden,
                &mut sampler,
                &mut history,
                options.sampling,
                &mut audio_logits,
                &mut text_logits,
                cancel,
            )?;
            buffered.extend(codes.iter().map(|c| *c as i32));
            generated += 1;
            if !eos {
                row[1..].copy_from_slice(&codes);
                self.pack.heads.embed_rows(
                    &row,
                    cfg.audio_pad_token_id,
                    anchor.as_deref(),
                    &mut embeddings,
                )?;
                let position = [(prompt.row_count() + frame) as i64];
                let mut inputs = ort::inputs! {
                    "inputs_embeds"=>TensorRef::from_array_view(([1,1,cfg.hidden_size],embeddings.as_slice())).map_err(runtime)?,
                    "position_ids"=>TensorRef::from_array_view(([1,1],position.as_slice())).map_err(runtime)?
                };
                append_cache(&mut inputs, &self.backbone_names, &backbone);
                let outputs = self.decode.run(inputs).map_err(runtime)?;
                let (h, next) = split_cache(
                    outputs,
                    cfg.num_hidden_layers,
                    cfg.num_key_value_heads,
                    cfg.head_size(),
                    prompt.row_count() + frame + 1,
                    limits.max_state_bytes,
                )?;
                hidden = hidden_row(&h, cfg.hidden_size, Some(0))?;
                backbone = next;
            }
            let lead = first_audio.map_or(0.0, |t| {
                emitted as f64 / 48_000.0 - t.elapsed().as_secs_f64()
            });
            let target = chunk_target(options.chunk_frames, first_audio.is_some(), lead);
            if native_streaming && (buffered.len() / cfg.n_vq >= target || eos) {
                cancel.check()?;
                let chunk = decode_codec(
                    &mut self.codec,
                    &buffered,
                    cfg.n_vq,
                    &mut states,
                    limits.max_pcm_samples,
                )?;
                emit(
                    &chunk,
                    cancel,
                    &mut on_chunk,
                    &mut emitted,
                    limits.max_pcm_samples,
                )?;
                if !chunk.is_empty() && first_audio.is_none() {
                    first_audio = Some(Instant::now());
                }
                buffered.clear();
            }
            if eos {
                stopped = StopReason::Eos;
                break;
            }
        }
        if !buffered.is_empty() {
            cancel.check()?;
            let chunk = decode_codec(
                &mut self.codec,
                &buffered,
                cfg.n_vq,
                &mut states,
                limits.max_pcm_samples,
            )?;
            emit(
                &chunk,
                cancel,
                &mut on_chunk,
                &mut emitted,
                limits.max_pcm_samples,
            )?;
        }
        cancel.check()?;
        Ok(SynthesisStats {
            sample_rate: 48_000,
            generated_frames: generated,
            emitted_samples: emitted,
            stop_reason: stopped,
            native_streaming,
        })
    }
}

fn runtime(e: impl std::fmt::Display) -> Error {
    Error::Runtime(e.to_string())
}

fn cache_names(layers: usize) -> Vec<(String, String)> {
    (0..layers)
        .map(|i| (format!("past_k_{i}"), format!("past_v_{i}")))
        .collect()
}

fn append_cache<'a>(
    inputs: &mut Vec<(Cow<'a, str>, SessionInputValue<'a>)>,
    names: &'a [(String, String)],
    cache: &'a Cache,
) {
    for (i, (key, value)) in names.iter().enumerate() {
        inputs.push((Cow::Borrowed(key), (&cache.keys[i]).into()));
        inputs.push((Cow::Borrowed(value), (&cache.values[i]).into()));
    }
}

fn split_cache(
    outputs: SessionOutputs<'_>,
    layers: usize,
    heads: usize,
    head_dim: usize,
    positions: usize,
    max_bytes: usize,
) -> Result<(DynValue, Cache)> {
    if outputs.len() != 1 + 2 * layers {
        return Err(invalid("graph cache output count"));
    }
    let mut outputs = outputs.into_iter().map(|(_, v)| v);
    let hidden = outputs
        .next()
        .ok_or_else(|| invalid("missing hidden output"))?;
    let mut keys = Vec::with_capacity(layers);
    let mut values = Vec::with_capacity(layers);
    let mut bytes = 0usize;
    for i in 0..2 * layers {
        let value = outputs
            .next()
            .ok_or_else(|| invalid("missing cache tensor"))?;
        let (shape, data) = value.try_extract_tensor::<f32>().map_err(runtime)?;
        // Shape/byte admission is O(layers). Re-scanning the entire growing KV
        // buffer at every step would add quadratic work. We validate finite
        // hidden/head/PCM boundaries; graph artifacts remain trusted inputs.
        if shape.as_ref() != [1, heads as i64, positions as i64, head_dim as i64] {
            return Err(invalid("graph cache shape/value"));
        }
        bytes = bytes
            .checked_add(
                data.len()
                    .checked_mul(4)
                    .ok_or_else(|| Error::Limit("cache bytes".into()))?,
            )
            .ok_or_else(|| Error::Limit("cache bytes".into()))?;
        if bytes > max_bytes {
            return Err(Error::Limit("returned KV cache budget".into()));
        }
        if i < layers {
            keys.push(value);
        } else {
            values.push(value);
        }
    }
    Ok((hidden, Cache { keys, values }))
}

fn hidden_row(value: &DynValue, hidden: usize, row: Option<usize>) -> Result<Vec<f32>> {
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(runtime)?;
    if shape.len() != 3 || shape[0] != 1 || shape[1] < 1 || shape[2] != hidden as i64 {
        return Err(invalid("graph hidden shape"));
    }
    let row = row.unwrap_or(shape[1] as usize - 1);
    if row >= shape[1] as usize {
        return Err(invalid("graph hidden row"));
    }
    let result = data
        .get(row * hidden..(row + 1) * hidden)
        .ok_or_else(|| invalid("graph hidden length"))?;
    if result.iter().any(|v| !v.is_finite()) {
        return Err(invalid("graph hidden nonfinite"));
    }
    Ok(result.to_vec())
}

#[allow(clippy::too_many_arguments)]
fn acoustic_frame(
    session: &mut Session,
    names: &[(String, String)],
    heads: &crate::Heads,
    cfg: &ModelConfig,
    max_state_bytes: usize,
    hidden: &[f32],
    sampler: &mut Sampler,
    history: &mut RepetitionHistory,
    sampling: SamplingConfig,
    audio_logits: &mut [f32],
    text_logits: &mut [f32],
    cancel: &CancellationToken,
) -> Result<(Vec<i64>, bool)> {
    let mut tokens = Vec::with_capacity(2 * cfg.hidden_size);
    tokens.extend_from_slice(hidden);
    tokens.extend_from_slice(heads.text_embedding(cfg.speech_generation_start_token_id)?);
    let position = [0i64, 1];
    let empty = Tensor::<f32>::from_array((
        [1, cfg.local_num_attention_heads, 0, cfg.local_head_size()],
        Vec::<f32>::new(),
    ))
    .map_err(runtime)?
    .into_dyn();
    let mut inputs = ort::inputs! {
        "token_emb"=>TensorRef::from_array_view(([1,2,cfg.hidden_size],tokens.as_slice())).map_err(runtime)?,
        "position_ids"=>TensorRef::from_array_view(([1,2],position.as_slice())).map_err(runtime)?
    };
    for (key, value) in names {
        inputs.push((Cow::Borrowed(key), (&empty).into()));
        inputs.push((Cow::Borrowed(value), (&empty).into()));
    }
    let outputs = session.run(inputs).map_err(runtime)?;
    let (h, mut cache) = split_cache(
        outputs,
        cfg.local_num_hidden_layers,
        cfg.local_num_attention_heads,
        cfg.local_head_size(),
        2,
        max_state_bytes,
    )?;
    let slot0 = hidden_row(&h, cfg.hidden_size, Some(0))?;
    let mut code_hidden = hidden_row(&h, cfg.hidden_size, Some(1))?;
    let mut codes = Vec::with_capacity(cfg.n_vq);
    for channel in 0..cfg.n_vq {
        cancel.check()?;
        if channel > 0 {
            let embedding = heads.audio_embedding(channel - 1, codes[channel - 1])?;
            let position = [(channel + 1) as i64];
            let mut inputs = ort::inputs! {
                "token_emb"=>TensorRef::from_array_view(([1,1,cfg.hidden_size],embedding)).map_err(runtime)?,
                "position_ids"=>TensorRef::from_array_view(([1,1],position.as_slice())).map_err(runtime)?
            };
            append_cache(&mut inputs, names, &cache);
            let outputs = session.run(inputs).map_err(runtime)?;
            let (h, next) = split_cache(
                outputs,
                cfg.local_num_hidden_layers,
                cfg.local_num_attention_heads,
                cfg.local_head_size(),
                channel + 2,
                max_state_bytes,
            )?;
            code_hidden = hidden_row(&h, cfg.hidden_size, Some(0))?;
            cache = next;
        }
        heads.audio_logits(channel, &code_hidden, audio_logits)?;
        let code = sampler.sample(audio_logits, sampling, Some(history.channel(channel)?))?;
        history.add(channel, code)?;
        codes.push(code as i64);
    }
    heads.text_logits(&slot0, text_logits)?;
    let eos = text_logits
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1).then_with(|| b.0.cmp(&a.0)))
        .is_some_and(|(id, _)| id as i64 == cfg.speech_generation_end_token_id);
    Ok((codes, eos))
}

fn initial_codec_state(specs: &[StateSpec]) -> Result<BTreeMap<String, DynValue>> {
    let mut result = BTreeMap::new();
    for s in specs {
        let n = checked_elements(&s.shape)?;
        let value = match s.dtype {
            StateDtype::Float32 => Tensor::from_array((s.shape.clone(), vec![0.0f32; n]))
                .map_err(runtime)?
                .into_dyn(),
            StateDtype::Int32 => Tensor::from_array((s.shape.clone(), vec![s.initial_i32; n]))
                .map_err(runtime)?
                .into_dyn(),
        };
        result.insert(s.input_name.clone(), value);
    }
    Ok(result)
}

fn decode_codec(
    codec: &mut Codec,
    codes: &[i32],
    channels: usize,
    states: &mut BTreeMap<String, DynValue>,
    max_samples: usize,
) -> Result<Vec<f32>> {
    if codes.is_empty() || !codes.len().is_multiple_of(channels) {
        return Err(invalid("codec frame shape"));
    }
    let frames = codes.len() / channels;
    let lengths = [frames as i32];
    let mut inputs = ort::inputs! {
        "audio_codes"=>TensorRef::from_array_view(([1,frames,channels],codes)).map_err(runtime)?,
        "audio_code_lengths"=>TensorRef::from_array_view(([1],lengths.as_slice())).map_err(runtime)?
    };
    match codec {
        Codec::Streaming { session, specs } => {
            for (name, value) in states.iter() {
                inputs.push((Cow::Borrowed(name), value.into()));
            }
            let mut outputs = session
                .run(inputs)
                .map_err(runtime)?
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect::<BTreeMap<_, _>>();
            let audio = outputs
                .remove("audio")
                .ok_or_else(|| invalid("codec audio output missing"))?;
            let length = outputs
                .remove("audio_lengths")
                .ok_or_else(|| invalid("codec lengths missing"))?;
            let (_, length) = length.try_extract_tensor::<i32>().map_err(runtime)?;
            if length.len() != 1 || length[0] < 0 {
                return Err(invalid("codec audio length"));
            }
            let mono = mono_audio(&audio, Some(length[0] as usize), max_samples)?;
            let mut next = BTreeMap::new();
            for spec in specs {
                let value = outputs
                    .remove(&spec.output_name)
                    .ok_or_else(|| invalid("codec state output missing"))?;
                let shape = match spec.dtype {
                    StateDtype::Float32 => value.try_extract_tensor::<f32>().map_err(runtime)?.0,
                    StateDtype::Int32 => value.try_extract_tensor::<i32>().map_err(runtime)?.0,
                };
                if shape.len() != spec.shape.len()
                    || shape.iter().zip(&spec.shape).any(|(a, b)| *a != *b as i64)
                {
                    return Err(invalid("codec state changed shape"));
                }
                next.insert(spec.input_name.clone(), value);
            }
            *states = next;
            Ok(mono)
        }
        Codec::Full(session) => {
            let outputs = session.run(inputs).map_err(runtime)?;
            let (_, audio) = outputs
                .into_iter()
                .next()
                .ok_or_else(|| invalid("codec audio missing"))?;
            mono_audio(&audio, None, max_samples)
        }
    }
}

fn mono_audio(value: &DynValue, length: Option<usize>, max_samples: usize) -> Result<Vec<f32>> {
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(runtime)?;
    if shape.len() != 3 || shape[0] != 1 || !(1..=8).contains(&shape[1]) || shape[2] < 0 {
        return Err(invalid("codec waveform shape"));
    }
    let available = shape[2] as usize;
    let n = length.unwrap_or(available);
    if n > available || n > max_samples {
        return Err(Error::Limit("codec PCM length".into()));
    }
    let channels = shape[1] as usize;
    let mut result = vec![0.0; n];
    for channel in 0..channels {
        let chunk = data
            .get(channel * available..channel * available + n)
            .ok_or_else(|| invalid("codec PCM shape/length"))?;
        for (out, sample) in result.iter_mut().zip(chunk) {
            *out += sample / channels as f32;
        }
    }
    if result.iter().any(|v| !v.is_finite()) {
        return Err(invalid("nonfinite PCM"));
    }
    Ok(result)
}

fn emit<F: FnMut(&[f32]) -> Result<()>>(
    chunk: &[f32],
    cancel: &CancellationToken,
    on_chunk: &mut F,
    emitted: &mut usize,
    max_samples: usize,
) -> Result<()> {
    cancel.check()?;
    let count = emitted
        .checked_add(chunk.len())
        .ok_or_else(|| Error::Limit("PCM count".into()))?;
    if count > max_samples {
        return Err(Error::Limit("total PCM budget".into()));
    }
    if !chunk.is_empty() {
        on_chunk(chunk)?;
        *emitted = count;
    }
    cancel.check()
}

fn chunk_target(cap: usize, has_started: bool, lead: f64) -> usize {
    if !has_started || lead < 0.20 {
        cap.min(4)
    } else if lead < 0.55 {
        cap.min(6)
    } else if lead < 1.10 {
        cap.min(8)
    } else {
        cap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_target_and_cancellation_before_emit() {
        assert_eq!(chunk_target(25, false, 100.0), 4);
        assert_eq!(chunk_target(25, true, 0.6), 8);
        assert_eq!(chunk_target(2, true, 2.0), 2);
        let cancel = CancellationToken::default();
        cancel.cancel();
        let mut count = 0;
        assert!(
            emit(
                &[1.0],
                &cancel,
                &mut |_| panic!("must not emit cancelled audio"),
                &mut count,
                10
            )
            .is_err()
        );
        assert_eq!(count, 0);
    }
}
