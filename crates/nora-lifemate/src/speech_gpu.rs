//! GPU ASR through a pinned native executable. Only loopback is used, behind a
//! random per-run URL namespace. The public directory is empty; conversion and
//! external programs are disabled. The model process receives no credentials.
use crate::{Result, bridge::gpu_command};
use serde_json::Value;
use std::{
    fs::File,
    io::Read,
    net::TcpListener,
    path::Path,
    process::{Child, Stdio},
    time::{Duration, Instant},
};

pub struct GpuSpeech {
    child: Child,
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    origin: String,
    boundary: String,
    beam_size: u32,
    _scratch: tempfile::TempDir,
}
impl GpuSpeech {
    pub fn start(executable: &Path, model: &Path, log: &Path) -> Result<Self> {
        Self::start_with_beam(executable, model, log, 1)
    }
    /// A bounded decoding ablation. Validate before opening files or spawning
    /// the native worker; the same value configures startup and each request.
    pub fn start_with_beam(
        executable: &Path,
        model: &Path,
        log: &Path,
        beam_size: u32,
    ) -> Result<Self> {
        if !(1..=10).contains(&beam_size) {
            return Err("invalid GPU ASR beam size; expected 1..10".into());
        }
        let beam = beam_size.to_string();
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| "ASR service entropy unavailable")?;
        let secret = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let prefix = format!("/nora-{secret}");
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let scratch = tempfile::Builder::new()
            .prefix("nora-asr-public-")
            .tempdir_in(log.parent().ok_or("ASR log has no parent")?)?;
        let runtime = crate::async_transport::runtime()?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            // Each sample alternates synchronous model and ASR phases.
            // Do not reuse connections a server may close during that gap.
            .pool_max_idle_per_host(0)
            .connect_timeout(Duration::from_secs(2))
            .build()?;
        let output = File::create_new(log)?;
        let child = gpu_command(executable)
            .arg("--model")
            .arg(model)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--request-path",
                &prefix,
            ])
            .arg("--public")
            .arg(scratch.path())
            .args([
                "--threads",
                "8",
                "--processors",
                "1",
                "--language",
                "vi",
                "--best-of",
                "1",
                "--beam-size",
                &beam,
                "--max-context",
                "0",
                "--no-fallback",
                "--flash-attn",
                "--no-timestamps",
                "--device",
                "0",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(output))
            .spawn()?;
        let mut speech = Self {
            child,
            client,
            runtime,
            origin: format!("http://127.0.0.1:{port}{prefix}"),
            boundary: format!("NORA_AUDIO_{secret}"),
            beam_size,
            _scratch: scratch,
        };
        let startup = Instant::now();
        loop {
            if speech.child.try_wait()?.is_some() {
                return Err("GPU ASR process exited; inspect its local log".into());
            }
            if startup.elapsed() > Duration::from_secs(180) {
                return Err("GPU ASR startup deadline exceeded".into());
            }
            let healthy = speech.runtime.block_on(async {
                let response = speech
                    .client
                    .get(format!("{}/health", speech.origin))
                    .timeout(Duration::from_secs(1))
                    .send()
                    .await?;
                Ok::<_, reqwest::Error>(response.status().is_success())
            });
            if matches!(healthy, Ok(true)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        // Prewarm kernels using synthetic silence. max_context=0 and the
        // upstream default no_context=true prohibit carry-over to real audio.
        let _ = speech.infer_bytes(&zero_wav(), Duration::from_secs(180))?;
        Ok(speech)
    }
    fn infer_bytes(&mut self, wav: &[u8], timeout: Duration) -> Result<String> {
        if timeout.is_zero() || wav.len() > 2_000_000 {
            return Err("ASR deadline or size bound".into());
        }
        if self.child.try_wait()?.is_some() {
            return Err("GPU ASR service is no longer alive".into());
        }
        let mut body = Vec::with_capacity(wav.len() + 2048);
        let beam = self.beam_size.to_string();
        for (name, value) in [
            ("response_format", "json"),
            ("temperature", "0"),
            ("temperature_inc", "0"),
            ("beam_size", beam.as_str()),
            ("best_of", "1"),
            ("language", "vi"),
        ] {
            body.extend_from_slice(
                format!(
                    "--{}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n",
                    self.boundary
                )
                .as_bytes(),
            );
        }
        body.extend_from_slice(format!("--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n",self.boundary).as_bytes());
        body.extend_from_slice(wav);
        body.extend_from_slice(format!("\r\n--{}--\r\n", self.boundary).as_bytes());
        let content_type = format!("multipart/form-data; boundary={}", self.boundary);
        self.runtime.block_on(async {
            let mut response = self
                .client
                .post(format!("{}/inference", self.origin))
                .timeout(timeout)
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        "GPU ASR request deadline exceeded"
                    } else if error.is_connect() {
                        "GPU ASR connection failed"
                    } else {
                        "GPU ASR request transport failed"
                    }
                })?;
            if !response.status().is_success() {
                return Err(format!("GPU ASR HTTP {}", response.status().as_u16()).into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "GPU ASR response read failed")?
            {
                if bytes.len().saturating_add(chunk.len()) > 262_144 {
                    return Err("ASR response exceeds bound".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let value: Value = serde_json::from_slice(&bytes)?;
            let text = value
                .get("text")
                .and_then(Value::as_str)
                .ok_or("GPU ASR response is missing text")?;
            if text.len() > 65_536 {
                return Err("GPU ASR transcript exceeds bound".into());
            }
            Ok(text.trim().to_owned())
        })
    }
}
impl crate::runner::Speech for GpuSpeech {
    fn transcribe(&mut self, path: &Path, deadline: Instant) -> Result<String> {
        drop(crate::speech::read_official_wav(path)?);
        let mut wav = Vec::new();
        File::open(path)?.take(2_000_001).read_to_end(&mut wav)?;
        let text = self.infer_bytes(&wav, deadline.saturating_duration_since(Instant::now()))?;
        if text.is_empty() {
            return Err("GPU ASR returned an empty transcript".into());
        }
        Ok(text)
    }
}
impl Drop for GpuSpeech {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn zero_wav() -> Vec<u8> {
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&32_036u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&32_000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&32_000u32.to_le_bytes());
    wav.resize(32_044, 0);
    wav
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_beam_is_rejected_before_worker_or_log_creation() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-native-worker");
        let log = directory.path().join("must-not-exist.log");
        for beam in [0, 11, u32::MAX] {
            let error = super::GpuSpeech::start_with_beam(&missing, &missing, &log, beam)
                .err()
                .expect("invalid beam must fail before spawning");
            assert!(error.to_string().contains("invalid GPU ASR beam size"));
            assert!(!log.exists());
        }
    }

    #[test]
    fn warmup_is_one_second_of_synthetic_silence_not_a_dataset_asset() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("warmup.wav");
        std::fs::write(&path, super::zero_wav()).unwrap();
        let samples = crate::speech::read_official_wav(&path).unwrap();
        assert_eq!(samples.len(), 16_000);
        assert!(samples.iter().all(|s| *s == 0.0));
    }
}
