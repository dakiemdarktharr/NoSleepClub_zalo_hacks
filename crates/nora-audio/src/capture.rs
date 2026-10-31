//! Explicit microphone capture through CPAL. Nothing opens a device at import.
//! Caller must obtain user microphone/device permission before start_default.
//! Any dropped frame/backend error invalidates the current utterance: reset the
//! frontend rather than silently joining incomplete audio into an ASR request.

use crate::{AudioError, PcmRing};
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Debug, Clone, Serialize)]
pub struct InputDeviceInfo {
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CaptureStatus {
    pub dropped_frames: u64,
    pub backend_errors: u64,
}

/// Enumerates at most64 inputs with a usable default configuration. Devices
/// without one are omitted; no capture stream is opened by enumeration.
pub fn input_devices() -> Result<Vec<InputDeviceInfo>, AudioError> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(backend)?;
    let mut out = Vec::new();
    for device in devices.take(64) {
        let config = match device.default_input_config() {
            Ok(config) => config,
            Err(_) => continue,
        };
        out.push(InputDeviceInfo {
            name: device.name().map_err(backend)?,
            sample_rate: config.sample_rate().0,
            channels: config.channels(),
            sample_format: format!("{:?}", config.sample_format()),
        });
    }
    Ok(out)
}

fn backend(error: impl std::fmt::Display) -> AudioError {
    AudioError::Backend(error.to_string())
}

pub struct CaptureStream {
    stream: Stream,
    ring: Arc<Mutex<PcmRing>>,
    dropped: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
}

impl CaptureStream {
    pub fn start_default(capacity_samples: usize) -> Result<Self, AudioError> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| backend("no default microphone"))?;
        let supported = device.default_input_config().map_err(backend)?;
        let config: StreamConfig = supported.clone().into();
        if config.channels == 0
            || config.channels > 32
            || !(8_000..=192_000).contains(&config.sample_rate.0)
        {
            return Err(AudioError::Configuration(
                "unsupported input device configuration",
            ));
        }
        let ring = Arc::new(Mutex::new(PcmRing::new(capacity_samples)?));
        let dropped = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));
        let active = Arc::new(AtomicBool::new(true));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(
                &device,
                &config,
                ring.clone(),
                dropped.clone(),
                errors.clone(),
                active.clone(),
            )?,
            SampleFormat::I16 => build::<i16>(
                &device,
                &config,
                ring.clone(),
                dropped.clone(),
                errors.clone(),
                active.clone(),
            )?,
            SampleFormat::U16 => build::<u16>(
                &device,
                &config,
                ring.clone(),
                dropped.clone(),
                errors.clone(),
                active.clone(),
            )?,
            _ => {
                return Err(AudioError::Configuration(
                    "unsupported microphone PCM format",
                ));
            }
        };
        stream.play().map_err(backend)?;
        Ok(Self {
            stream,
            ring,
            dropped,
            errors,
            active,
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn device_channels(&self) -> u16 {
        self.channels
    }
    pub fn status(&self) -> CaptureStatus {
        CaptureStatus {
            dropped_frames: self.dropped.load(Ordering::Relaxed),
            backend_errors: self.errors.load(Ordering::Relaxed),
        }
    }
    pub fn drain_into(&self, output: &mut [f32]) -> Result<usize, AudioError> {
        Ok(self
            .ring
            .lock()
            .map_err(|_| backend("capture queue poisoned"))?
            .read_into(output))
    }
    pub fn pause(&self) -> Result<(), AudioError> {
        self.active.store(false, Ordering::Release);
        let paused = self.stream.pause().map_err(backend);
        self.ring
            .lock()
            .map_err(|_| backend("capture queue poisoned"))?
            .clear();
        paused
    }
    /// Stop accepting callbacks while preserving the captured tail. The caller
    /// drains the bounded queue before ending the utterance; cancel uses pause.
    pub fn pause_preserving_buffer(&self) -> Result<(), AudioError> {
        self.active.store(false, Ordering::Release);
        self.stream.pause().map_err(backend)
    }
    pub fn resume(&self) -> Result<(), AudioError> {
        self.active.store(true, Ordering::Release);
        if let Err(error) = self.stream.play() {
            self.active.store(false, Ordering::Release);
            return Err(backend(error));
        }
        Ok(())
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    ring: Arc<Mutex<PcmRing>>,
    dropped: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
) -> Result<Stream, AudioError>
where
    T: SizedSample + Sample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let error_counter = errors.clone();
    device
        .build_input_stream(
            config,
            move |samples: &[T], _| {
                if !active.load(Ordering::Acquire) {
                    return;
                }
                let mut queue = match ring.try_lock() {
                    Ok(queue) => queue,
                    Err(_) => {
                        dropped.fetch_add((samples.len() / channels) as u64, Ordering::Relaxed);
                        return;
                    }
                };
                if !active.load(Ordering::Acquire) {
                    return;
                }
                if !samples.len().is_multiple_of(channels) {
                    error_counter.fetch_add(1, Ordering::Relaxed);
                }
                for frame in samples.chunks_exact(channels) {
                    let mono = (frame
                        .iter()
                        .map(|sample| f64::from(sample.to_sample::<f32>()))
                        .sum::<f64>()
                        / channels as f64) as f32;
                    if queue.try_push(&[mono]).is_err() {
                        dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
            move |_| {
                errors.fetch_add(1, Ordering::Relaxed);
            },
            None,
        )
        .map_err(backend)
}
