//! Official LifeMate audio is PCM16 mono at 16 kHz. Decode exactly once, without
//! lossy re-encoding, online ASR, a gold transcript, or an environment read.
use crate::Result;
use nora_audio::WhisperEngine;
use std::{io::Read, path::Path, time::Instant};

pub struct SpeechRecognizer {
    engine: WhisperEngine,
}
impl SpeechRecognizer {
    pub fn load(model: &Path) -> Result<Self> {
        Ok(Self {
            engine: WhisperEngine::load(model, 16_000 * 60, 8)?,
        })
    }
    pub fn transcribe(&mut self, path: &Path, deadline: Instant) -> Result<String> {
        if Instant::now() >= deadline {
            return Err("sample deadline exceeded before ASR".into());
        }
        let samples = read_official_wav(path)?;
        let transcript = self.engine.transcribe_16khz(&samples, "vi")?;
        if Instant::now() >= deadline {
            return Err("sample deadline exceeded during ASR".into());
        }
        if transcript.text.len() > 65_536 || transcript.text.trim().is_empty() {
            return Err("ASR returned an empty or oversized transcript".into());
        }
        Ok(transcript.text.trim().to_owned())
    }
}

/// A native inference operation cannot hold the supervisor beyond its deadline.
/// Do not use the pinned upstream's callback helper for a captured Rust closure.
pub struct SpeechWorker {
    child: std::process::Child,
    input: std::process::ChildStdin,
    responses: std::sync::mpsc::Receiver<std::result::Result<String, String>>,
    executable: std::path::PathBuf,
    model: std::path::PathBuf,
    root: std::path::PathBuf,
}
impl SpeechWorker {
    pub fn start(executable: &Path, model: &Path, root: &Path) -> Result<Self> {
        use std::{
            io::{BufRead, BufReader, Read},
            process::Stdio,
            sync::mpsc,
            thread,
            time::Duration,
        };
        let mut child = crate::bridge::clean_command(executable)
            .arg("--speech-worker")
            .arg(model)
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().ok_or("missing speech worker input")?;
        let output = child.stdout.take().ok_or("missing speech worker output")?;
        let (sender, responses) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let result = (|| -> std::result::Result<String, String> {
                    let count = reader
                        .by_ref()
                        .take(262_145)
                        .read_until(b'\n', &mut line)
                        .map_err(|_| "speech transport failed")?;
                    if count == 0 || count > 262_144 {
                        return Err("speech output bound or closed stream".into());
                    }
                    let value: serde_json::Value =
                        serde_json::from_slice(&line).map_err(|_| "invalid speech worker JSON")?;
                    value
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| "speech worker did not return text".into())
                })();
                let failed = result.is_err();
                if sender.send(result).is_err() || failed {
                    break;
                }
            }
        });
        let mut worker = Self {
            child,
            input,
            responses,
            executable: executable.to_owned(),
            model: model.to_owned(),
            root: root.to_owned(),
        };
        match worker.responses.recv_timeout(Duration::from_secs(90)) {
            Ok(Ok(text)) if text == "NORA_ASR_READY" => Ok(worker),
            _ => {
                let _ = worker.child.kill();
                let _ = worker.child.wait();
                Err("native ASR worker startup failed".into())
            }
        }
    }
    pub fn recognize(&mut self, path: &Path, deadline: Instant) -> Result<String> {
        use std::io::Write;
        if self.child.try_wait()?.is_some() {
            return Err("native ASR worker exited; start a new validation run".into());
        }
        let relative = path
            .strip_prefix(&self.root)?
            .to_str()
            .ok_or("audio path is not UTF-8")?
            .replace('\\', "/");
        let request = serde_json::to_vec(&serde_json::json!({"audio":relative}))?;
        self.input.write_all(&request)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        let duration = deadline.saturating_duration_since(Instant::now());
        match self.responses.recv_timeout(duration) {
            Ok(Ok(text)) if !text.trim().is_empty() => Ok(text),
            _ => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                Err("native ASR failed or exceeded sample deadline".into())
            }
        }
    }
    pub fn configuration(&self) -> (&Path, &Path) {
        (&self.executable, &self.model)
    }
}
impl Drop for SpeechWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn worker_main(model: &Path, root: &Path) -> Result<()> {
    use std::io::{BufRead, Write};
    #[cfg(windows)]
    let _guard = nora_platform::BudgetJob::install(1_700_000_000)?;
    let mut recognizer = SpeechRecognizer::load(model)?;
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{{\"text\":\"NORA_ASR_READY\"}}")?;
    stdout.flush()?;
    loop {
        let mut line = Vec::new();
        if stdin.by_ref().take(8193).read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if line.len() > 8192 {
            return Err("speech input exceeds bound".into());
        }
        let request: serde_json::Value = serde_json::from_slice(&line)?;
        let relative = request
            .get("audio")
            .and_then(serde_json::Value::as_str)
            .ok_or("missing speech path")?;
        let path = crate::protocol::audio_path(root, relative)?;
        let text =
            recognizer.transcribe(&path, Instant::now() + std::time::Duration::from_secs(60))?;
        serde_json::to_writer(&mut stdout, &serde_json::json!({"text":text}))?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
pub fn read_official_wav(path: &Path) -> Result<Vec<f32>> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > 2_000_000 {
        return Err("contest audio exceeds 60 second bound".into());
    }
    let mut bytes = Vec::new();
    file.take(2_000_001).read_to_end(&mut bytes)?;
    decode(&bytes)
}
fn decode(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() < 44
        || bytes.len() > 2_000_000
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into()?) as usize + 8 != bytes.len()
    {
        return Err("invalid WAV container".into());
    }
    let mut offset = 12usize;
    let mut format = false;
    let mut samples = None;
    let mut chunks = 0;
    while offset < bytes.len() {
        chunks += 1;
        if offset + 8 > bytes.len() || chunks > 64 {
            return Err("WAV chunk limit".into());
        }
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size).ok_or("WAV overflow")?;
        if end > bytes.len() {
            return Err("truncated WAV chunk".into());
        }
        match &bytes[offset..offset + 4] {
            b"fmt " => {
                if format || size < 16 {
                    return Err("invalid WAV format".into());
                }
                let fmt = &bytes[start..end];
                if u16::from_le_bytes(fmt[0..2].try_into()?) != 1
                    || u16::from_le_bytes(fmt[2..4].try_into()?) != 1
                    || u32::from_le_bytes(fmt[4..8].try_into()?) != 16_000
                    || u32::from_le_bytes(fmt[8..12].try_into()?) != 32_000
                    || u16::from_le_bytes(fmt[12..14].try_into()?) != 2
                    || u16::from_le_bytes(fmt[14..16].try_into()?) != 16
                {
                    return Err("expected official 16 kHz mono PCM16 WAV".into());
                }
                format = true;
            }
            b"data" => {
                if samples.is_some()
                    || size == 0
                    || !size.is_multiple_of(2)
                    || size / 2 > 16_000 * 60
                {
                    return Err("invalid WAV sample bound".into());
                }
                samples = Some(
                    bytes[start..end]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
                        .collect(),
                );
            }
            _ => {}
        }
        offset = end.checked_add(size % 2).ok_or("WAV padding overflow")?;
    }
    if !format || offset != bytes.len() {
        return Err("missing WAV format or padding".into());
    }
    samples.ok_or_else(|| "missing WAV audio".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_pcm16_without_resampling_or_channel_loss() {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&(-32768i16).to_le_bytes());
        bytes.extend_from_slice(&16384i16.to_le_bytes());
        assert_eq!(decode(&bytes).unwrap(), vec![-1.0, 0.5]);
        bytes[22] = 2;
        assert!(decode(&bytes).is_err());
    }
}
