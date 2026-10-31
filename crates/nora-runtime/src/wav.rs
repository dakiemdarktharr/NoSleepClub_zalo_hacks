//! Bounded, local WAV import. Decoding precedes any cloud admission. Supports
//! mono/stereo PCM16 and float32, then band-limited conversion to16 kHz mono.
use crate::Result;
use std::{fs::File, io::Read, path::Path};

pub fn read_voice_wav(path: &Path, max_seconds: u32) -> Result<Vec<f32>> {
    if !(1..=60).contains(&max_seconds) {
        return Err("invalid voice duration".into());
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 24_000_000 {
        return Err("WAV file exceeds limit".into());
    }
    let mut bytes = Vec::new();
    file.take(24_000_001).read_to_end(&mut bytes)?;
    decode(&bytes, max_seconds)
}
fn decode(bytes: &[u8], max_seconds: u32) -> Result<Vec<f32>> {
    if bytes.len() < 12
        || bytes.len() > 24_000_000
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into()?) as usize + 8 != bytes.len()
    {
        return Err("invalid RIFF/WAVE length".into());
    }
    let mut offset = 12;
    let mut format = None;
    let mut audio = None;
    let mut chunks = 0;
    while offset < bytes.len() {
        chunks += 1;
        if chunks > 64 || offset + 8 > bytes.len() {
            return Err("WAV chunk bound".into());
        }
        let tag = &bytes[offset..offset + 4];
        let len = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(len).ok_or("WAV overflow")?;
        if end > bytes.len() {
            return Err("truncated WAV chunk".into());
        }
        match tag {
            b"fmt " => {
                if format.is_some() || len != 16 {
                    return Err("unsupported WAV format header".into());
                }
                format = Some(&bytes[start..end]);
            }
            b"data" => {
                if audio.is_some() {
                    return Err("duplicate WAV audio".into());
                }
                audio = Some(&bytes[start..end]);
            }
            _ => {}
        }
        offset = end.checked_add(len % 2).ok_or("WAV padding overflow")?;
    }
    if offset != bytes.len() {
        return Err("missing WAV padding".into());
    }
    let fmt = format.ok_or("missing WAV format")?;
    let data = audio.ok_or("missing WAV audio")?;
    let u16_at = |n| u16::from_le_bytes([fmt[n], fmt[n + 1]]);
    let encoding = u16_at(0);
    let channels = usize::from(u16_at(2));
    let rate = u32::from_le_bytes(fmt[4..8].try_into()?);
    let bits = u16_at(14);
    let width = usize::from(bits / 8);
    if !(1..=2).contains(&channels)
        || !matches!((encoding, bits), (1, 16) | (3, 32))
        || u16_at(12) as usize != channels * width
        || u32::from_le_bytes(fmt[8..12].try_into()?)
            != rate
                .checked_mul((channels * width) as u32)
                .ok_or("WAV rate overflow")?
        || data.is_empty()
        || !data.len().is_multiple_of(channels * width)
    {
        return Err("unsupported WAV encoding".into());
    }
    let frames = data.len() / (channels * width);
    if frames as u64 > u64::from(rate) * u64::from(max_seconds) {
        return Err("WAV duration exceeds recording limit".into());
    }
    let chunk = (rate / 50) as usize;
    let mut resampler = nora_audio::MonoResampler::new(rate, 16_000, chunk)?;
    let target = (frames as u64 * 16_000 / u64::from(rate)) as usize;
    if target == 0 {
        return Err("empty resampled WAV".into());
    }
    let mut output = Vec::with_capacity(target);
    let mut input = vec![0.0; chunk];
    let mut converted = vec![0.0; resampler.output_samples_max()];
    let mut skip = resampler.output_delay_samples();
    let source_chunks = frames.div_ceil(chunk);
    for block in 0..source_chunks + 32 {
        input.fill(0.0);
        for (n, sample) in input.iter_mut().enumerate() {
            let frame = block * chunk + n;
            if frame >= frames {
                break;
            }
            let mut sum = 0.0;
            for channel in 0..channels {
                let p = (frame * channels + channel) * width;
                let value = if encoding == 1 {
                    f32::from(i16::from_le_bytes(data[p..p + 2].try_into()?)) / 32768.0
                } else {
                    f32::from_le_bytes(data[p..p + 4].try_into()?)
                };
                if !value.is_finite() || value.abs() > 1.0 {
                    return Err("invalid WAV sample".into());
                }
                sum += value;
            }
            *sample = sum / channels as f32;
        }
        let count = resampler.process_into(&input, &mut converted)?;
        let discard = skip.min(count);
        skip -= discard;
        let available = &converted[discard..count];
        let take = available.len().min(target - output.len());
        output.extend_from_slice(&available[..take]);
        if output.len() == target {
            return Ok(output);
        }
    }
    Err("WAV resampling did not complete".into())
}
#[cfg(test)]
mod tests {
    fn wav(rate: u32, channels: u16, frames: usize) -> Vec<u8> {
        let len = frames * usize::from(channels) * 2;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + len as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        b.extend_from_slice(&(channels * 2).to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(len as u32).to_le_bytes());
        for _ in 0..frames * usize::from(channels) {
            b.extend_from_slice(&8192i16.to_le_bytes());
        }
        b
    }
    #[test]
    fn stereo_conversion_preserves_duration_and_level() {
        let out = super::decode(&wav(48_000, 2, 48_000), 30).unwrap();
        assert_eq!(out.len(), 16_000);
        assert!((out[8000] - 0.25).abs() < 0.01);
    }
    #[test]
    fn rejects_truncation_and_excess_duration_without_clipping() {
        let mut bytes = wav(16_000, 1, 32_000);
        assert!(super::decode(&bytes, 1).is_err());
        bytes.pop();
        assert!(super::decode(&bytes, 30).is_err());
        assert!(super::decode(b"RIFF", 30).is_err());
    }
}
