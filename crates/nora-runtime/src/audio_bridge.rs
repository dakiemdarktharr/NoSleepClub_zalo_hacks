//! Parent-side voice lifecycle. At most one native model child exists; all
//! native jobs use the AppContainer launcher and are killed on interruption.

use crate::{Result, config::AppConfig};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum AudioEvent {
    RecordingStarted,
    Transcribing,
    Transcript(String),
    Speaking { notice_id: String },
    SpeechFinished { notice_id: String },
    SpeechSkipped { notice_id: String },
    Idle,
    Error(String),
}

#[cfg(all(windows, feature = "native-voice"))]
mod native {
    use super::*;
    use crate::{
        assets::VoiceAssets,
        audio_worker::{self, WorkerMetadata, WorkerRequest},
        voice::{Recorder, Speaker},
    };
    use nora_platform::sandbox::{SandboxChild, SandboxRequest, spawn_with_lease};
    use std::{
        collections::VecDeque,
        fs::{self, File},
        io::Write,
        time::{Duration, Instant},
    };

    enum Kind {
        Asr,
        Tts(String),
    }
    struct Pending {
        child: SandboxChild,
        _scratch: tempfile::TempDir,
        metadata: PathBuf,
        pcm: PathBuf,
        lease_path: PathBuf,
        started: Instant,
        kind: Kind,
    }

    pub struct AudioBridge {
        config: AppConfig,
        executable: PathBuf,
        recorder: Option<Recorder>,
        native: Option<Pending>,
        speaker: Option<(String, Speaker)>,
        speech: VecDeque<(String, String)>,
        skipped: VecDeque<String>,
        quiet: bool,
    }

    impl AudioBridge {
        pub fn new(config: AppConfig, executable: PathBuf) -> Self {
            Self {
                quiet: config.quiet,
                config,
                executable,
                recorder: None,
                native: None,
                speaker: None,
                speech: VecDeque::new(),
                skipped: VecDeque::new(),
            }
        }
        pub fn recording(&self) -> bool {
            self.recorder.is_some()
        }
        pub fn speaking(&self) -> bool {
            self.speaker.is_some()
                || self
                    .native
                    .as_ref()
                    .is_some_and(|p| matches!(p.kind, Kind::Tts(_)))
        }
        pub fn busy(&self) -> bool {
            self.recording() || self.speaking() || self.native.is_some()
        }
        pub fn set_quiet(&mut self, quiet: bool) {
            self.quiet = quiet;
            if quiet {
                self.stop_speech();
            }
        }

        pub fn start_recording(&mut self) -> Result<()> {
            self.stop_speech();
            self.cancel_recording();
            self.recorder = Some(Recorder::start(
                &VoiceAssets::new(self.config.asset_root.clone()),
                self.config.max_recording_seconds,
            )?);
            Ok(())
        }
        pub fn stop_recording(&mut self) -> Result<bool> {
            let Some(recorder) = self.recorder.take() else {
                return Ok(false);
            };
            let recording = recorder.finish()?;
            self.spawn_asr(&recording.pcm_16khz)?;
            Ok(true)
        }
        pub fn cancel_recording(&mut self) {
            if let Some(mut recorder) = self.recorder.take() {
                let _ = recorder.cancel();
            }
            if self
                .native
                .as_ref()
                .is_some_and(|p| matches!(p.kind, Kind::Asr))
            {
                self.cancel_native();
            }
        }
        pub fn stop_speech(&mut self) {
            self.skipped.extend(self.speech.drain(..).map(|(id, _)| id));
            if let Some((id, mut speaker)) = self.speaker.take() {
                self.skipped.push_back(id);
                let _ = speaker.stop();
            }
            if self
                .native
                .as_ref()
                .is_some_and(|p| matches!(p.kind, Kind::Tts(_)))
            {
                if let Some(Pending {
                    kind: Kind::Tts(id),
                    ..
                }) = &self.native
                {
                    self.skipped.push_back(id.clone());
                }
                self.cancel_native();
            }
        }
        pub fn enqueue(&mut self, notice_id: String, text: &str) {
            if self.quiet || self.recorder.is_some() {
                self.skipped.push_back(notice_id);
                return;
            }
            let text = speakable(text);
            if text.is_empty() {
                self.skipped.push_back(notice_id);
                return;
            }
            while self.speech.len() >= 2 {
                if let Some((id, _)) = self.speech.pop_front() {
                    self.skipped.push_back(id);
                }
            }
            self.speech.push_back((notice_id, text));
        }
        fn cancel_native(&mut self) {
            if let Some(mut pending) = self.native.take() {
                let _ = pending.child.kill();
                if pending.child.finish().is_ok() {
                    let _ = fs::remove_file(pending.lease_path);
                }
            }
        }
        fn fresh_scratch(&self) -> Result<tempfile::TempDir> {
            let directory = self.config.state_dir.join("native-scratch");
            fs::create_dir_all(&directory)?;
            Ok(tempfile::Builder::new()
                .prefix("audio-")
                .tempdir_in(directory)?)
        }
        pub fn import_pcm(&mut self, pcm: &[f32]) -> Result<()> {
            if self.busy() {
                return Err("voice job already active".into());
            }
            self.spawn_asr(pcm)
        }
        fn spawn_asr(&mut self, pcm: &[f32]) -> Result<()> {
            if self.native.is_some() {
                return Err("a native model is already resident".into());
            }
            let scratch = self.fresh_scratch()?;
            let input = scratch.path().join("input.pcm");
            let metadata = scratch.path().join("result.json");
            audio_worker::write_pcm(&input, pcm)?;
            let request = WorkerRequest::Asr {
                assets_root: self.config.asset_root.canonicalize()?,
                input_pcm: input,
                output_json: metadata.clone(),
            };
            self.spawn(request, scratch, metadata, PathBuf::new(), Kind::Asr)
        }
        fn spawn_tts(&mut self, id: String, text: String) -> Result<()> {
            let scratch = self.fresh_scratch()?;
            let metadata = scratch.path().join("result.json");
            let pcm = scratch.path().join("output.pcm");
            let request = WorkerRequest::Tts {
                assets_root: self.config.asset_root.canonicalize()?,
                text,
                output_pcm: pcm.clone(),
                output_json: metadata.clone(),
            };
            self.spawn(request, scratch, metadata, pcm, Kind::Tts(id))
        }
        fn spawn(
            &mut self,
            request: WorkerRequest,
            scratch: tempfile::TempDir,
            metadata: PathBuf,
            pcm: PathBuf,
            kind: Kind,
        ) -> Result<()> {
            let request_path = scratch.path().join("request.json");
            let mut request_file = File::create_new(&request_path)?;
            serde_json::to_writer(&mut request_file, &request)?;
            request_file.sync_all()?;
            drop(request_file);
            let lease_dir = self.config.state_dir.join("native-leases");
            fs::create_dir_all(&lease_dir)?;
            crate::leases::validate_native_roots(&self.config, &self.executable)?;
            let lease_path = lease_dir.join(format!("{}.json", crate::new_id("lease")?));
            let executable = self.executable.canonicalize()?;
            let bin = executable
                .parent()
                .ok_or("native executable has no parent")?
                .to_owned();
            let work_dir = scratch.path().canonicalize()?;
            let spec = SandboxRequest {
                executable,
                args: vec![
                    "--voice-worker".into(),
                    request_path
                        .to_str()
                        .ok_or("worker path must be UTF-8")?
                        .to_owned(),
                ],
                read_roots: vec![bin, self.config.asset_root.canonicalize()?],
                log_path: work_dir.join("worker.log"),
                work_dir,
            };
            let child = spawn_with_lease(&spec, &mut |lease| {
                let mut file = tempfile::NamedTempFile::new_in(&lease_dir)?;
                serde_json::to_writer(&mut file, lease).map_err(std::io::Error::other)?;
                file.flush()?;
                file.as_file().sync_all()?;
                file.persist(&lease_path).map_err(|e| e.error)?;
                Ok(())
            })?;
            self.native = Some(Pending {
                child,
                _scratch: scratch,
                metadata,
                pcm,
                lease_path,
                started: Instant::now(),
                kind,
            });
            Ok(())
        }
        pub fn poll(&mut self) -> Vec<AudioEvent> {
            let mut events: Vec<_> = self
                .skipped
                .drain(..)
                .map(|notice_id| AudioEvent::SpeechSkipped { notice_id })
                .collect();
            if let Some(recorder) = self.recorder.as_mut() {
                match recorder.poll() {
                    Ok(p) if p.limit_reached => {
                        self.cancel_recording();
                        events.push(AudioEvent::Error("Đã chạm thời lượng ghi âm tối đa; hãy nói lại ngắn hơn để tránh mất phần cuối.".into()));
                    }
                    Ok(_) => {}
                    Err(_) => {
                        self.cancel_recording();
                        events.push(AudioEvent::Error(
                            "Ghi âm bị gián đoạn. Chưa gửi yêu cầu; hãy thử nói lại.".into(),
                        ));
                    }
                }
            }
            if let Some((id, speaker)) = self.speaker.as_ref() {
                if speaker.failed() {
                    events.push(AudioEvent::SpeechSkipped {
                        notice_id: id.clone(),
                    });
                    self.speaker.take();
                    events.push(AudioEvent::Error(
                        "Không thể phát giọng nói; kết quả vẫn ở trong cửa sổ.".into(),
                    ));
                } else if speaker.finished() {
                    let id = id.clone();
                    self.speaker.take();
                    events.push(AudioEvent::SpeechFinished { notice_id: id });
                }
            }
            if let Some(pending) = self.native.as_mut() {
                let storage = pending.child.storage_bytes();
                let done = if pending.started.elapsed() > Duration::from_secs(180)
                    || storage.is_err()
                    || storage.is_ok_and(|b| b > 32_000_000)
                {
                    let _ = pending.child.kill();
                    Some(Err(std::io::Error::other(
                        "native worker deadline/disk bound",
                    )))
                } else {
                    match pending.child.try_wait() {
                        Ok(Some(code)) => Some(Ok(code)),
                        Ok(None) => None,
                        Err(e) => Some(Err(e)),
                    }
                };
                if let Some(done) = done {
                    let pending = self.native.take().expect("pending native job exists");
                    let parsed = match done {
                        Ok(0) => audio_worker::read_worker_metadata(&pending.metadata),
                        _ => Err("native inference did not complete".into()),
                    };
                    match (parsed, &pending.kind) {
                        (Ok(WorkerMetadata::Asr(asr)), Kind::Asr)
                            if asr.sample_rate == 16_000
                                && asr.language == "vi"
                                && !asr.text.trim().is_empty()
                                && asr.text.len() <= 16_384 =>
                        {
                            events.push(AudioEvent::Transcript(asr.text))
                        }
                        (Ok(WorkerMetadata::Tts(tts)), Kind::Tts(id))
                            if tts.sample_rate == 48_000
                                && tts.samples > 0
                                && tts.samples <= 4_000_000
                                && tts.stop_reason == audio_worker::TtsStopReason::Eos =>
                        {
                            let playback = File::open(&pending.pcm)
                                .map_err(crate::Error::from)
                                .and_then(|file| audio_worker::read_pcm(file, 4_000_000))
                                .and_then(|pcm| {
                                    if pcm.len() != tts.samples {
                                        return Err("PCM metadata mismatch".into());
                                    }
                                    Speaker::start(pcm)
                                });
                            match playback {
                                Ok(speaker) => {
                                    self.speaker = Some((id.clone(), speaker));
                                    events.push(AudioEvent::Speaking {
                                        notice_id: id.clone(),
                                    });
                                }
                                Err(_) => {
                                    events.push(AudioEvent::SpeechSkipped {
                                        notice_id: id.clone(),
                                    });
                                    events.push(AudioEvent::Error(
                                        "Không thể phát giọng nói; kết quả vẫn ở trong cửa sổ."
                                            .into(),
                                    ));
                                }
                            }
                        }
                        _ => {
                            if let Kind::Tts(id) = &pending.kind {
                                events.push(AudioEvent::SpeechSkipped {
                                    notice_id: id.clone(),
                                });
                            }
                            events.push(AudioEvent::Error(
                            "Xử lý giọng nói chưa hoàn tất. Bạn có thể nhập yêu cầu bằng văn bản."
                                .into(),
                        ));
                        }
                    }
                    if pending.child.finish().is_ok() {
                        let _ = fs::remove_file(pending.lease_path);
                    }
                }
            }
            if self.native.is_none()
                && self.recorder.is_none()
                && self.speaker.is_none()
                && !self.quiet
                && let Some((id, text)) = self.speech.pop_front()
                && self.spawn_tts(id.clone(), text).is_err()
            {
                events.push(AudioEvent::SpeechSkipped { notice_id: id });
                events.push(AudioEvent::Error(
                    "Chưa thể chuẩn bị giọng nói; kết quả vẫn ở trong cửa sổ.".into(),
                ));
            }
            events
        }
    }
    impl Drop for AudioBridge {
        fn drop(&mut self) {
            self.cancel_recording();
            self.stop_speech();
            self.cancel_native();
        }
    }
}

#[cfg(all(windows, feature = "native-voice"))]
pub use native::AudioBridge;

#[cfg(not(all(windows, feature = "native-voice")))]
pub struct AudioBridge;
#[cfg(not(all(windows, feature = "native-voice")))]
impl AudioBridge {
    pub fn new(_config: AppConfig, _executable: PathBuf) -> Self {
        Self
    }
    pub fn recording(&self) -> bool {
        false
    }
    pub fn speaking(&self) -> bool {
        false
    }
    pub fn busy(&self) -> bool {
        false
    }
    pub fn set_quiet(&mut self, _quiet: bool) {}
    pub fn start_recording(&mut self) -> Result<()> {
        Err("this build has no native voice support".into())
    }
    pub fn stop_recording(&mut self) -> Result<bool> {
        Ok(false)
    }
    pub fn cancel_recording(&mut self) {}
    pub fn stop_speech(&mut self) {}
    pub fn enqueue(&mut self, _id: String, _text: &str) {}
    pub fn import_pcm(&mut self, _pcm: &[f32]) -> Result<()> {
        Err("this build has no native voice support".into())
    }
    pub fn poll(&mut self) -> Vec<AudioEvent> {
        Vec::new()
    }
}

#[cfg(any(all(windows, feature = "native-voice"), test))]
fn speakable(text: &str) -> String {
    let first = text
        .split("\n\n")
        .find(|p| !p.trim().is_empty() && !p.trim_start().starts_with("```"))
        .unwrap_or("");
    // Remove presentation markup only. Mathematical/comparison symbols must
    // never silently change meaning: fall back to a pointer to the full text.
    if first.contains(['=', '<', '>', '[', ']', '{', '}', '|', '\\']) {
        return "Kết quả chi tiết đã có trong cửa sổ NORA.".into();
    }
    let plain: String = first
        .chars()
        .filter(|c| {
            !matches!(c, '*' | '`' | '#' | '_')
                && !matches!(*c as u32,0x1f000..=0x1ffff|0x2600..=0x27ff|0xfe0f)
        })
        .collect();
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = crate::truncate_utf8(plain.trim(), 1024);
    if text.len() == plain.trim().len() {
        text.to_owned()
    } else {
        text.rsplit_once(['.', '!', '?'])
            .map(|(complete, _)| format!("{complete}."))
            .unwrap_or_else(|| "Kết quả đã có trong cửa sổ NORA.".into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn speech_summary_never_splits_unicode_or_reads_code_fences() {
        assert_eq!(super::speakable("```sh\nrm data\n```"), "");
        let text = "Nội dung tiếng Việt. ".repeat(200);
        let speech = super::speakable(&text);
        assert!(speech.len() <= 1024 && speech.ends_with('.'));
    }
    #[test]
    fn speech_removes_presentation_but_does_not_rewrite_formula() {
        assert_eq!(
            super::speakable("**Đã tạo 12 ghi chú.** ✅"),
            "Đã tạo 12 ghi chú."
        );
        assert_eq!(
            super::speakable("x < 3"),
            "Kết quả chi tiết đã có trong cửa sổ NORA."
        );
    }
}
