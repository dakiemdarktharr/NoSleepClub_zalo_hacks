//! Explicit PTT capture and bounded native output. Recording does not end on an
//! internal pause: VAD only trims the beginning/end when PTT is released.
//! No acoustic echo cancellation, speaker authentication or physical audibility
//! guarantee is implied. The supervisor serializes recording/playback/models.

use serde::Serialize;

pub const MAX_RECORD_SECONDS: u32 = 60;
pub const MAX_CAPTURE_RING_SAMPLES: usize = 384_000;
pub const OUTPUT_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Serialize)]
pub struct RecordingProgress {
    pub elapsed_ms: u64,
    pub captured_samples_16khz: usize,
    pub speech_detected: bool,
    pub limit_reached: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingMetrics {
    pub source_sample_rate: u32,
    pub source_channels: u16,
    pub raw_input_samples: u64,
    pub resampled_samples: usize,
    pub trimmed_samples: usize,
    pub speech_frames: u64,
    pub recording_ms: u64,
}

pub struct Recording {
    pub pcm_16khz: Vec<f32>,
    pub metrics: RecordingMetrics,
}

#[cfg(any(feature = "native-voice", test))]
mod processing {
    use crate::Result;
    use nora_audio::MonoResampler;

    pub(crate) struct CaptureConverter {
        resampler: MonoResampler,
        input: Vec<f32>,
        used: usize,
        output: Vec<f32>,
        skip: usize,
        input_rate: u32,
        total_input: u64,
        max_input: u64,
        max_output: usize,
        emitted: usize,
    }

    impl CaptureConverter {
        pub(crate) fn new(rate: u32, max_seconds: u32) -> Result<Self> {
            if !(1..=super::MAX_RECORD_SECONDS).contains(&max_seconds) || !rate.is_multiple_of(50) {
                return Err("unsupported capture duration/rate".into());
            }
            let chunk = rate as usize / 50;
            let resampler = MonoResampler::new(rate, 16_000, chunk)?;
            let output_max = resampler.output_samples_max();
            let skip = resampler.output_delay_samples();
            Ok(Self {
                resampler,
                input: vec![0.0; chunk],
                used: 0,
                output: vec![0.0; output_max],
                skip,
                input_rate: rate,
                total_input: 0,
                max_input: u64::from(rate) * u64::from(max_seconds),
                max_output: 16_000 * max_seconds as usize,
                emitted: 0,
            })
        }
        pub(crate) fn input_samples(&self) -> u64 {
            self.total_input
        }
        pub(crate) fn emitted_samples(&self) -> usize {
            self.emitted
        }

        pub(crate) fn push(
            &mut self,
            mut samples: &[f32],
            mut emit: impl FnMut(&[f32]) -> Result<()>,
        ) -> Result<()> {
            if samples.iter().any(|v| !v.is_finite()) {
                return Err("non-finite microphone samples".into());
            }
            self.total_input = self
                .total_input
                .checked_add(samples.len() as u64)
                .ok_or("capture sample overflow")?;
            if self.total_input > self.max_input {
                return Err("recording duration limit reached; utterance discarded".into());
            }
            while !samples.is_empty() {
                let n = (self.input.len() - self.used).min(samples.len());
                self.input[self.used..self.used + n].copy_from_slice(&samples[..n]);
                self.used += n;
                samples = &samples[n..];
                if self.used == self.input.len() {
                    self.convert(&mut emit, self.max_output)?;
                    self.used = 0;
                }
            }
            Ok(())
        }
        fn convert(
            &mut self,
            emit: &mut impl FnMut(&[f32]) -> Result<()>,
            target: usize,
        ) -> Result<()> {
            let n = self.resampler.process_into(&self.input, &mut self.output)?;
            let skip = self.skip.min(n);
            self.skip -= skip;
            let available = n - skip;
            let count = available.min(target.saturating_sub(self.emitted));
            if count > 0 {
                emit(&self.output[skip..skip + count])?;
                self.emitted += count;
            }
            Ok(())
        }
        pub(crate) fn finish(&mut self, mut emit: impl FnMut(&[f32]) -> Result<()>) -> Result<()> {
            let target = self
                .total_input
                .checked_mul(16_000)
                .ok_or("capture resampling overflow")?
                .div_ceil(u64::from(self.input_rate)) as usize;
            if target > self.max_output {
                return Err("recording duration limit reached".into());
            }
            if self.used > 0 {
                self.input[self.used..].fill(0.0);
                self.convert(&mut emit, target)?;
                self.used = 0;
            }
            self.input.fill(0.0);
            for _ in 0..64 {
                if self.emitted >= target {
                    break;
                }
                self.convert(&mut emit, target)?;
            }
            if self.emitted != target {
                return Err("resampler did not flush complete recording".into());
            }
            Ok(())
        }
        pub(crate) fn clear(&mut self) {
            self.input.fill(0.0);
            self.output.fill(0.0);
            self.used = 0;
            self.resampler.reset();
        }
    }

    #[derive(Default)]
    pub(crate) struct VadTrim {
        pub(crate) frames: u64,
        pub(crate) speech_frames: u64,
        consecutive: u64,
        first: Option<usize>,
        last_end: usize,
    }
    impl VadTrim {
        pub(crate) fn observe(&mut self, p: f32) -> Result<()> {
            if !p.is_finite() || !(0.0..=1.0).contains(&p) {
                return Err("invalid speech probability".into());
            }
            let start = (self.frames as usize)
                .checked_mul(512)
                .ok_or("VAD position overflow")?;
            self.frames += 1;
            if p >= 0.5 {
                self.consecutive += 1;
            } else {
                self.consecutive = 0;
            }
            if self.first.is_none() && self.consecutive >= 3 {
                self.first = Some(start.saturating_sub(1024));
            }
            if p >= 0.35 {
                self.speech_frames += 1;
                self.last_end = start + 512;
            }
            Ok(())
        }
        pub(crate) fn speech_detected(&self) -> bool {
            self.first.is_some()
        }
        pub(crate) fn bounds(&self, len: usize) -> Result<(usize, usize)> {
            let first = self
                .first
                .ok_or("no speech detected; recording discarded")?;
            let start = first.saturating_sub(2560);
            let end = self.last_end.saturating_add(2560).min(len);
            if start >= end {
                return Err("recording contains no valid speech range".into());
            }
            Ok((start, end))
        }
    }

    /// Lazily resamples only the device callback's next bounded chunk. Mono PCM
    /// is never expanded into a full multichannel/high-rate allocation.
    pub(crate) struct PlaybackSource {
        pcm: Vec<f32>,
        cursor: usize,
        resampler: MonoResampler,
        input: Vec<f32>,
        output: Vec<f32>,
        output_cursor: usize,
        output_len: usize,
        skip: usize,
        remaining: usize,
    }
    impl PlaybackSource {
        pub(crate) fn new(pcm: Vec<f32>, rate: u32) -> Result<Self> {
            if pcm.is_empty()
                || pcm.len() > crate::audio_worker::MAX_TTS_PCM_BYTES / 4
                || pcm.iter().any(|v| !v.is_finite())
            {
                return Err("invalid speaker PCM".into());
            }
            let resampler = MonoResampler::new(48_000, rate, 960)?;
            let remaining = (pcm.len() as u64)
                .checked_mul(u64::from(rate))
                .ok_or("playback duration overflow")?
                .div_ceil(48_000) as usize;
            let skip = resampler.output_delay_samples();
            let output_max = resampler.output_samples_max();
            Ok(Self {
                pcm,
                cursor: 0,
                resampler,
                input: vec![0.0; 960],
                output: vec![0.0; output_max],
                output_cursor: 0,
                output_len: 0,
                skip,
                remaining,
            })
        }
        pub(crate) fn next_sample(&mut self) -> Result<Option<f32>> {
            if self.remaining == 0 {
                return Ok(None);
            }
            let mut fills = 0;
            while self.output_cursor >= self.output_len {
                fills += 1;
                if fills > 64 {
                    return Err("playback resampler produced no frames".into());
                }
                self.input.fill(0.0);
                let n = (self.pcm.len() - self.cursor).min(self.input.len());
                self.input[..n].copy_from_slice(&self.pcm[self.cursor..self.cursor + n]);
                self.cursor += n;
                self.output_len = self.resampler.process_into(&self.input, &mut self.output)?;
                self.output_cursor = self.skip.min(self.output_len);
                self.skip -= self.output_cursor;
            }
            let sample = self.output[self.output_cursor];
            self.output_cursor += 1;
            self.remaining -= 1;
            Ok(Some(sample))
        }
    }
    impl Drop for PlaybackSource {
        fn drop(&mut self) {
            self.pcm.fill(0.0);
            self.input.fill(0.0);
            self.output.fill(0.0);
        }
    }
}

#[cfg(feature = "native-voice")]
mod native {
    use super::{
        MAX_CAPTURE_RING_SAMPLES, MAX_RECORD_SECONDS, Recording, RecordingMetrics,
        RecordingProgress,
        processing::{CaptureConverter, PlaybackSource, VadTrim},
    };
    use crate::{
        Result,
        assets::{VoiceAssets, verify_manifest},
        audio_worker::model_environment,
    };
    use cpal::{
        FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    };
    use nora_audio::{FrameAssembler, SileroVad, capture::CaptureStream};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    pub struct Recorder {
        capture: CaptureStream,
        vad: SileroVad,
        _environment: ort::environment::Environment,
        converter: CaptureConverter,
        frames: FrameAssembler,
        trim: VadTrim,
        pcm: Vec<f32>,
        drain: Vec<f32>,
        began: Instant,
        max_seconds: u32,
        invalid: bool,
        cancelled: bool,
    }
    impl Recorder {
        pub fn start(assets: &VoiceAssets, max_seconds: u32) -> Result<Self> {
            if !(1..=MAX_RECORD_SECONDS).contains(&max_seconds) {
                return Err("recording limit must be in1..=60seconds".into());
            }
            assets.verify_vad()?;
            verify_manifest(
                &assets.root.join("ort"),
                include_str!("../../../assets/manifests/onnxruntime-win-x64-cpu.json"),
            )?;
            // Capture starts only here, after explicit PTT. Its bounded queue
            // retains the first phonemes while the small VAD session loads.
            let capture = CaptureStream::start_default(MAX_CAPTURE_RING_SAMPLES)?;
            let began = Instant::now();
            let rate = capture.sample_rate();
            let converter = CaptureConverter::new(rate, max_seconds)?;
            let environment = model_environment(&assets.ort())?;
            let vad = SileroVad::load(&environment, assets.vad(), 2)?;
            let mut pcm = Vec::new();
            pcm.try_reserve_exact(max_seconds as usize * 16_000)?;
            let mut value = Self {
                capture,
                vad,
                _environment: environment,
                converter,
                frames: FrameAssembler::new(512)?,
                trim: VadTrim::default(),
                pcm,
                drain: vec![0.0; 4096],
                began,
                max_seconds,
                invalid: false,
                cancelled: false,
            };
            if value.poll()?.limit_reached {
                return Err("recording initialization exceeded duration limit".into());
            }
            Ok(value)
        }

        pub fn poll(&mut self) -> Result<RecordingProgress> {
            if self.cancelled || self.invalid {
                return Err("recording was cancelled or invalidated".into());
            }
            let elapsed_ms = self.began.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            if self.began.elapsed() >= Duration::from_secs(u64::from(self.max_seconds)) {
                self.invalid = true;
                let _ = self.capture.pause();
                self.pcm.fill(0.0);
                self.pcm.clear();
                return Ok(RecordingProgress {
                    elapsed_ms,
                    captured_samples_16khz: 0,
                    speech_detected: false,
                    limit_reached: true,
                });
            }
            if let Err(error) = self.drain_capture() {
                self.invalid = true;
                let _ = self.capture.pause();
                self.pcm.fill(0.0);
                self.pcm.clear();
                return Err(error);
            }
            Ok(RecordingProgress {
                elapsed_ms,
                captured_samples_16khz: self.pcm.len(),
                speech_detected: self.trim.speech_detected(),
                limit_reached: false,
            })
        }

        fn drain_capture(&mut self) -> Result<()> {
            let status = self.capture.status();
            if status.dropped_frames != 0 || status.backend_errors != 0 {
                return Err("microphone discontinuity; complete utterance discarded".into());
            }
            // Bounded even if a device continuously produces new frames.
            for _ in 0..128 {
                let count = self.capture.drain_into(&mut self.drain)?;
                if count == 0 {
                    break;
                }
                let Self {
                    converter,
                    pcm,
                    frames,
                    vad,
                    trim,
                    ..
                } = self;
                converter.push(&self.drain[..count], |samples| {
                    consume_samples(pcm, frames, vad, trim, samples)
                })?;
            }
            let status = self.capture.status();
            if status.dropped_frames != 0 || status.backend_errors != 0 {
                return Err("microphone discontinuity; complete utterance discarded".into());
            }
            Ok(())
        }

        pub fn finish(mut self) -> Result<Recording> {
            if self.cancelled
                || self.invalid
                || self.began.elapsed() >= Duration::from_secs(u64::from(self.max_seconds))
            {
                return Err("recording is cancelled, invalid or exceeded duration".into());
            }
            self.capture.pause_preserving_buffer()?;
            self.drain_capture()?;
            {
                let Self {
                    converter,
                    pcm,
                    frames,
                    vad,
                    trim,
                    ..
                } = &mut self;
                converter.finish(|samples| consume_samples(pcm, frames, vad, trim, samples))?;
            }
            let mut frame_error = None;
            self.frames.finish(|frame| {
                if frame_error.is_none() {
                    match self.vad.probability(frame) {
                        Ok(p) => {
                            if let Err(e) = self.trim.observe(p) {
                                frame_error = Some(e)
                            }
                        }
                        Err(e) => frame_error = Some(e.into()),
                    }
                }
            });
            if let Some(error) = frame_error {
                return Err(error);
            }
            let (start, end) = self.trim.bounds(self.pcm.len())?;
            let mut pcm = std::mem::take(&mut self.pcm);
            let resampled_samples = self.converter.emitted_samples();
            if pcm.len() != resampled_samples {
                return Err("recording sample count mismatch".into());
            }
            let trimmed = end - start;
            pcm.copy_within(start..end, 0);
            pcm[trimmed..].fill(0.0);
            pcm.truncate(trimmed);
            let metrics = RecordingMetrics {
                source_sample_rate: self.capture.sample_rate(),
                source_channels: self.capture.device_channels(),
                raw_input_samples: self.converter.input_samples(),
                resampled_samples,
                trimmed_samples: trimmed,
                speech_frames: self.trim.speech_frames,
                recording_ms: self.began.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            };
            self.cancelled = true;
            Ok(Recording {
                pcm_16khz: pcm,
                metrics,
            })
        }

        pub fn cancel(&mut self) -> Result<()> {
            self.cancelled = true;
            let stopped = self.capture.pause();
            self.pcm.fill(0.0);
            self.pcm.clear();
            self.drain.fill(0.0);
            self.frames.reset();
            self.converter.clear();
            self.vad.reset();
            stopped.map_err(Into::into)
        }
    }
    impl Drop for Recorder {
        fn drop(&mut self) {
            let _ = self.cancel();
        }
    }

    fn consume_samples(
        pcm: &mut Vec<f32>,
        frames: &mut FrameAssembler,
        vad: &mut SileroVad,
        trim: &mut VadTrim,
        samples: &[f32],
    ) -> Result<()> {
        if pcm
            .len()
            .checked_add(samples.len())
            .is_none_or(|n| n > 16_000 * MAX_RECORD_SECONDS as usize)
        {
            return Err("resampled recording exceeds bound".into());
        }
        pcm.extend_from_slice(samples);
        let mut error = None;
        frames.push(samples, |frame| {
            if error.is_none() {
                match vad.probability(frame) {
                    Ok(p) => {
                        if let Err(e) = trim.observe(p) {
                            error = Some(e)
                        }
                    }
                    Err(e) => error = Some(e.into()),
                }
            }
        })?;
        if let Some(error) = error {
            return Err(error);
        }
        Ok(())
    }

    pub struct Speaker {
        stream: Option<Stream>,
        stop: Arc<AtomicBool>,
        complete: Arc<AtomicBool>,
        error: Arc<AtomicBool>,
        drain_at_ns: Arc<AtomicU64>,
        began: Instant,
    }
    impl Speaker {
        pub fn start(pcm48kmono: Vec<f32>) -> Result<Self> {
            let device = cpal::default_host()
                .default_output_device()
                .ok_or("no default speaker device")?;
            let supported = device.default_output_config()?;
            let config: StreamConfig = supported.clone().into();
            if config.channels == 0 || config.channels > 32 {
                return Err("unsupported speaker channel count".into());
            }
            let source = PlaybackSource::new(pcm48kmono, config.sample_rate.0)?;
            let stop = Arc::new(AtomicBool::new(false));
            let complete = Arc::new(AtomicBool::new(false));
            let error = Arc::new(AtomicBool::new(false));
            let drain_at_ns = Arc::new(AtomicU64::new(0));
            let began = Instant::now();
            let signals = Signals {
                stop: stop.clone(),
                complete: complete.clone(),
                error: error.clone(),
                drain_at_ns: drain_at_ns.clone(),
                began,
            };
            let stream = match supported.sample_format() {
                SampleFormat::F32 => build_output::<f32>(&device, &config, source, signals)?,
                SampleFormat::I16 => build_output::<i16>(&device, &config, source, signals)?,
                SampleFormat::U16 => build_output::<u16>(&device, &config, source, signals)?,
                _ => return Err("unsupported speaker PCM format".into()),
            };
            stream.play()?;
            Ok(Self {
                stream: Some(stream),
                stop,
                complete,
                error,
                drain_at_ns,
                began,
            })
        }
        pub fn stop(&mut self) -> Result<()> {
            self.stop.store(true, Ordering::Release);
            let result = self
                .stream
                .take()
                .map(|stream| {
                    let paused = stream.pause();
                    drop(stream);
                    paused
                })
                .transpose();
            self.complete.store(true, Ordering::Release);
            self.drain_at_ns.store(0, Ordering::Release);
            result.map(|_| ()).map_err(Into::into)
        }
        /// Device-timestamp drain estimate, not proof the user heard the audio.
        pub fn finished(&self) -> bool {
            self.error.load(Ordering::Acquire)
                || (self.complete.load(Ordering::Acquire)
                    && self.began.elapsed().as_nanos()
                        >= u128::from(self.drain_at_ns.load(Ordering::Acquire)))
        }
        pub fn failed(&self) -> bool {
            self.error.load(Ordering::Acquire)
        }
    }
    impl Drop for Speaker {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    struct Signals {
        stop: Arc<AtomicBool>,
        complete: Arc<AtomicBool>,
        error: Arc<AtomicBool>,
        drain_at_ns: Arc<AtomicU64>,
        began: Instant,
    }
    fn build_output<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &StreamConfig,
        mut source: PlaybackSource,
        signals: Signals,
    ) -> Result<Stream> {
        let channels = usize::from(config.channels);
        let rate = config.sample_rate.0;
        let error_signal = signals.error.clone();
        Ok(device.build_output_stream(
            config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                if signals.stop.load(Ordering::Acquire) || signals.error.load(Ordering::Acquire) {
                    data.fill(T::EQUILIBRIUM);
                    return;
                }
                if !data.len().is_multiple_of(channels) {
                    signals.error.store(true, Ordering::Release);
                    data.fill(T::EQUILIBRIUM);
                    return;
                }
                for (index, frame) in data.chunks_exact_mut(channels).enumerate() {
                    let sample = match source.next_sample() {
                        Ok(Some(sample)) => sample.clamp(-1.0, 1.0),
                        Ok(None) => {
                            if !signals.complete.load(Ordering::Acquire) {
                                let stamp = info.timestamp();
                                let latency = stamp
                                    .playback
                                    .duration_since(&stamp.callback)
                                    .unwrap_or_default();
                                let drain = signals.began.elapsed()
                                    + latency
                                    + Duration::from_secs_f64(index as f64 / f64::from(rate))
                                    + Duration::from_millis(50);
                                signals.drain_at_ns.store(
                                    drain.as_nanos().min(u128::from(u64::MAX)) as u64,
                                    Ordering::Release,
                                );
                                signals.complete.store(true, Ordering::Release);
                            }
                            0.0
                        }
                        Err(_) => {
                            signals.error.store(true, Ordering::Release);
                            0.0
                        }
                    };
                    frame.fill(T::from_sample(sample));
                }
            },
            move |_| {
                error_signal.store(true, Ordering::Release);
            },
            None,
        )?)
    }
}

#[cfg(feature = "native-voice")]
pub use native::{Recorder, Speaker};

#[cfg(test)]
mod tests {
    use super::processing::*;

    #[test]
    fn capture_resampling_is_chunk_invariant_and_preserves_exact_duration() {
        let input: Vec<f32> = (0..44_100)
            .map(|i| (i as f32 * std::f32::consts::TAU * 440.0 / 44_100.0).sin() * 0.2)
            .collect();
        let render = |chunk: usize| {
            let mut converter = CaptureConverter::new(44_100, 2).unwrap();
            let mut output = Vec::new();
            for data in input.chunks(chunk) {
                converter
                    .push(data, |v| {
                        output.extend_from_slice(v);
                        Ok(())
                    })
                    .unwrap();
            }
            converter
                .finish(|v| {
                    output.extend_from_slice(v);
                    Ok(())
                })
                .unwrap();
            assert_eq!(converter.input_samples(), 44_100);
            assert_eq!(converter.emitted_samples(), 16_000);
            converter.clear();
            output
        };
        assert_eq!(render(1), render(777));
        assert_eq!(render(4096).len(), 16_000);
    }
    #[test]
    fn recording_bounds_nonfinite_and_sink_errors_do_not_silently_clip() {
        let mut converter = CaptureConverter::new(16_000, 1).unwrap();
        assert!(converter.push(&[f32::NAN], |_| Ok(())).is_err());
        assert!(CaptureConverter::new(48_000, 61).is_err());
        assert!(CaptureConverter::new(11_025, 1).is_err());
        let mut converter = CaptureConverter::new(16_000, 1).unwrap();
        assert!(converter.push(&vec![0.0; 16_001], |_| Ok(())).is_err());
        let mut converter = CaptureConverter::new(16_000, 1).unwrap();
        assert!(
            converter
                .push(&[0.0; 320], |_| Err("sink failed".into()))
                .is_err()
        );
    }
    #[test]
    fn vad_trims_edges_but_preserves_midutterance_pause() {
        let mut vad = VadTrim::default();
        for p in [
            0.0, 0.0, 0.0, 0.0, 0.0, 0.9, 0.8, 0.8, 0.0, 0.0, 0.0, 0.0, 0.9, 0.9, 0.9, 0.0,
        ] {
            vad.observe(p).unwrap();
        }
        assert!(vad.speech_detected());
        let (start, end) = vad.bounds(16 * 512).unwrap();
        assert_eq!(start, 0);
        assert!(end >= 15 * 512);
        assert_eq!(vad.speech_frames, 6);
        let mut silence = VadTrim::default();
        for _ in 0..100 {
            silence.observe(0.01).unwrap();
        }
        assert!(silence.bounds(51_200).is_err());
        assert!(silence.observe(f32::NAN).is_err());
    }
    #[test]
    fn playback_resampling_flushes_tail_without_full_multichannel_allocation() {
        for rate in [
            8_000, 16_000, 22_050, 24_000, 44_100, 48_000, 96_000, 192_000,
        ] {
            let mut source = PlaybackSource::new(vec![0.2; 4_800], rate).unwrap();
            let mut count = 0;
            while let Some(sample) = source.next_sample().unwrap() {
                assert!(sample.is_finite());
                count += 1;
                assert!(count <= rate as usize);
            }
            assert_eq!(count, rate as usize / 10);
            assert!(source.next_sample().unwrap().is_none());
        }
        assert!(PlaybackSource::new(vec![f32::INFINITY], 48_000).is_err());
        assert!(PlaybackSource::new(Vec::new(), 48_000).is_err());
    }
}
