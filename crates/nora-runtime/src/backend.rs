//! Single trusted desktop actor. UI commands, model proposals and native-host
//! outputs enter distinct channels; only UI commands can change permissions.

use crate::{
    Result,
    audio_bridge::{AudioBridge, AudioEvent},
    config::AppConfig,
    new_id, now_ms,
    runner::{self, GoalControl, GoalInput, RunOutcome, RunRequest, RunnerEvent},
    truncate_utf8,
};
use nora_broker::{ApprovedLaunch, Broker, RootPermissions};
use nora_core::notifications::{self, Context, Event, EventKind, Speech, Surface};
use nora_desktop::contracts::*;
use nora_llm::{ApiConfig, ResponsesClient};
use nora_store::{GoalStatus, Store};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct AppSession {
    channels: Option<UiChannels>,
    initial: AppSnapshot,
    commands: SyncSender<UiCommand>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<()>>>,
}

impl AppSession {
    pub fn prepare(mut config: AppConfig) -> Result<Self> {
        fs::create_dir_all(&config.state_dir)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let owner = options.open(config.state_dir.join("supervisor.lock"))?;
        owner
            .try_lock()
            .map_err(|_| "NORA đã chạy với thư mục dữ liệu này")?;
        config.load_preferences()?;
        crate::leases::recover(&config, &std::env::current_exe()?)?;
        let mut broker = Broker::open(config.state_dir.join("broker"))?;
        broker.protect_path(&config.state_dir)?;
        if config.llm_config.exists() {
            broker.protect_path(&config.llm_config)?;
        }
        let mut store = Store::open(config.state_dir.join("goals.sqlite"))?;
        store.recover_abandoned_goals(now_ms()?)?;
        let mut snapshot = AppSnapshot {
            quiet: config.quiet,
            public_research: config.public_research,
            ..Default::default()
        };
        snapshot.goals = store
            .list_goals(MAX_GOALS)?
            .into_iter()
            .map(|g| GoalView {
                id: g.id.clone(),
                title: truncate_utf8(&g.objective, 256).into(),
                status: goal_phase(g.status),
                detail: store
                    .result(&g.id)
                    .ok()
                    .flatten()
                    .map(|s| truncate_utf8(&s, 2048).to_owned())
                    .unwrap_or_default(),
            })
            .collect();
        refresh_config(&config, &mut snapshot);
        refresh_grants(&broker, &mut snapshot)?;
        snapshot.voice_status = if cfg!(all(windows, feature = "native-voice")) {
            "Nhấn nút micro hoặc giữ Ctrl+Shift+Space để nói.".into()
        } else {
            "Bản chạy này chưa bật giọng nói local.".into()
        };
        let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_CAPACITY);
        let (event_tx, event_rx) = mpsc::sync_channel(EVENT_CAPACITY);
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let (runner_tx, runner_rx) = mpsc::sync_channel(64);
        let stop = Arc::new(AtomicBool::new(false));
        let mut actor = Actor {
            audio: AudioBridge::new(config.clone(), std::env::current_exe()?),
            config,
            snapshot: snapshot.clone(),
            broker: Arc::new(Mutex::new(broker)),
            store: Arc::new(Mutex::new(store)),
            commands: command_rx,
            events: RefCell::new(crate::event_outbox::EventOutbox::new(event_tx)),
            wake_rx,
            wake: None,
            runner_tx,
            runner_rx,
            active: None,
            session_id: new_id("session")?,
            stop: stop.clone(),
            pending: BTreeMap::new(),
            visible: true,
            locked: false,
            last_session_check: Instant::now() - Duration::from_secs(2),
            last_reminder_check: Instant::now(),
            voice_hold: false,
            voice_reply_goal: None,
            _owner: owner,
        };
        let thread = thread::Builder::new()
            .name("nora-supervisor".into())
            .spawn(move || actor.run())?;
        Ok(Self {
            channels: Some(UiChannels::new(command_tx.clone(), event_rx).with_wake_sender(wake_tx)),
            initial: snapshot,
            commands: command_tx,
            stop,
            thread: Some(thread),
        })
    }
    pub fn run(self) -> Result<()> {
        self.run_with_options(nora_desktop::DesktopOptions::default())
    }
    pub fn run_with_options(mut self, options: nora_desktop::DesktopOptions) -> Result<()> {
        let channels = self.channels.take().ok_or("UI channels already consumed")?;
        let outcome = nora_desktop::run_with_options(channels, self.initial.clone(), options);
        self.stop.store(true, Ordering::Release);
        let _ = self.commands.try_send(UiCommand::Quit);
        let backend = self
            .thread
            .take()
            .ok_or("supervisor thread missing")?
            .join()
            .map_err(|_| "supervisor stopped unexpectedly")?;
        outcome?;
        backend
    }
    /// Explicit command-line audio import uses the same trusted UI boundary.
    pub fn enqueue(&self, command: UiCommand) -> Result<()> {
        self.commands.try_send(command)?;
        Ok(())
    }
    pub fn schedule_quit(&self, seconds: u64) -> Result<()> {
        if !(1..=600).contains(&seconds) {
            return Err("diagnostic deadline out of range".into());
        }
        let commands = self.commands.clone();
        thread::Builder::new()
            .name("nora-diagnostic-deadline".into())
            .spawn(move || {
                thread::sleep(Duration::from_secs(seconds));
                let _ = commands.send(UiCommand::Quit);
            })?;
        Ok(())
    }
    /// A test driver uses the same bounded command/event boundary as the native
    /// shell; it does not receive filesystem/approval authority from model JSON.
    pub fn into_driver(mut self) -> Result<AppDriver> {
        let channels = self.channels.take().ok_or("UI channels already consumed")?;
        Ok(AppDriver {
            channels,
            commands: self.commands.clone(),
            stop: self.stop.clone(),
            thread: self.thread.take(),
        })
    }
}
impl Drop for AppSession {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop.store(true, Ordering::Release);
            let _ = self.commands.try_send(UiCommand::Quit);
        }
    }
}

pub struct AppDriver {
    pub channels: UiChannels,
    pub commands: SyncSender<UiCommand>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<()>>>,
}
impl AppDriver {
    pub fn shutdown(mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        let _ = self.commands.try_send(UiCommand::Quit);
        self.thread
            .take()
            .ok_or("missing supervisor")?
            .join()
            .map_err(|_| "supervisor failed")?
    }
}
impl Drop for AppDriver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.commands.try_send(UiCommand::Quit);
    }
}

struct Active {
    id: String,
    control: Arc<GoalControl>,
    input: SyncSender<GoalInput>,
    thread: JoinHandle<RunOutcome>,
}
struct PendingNotice {
    goal_id: String,
    kind: NoticeKind,
    key: String,
    surface: bool,
    speech: bool,
    speech_required: bool,
    reminder: Option<String>,
}
impl PendingNotice {
    fn delivered(&self) -> bool {
        if self.speech_required {
            self.speech
        } else {
            self.surface
        }
    }
}
struct Actor {
    config: AppConfig,
    snapshot: AppSnapshot,
    broker: Arc<Mutex<Broker>>,
    store: Arc<Mutex<Store>>,
    commands: Receiver<UiCommand>,
    events: RefCell<crate::event_outbox::EventOutbox>,
    wake_rx: Receiver<UiWake>,
    wake: Option<UiWake>,
    runner_tx: SyncSender<RunnerEvent>,
    runner_rx: Receiver<RunnerEvent>,
    active: Option<Active>,
    session_id: String,
    stop: Arc<AtomicBool>,
    audio: AudioBridge,
    pending: BTreeMap<String, PendingNotice>,
    visible: bool,
    locked: bool,
    last_session_check: Instant,
    last_reminder_check: Instant,
    voice_hold: bool,
    voice_reply_goal: Option<String>,
    _owner: File,
}

impl Actor {
    fn run(&mut self) -> Result<()> {
        self.publish_snapshot();
        while !self.stop.load(Ordering::Acquire) {
            match self.events.borrow_mut().flush() {
                Ok(n) if n > 0 => {
                    if let Some(wake) = &self.wake {
                        wake.wake();
                    }
                }
                Err(()) => break,
                _ => {}
            }
            if let Ok(wake) = self.wake_rx.try_recv() {
                self.wake = Some(wake);
                self.publish_snapshot();
            }
            self.session_check();
            for _ in 0..64 {
                match self.runner_rx.try_recv() {
                    Ok(event) => self.runner_event(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
            self.reap();
            for event in self.audio.poll() {
                self.audio_event(event);
            }
            if self.snapshot.speaking != self.audio.speaking() {
                self.snapshot.speaking = self.audio.speaking();
                self.publish_snapshot();
            }
            if self.last_reminder_check.elapsed() >= Duration::from_secs(1) {
                self.last_reminder_check = Instant::now();
                self.reminders();
            }
            match self.commands.recv_timeout(Duration::from_millis(30)) {
                Ok(command) => {
                    if self.command(command).is_err() {
                        self.error("Thao tác chưa thực hiện được. Kiểm tra quyền hoặc trạng thái hiện tại rồi thử lại.");
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        self.audio.cancel_recording();
        self.audio.stop_speech();
        if let Some(active) = self.active.take() {
            active.control.shutdown.store(true, Ordering::Release);
            active.control.abort_request.store(true, Ordering::Release);
            let _ = active.thread.join();
        }
        self.publish(RuntimeEvent::Shutdown);
        Ok(())
    }
    fn publish(&self, event: RuntimeEvent) {
        if self.events.borrow_mut().push(event) {
            if let Some(wake) = &self.wake {
                wake.wake();
            }
        } else {
            self.stop.store(true, Ordering::Release);
        }
    }
    fn publish_snapshot(&self) {
        self.publish(RuntimeEvent::Snapshot(Box::new(self.snapshot.clone())));
    }
    fn error(&self, message: &str) {
        self.publish(RuntimeEvent::Error {
            message: message.into(),
        });
    }
    fn user_message(&self, text: String) {
        self.publish(RuntimeEvent::Message(ChatMessage::new(
            new_id("message").unwrap_or_else(|_| "user-message".into()),
            MessageRole::User,
            text,
        )));
    }
    fn assistant_message(&self, text: String) {
        self.publish(RuntimeEvent::Message(ChatMessage::new(
            new_id("message").unwrap_or_else(|_| "assistant-message".into()),
            MessageRole::Assistant,
            text,
        )));
    }
    fn phase(&mut self, id: &str, phase: GoalPhase, detail: &str) {
        if let Some(goal) = self.snapshot.goals.iter_mut().find(|g| g.id == id) {
            goal.status = phase;
            goal.detail = truncate_utf8(detail, 2048).into();
        }
        self.snapshot.status_text = truncate_utf8(detail, 512).into();
        self.publish_snapshot();
    }
    fn authority_changed(&mut self) {
        if let Some(active) = &self.active {
            active
                .control
                .authority_epoch
                .fetch_add(1, Ordering::AcqRel);
            active.control.abort_request.store(true, Ordering::Release);
        }
    }
    fn command(&mut self, command: UiCommand) -> Result<()> {
        match command {
            UiCommand::Quit => self.stop.store(true, Ordering::Release),
            UiCommand::Submit { text } => {
                if self.submit(text.clone()).is_err() {
                    self.publish(RuntimeEvent::InputRejected{text,message:"Yêu cầu chưa được gửi. Kiểm tra cấu hình, trạng thái goal và dữ liệu nhạy cảm trước khi thử lại.".into()});
                }
            }
            UiCommand::ImportAudio { path } => {
                if self.locked
                    || self.audio.recording()
                    || self.snapshot.mic == MicState::Transcribing
                {
                    return Err("voice import unavailable".into());
                }
                let pcm = crate::wav::read_voice_wav(&path, self.config.max_recording_seconds)?;
                self.audio.stop_speech();
                self.drop_pending_speech();
                self.capture_voice_reply_goal();
                if let Some(active) = &self.active
                    && !active.control.paused.swap(true, Ordering::AcqRel)
                {
                    self.voice_hold = true;
                }
                if self.audio.import_pcm(&pcm).is_err() {
                    self.release_voice_hold();
                    return Err("voice import failed".into());
                }
                self.snapshot.mic = MicState::Transcribing;
                self.snapshot.voice_status =
                    "Đang nhận diện file âm thanh được bạn chọn trên máy.".into();
            }
            UiCommand::StartRecording => {
                if self.locked {
                    return Err("session is locked".into());
                }
                if self.audio.recording() {
                    return Ok(());
                }
                self.drop_pending_speech();
                self.capture_voice_reply_goal();
                if let Some(active) = &self.active
                    && !active.control.paused.swap(true, Ordering::AcqRel)
                {
                    self.voice_hold = true;
                }
                if self.audio.start_recording().is_err() {
                    self.release_voice_hold();
                    self.error("Chưa thể bật micro. Kiểm tra quyền micro, thiết bị đầu vào và model giọng nói.");
                    self.snapshot.mic = MicState::Unavailable;
                } else {
                    self.snapshot.mic = MicState::Recording;
                    self.snapshot.speaking = false;
                    self.snapshot.voice_status =
                        "Đang nghe. Thả phím hoặc nhấn dừng để gửi.".into();
                }
            }
            UiCommand::StopRecording => match self.audio.stop_recording() {
                Ok(true) => {
                    self.snapshot.mic = MicState::Transcribing;
                    self.snapshot.voice_status = "Đang nhận diện tiếng Việt trên máy.".into();
                }
                Ok(false) => {}
                Err(_) => {
                    self.snapshot.mic = MicState::Off;
                    self.release_voice_hold();
                    self.error("Chưa nhận được câu nói đủ rõ. Yêu cầu chưa được gửi.");
                }
            },
            UiCommand::CancelVoice => {
                if !matches!(
                    self.snapshot.mic,
                    MicState::Recording | MicState::Transcribing
                ) {
                    return Ok(());
                }
                self.audio.cancel_recording();
                self.voice_reply_goal = None;
                self.release_voice_hold();
                self.snapshot.mic = MicState::Off;
                self.snapshot.voice_status = "Đã hủy thu/nhận diện; chưa gửi câu nói này.".into();
            }
            UiCommand::StopSpeech => {
                self.audio.stop_speech();
                self.snapshot.speaking = false;
                self.drop_pending_speech();
            }
            UiCommand::SetQuiet { enabled } => {
                self.config.quiet = enabled;
                self.snapshot.quiet = enabled;
                self.audio.set_quiet(enabled);
                if enabled {
                    self.drop_pending_speech();
                    self.snapshot.speaking = false;
                }
                self.config.save_preferences()?;
            }
            UiCommand::SetPublicResearch { enabled } => {
                let broker = self.broker.clone();
                let _guard = broker.lock().map_err(|_| "broker unavailable")?;
                self.config.public_research = enabled;
                self.snapshot.public_research = enabled;
                if let Some(active) = &self.active {
                    active
                        .control
                        .public_research
                        .store(enabled, Ordering::Release);
                }
                self.authority_changed();
                self.config.save_preferences()?;
            }
            UiCommand::PauseGoal { goal_id } => {
                self.voice_hold = false;
                self.voice_reply_goal = None;
                if let Some(active) = self.active.as_ref().filter(|a| a.id == goal_id) {
                    active.control.paused.store(true, Ordering::Release);
                    active.control.abort_request.store(true, Ordering::Release);
                    self.phase(
                        &goal_id,
                        GoalPhase::Paused,
                        "Đã tạm dừng. Các bước đã kiểm chứng vẫn được giữ.",
                    );
                }
            }
            UiCommand::ResumeGoal { goal_id } => {
                if let Some(active) = self.active.as_ref().filter(|a| a.id == goal_id) {
                    active.control.paused.store(false, Ordering::Release);
                    active.input.try_send(GoalInput::Resume)?;
                } else if self.active.is_none() {
                    let goal = self
                        .store
                        .lock()
                        .map_err(|_| "goal store unavailable")?
                        .goal(&goal_id)?;
                    if matches!(
                        goal.status,
                        GoalStatus::Completed | GoalStatus::Cancelled | GoalStatus::Failed
                    ) {
                        return Err("goal cannot resume".into());
                    }
                    self.start_goal(goal_id, goal.objective)?;
                }
            }
            UiCommand::ShowResult { goal_id } => {
                if self.locked {
                    return Err("session is locked".into());
                }
                let result = self
                    .store
                    .lock()
                    .map_err(|_| "goal store unavailable")?
                    .result(&goal_id)?;
                self.assistant_message(
                    result.unwrap_or_else(|| "Mục tiêu này chưa có kết quả cuối được lưu.".into()),
                );
            }
            UiCommand::CancelGoal { goal_id } => {
                self.voice_hold = false;
                self.voice_reply_goal = None;
                self.drop_pending_speech();
                if let Some(active) = self.active.as_ref().filter(|a| a.id == goal_id) {
                    active.control.cancelled.store(true, Ordering::Release);
                    active.control.abort_request.store(true, Ordering::Release);
                    self.audio.stop_speech();
                    self.audio.cancel_recording();
                    self.snapshot.mic = MicState::Off;
                    self.snapshot.speaking = false;
                    self.phase(
                        &goal_id,
                        GoalPhase::Cancelled,
                        "Đang dừng; các tác động đã xảy ra sẽ được báo riêng.",
                    );
                } else {
                    let mut store = self.store.lock().map_err(|_| "goal store unavailable")?;
                    let g = store.goal(&goal_id)?;
                    let updated = store.cancel_future_work(&goal_id, g.revision, now_ms()?)?;
                    drop(store);
                    if updated.status == GoalStatus::NeedsReconciliation {
                        self.phase(&goal_id,GoalPhase::NeedsReconciliation,"Đã dừng các bước mới. Thao tác trước đó chưa rõ kết quả; cần đối chiếu trước khi tiếp tục.");
                    } else {
                        self.phase(
                            &goal_id,
                            GoalPhase::Cancelled,
                            "Đã hủy các bước chưa thực hiện.",
                        );
                    }
                }
            }
            UiCommand::AddFolder {
                path,
                can_read,
                can_create,
                allow_cloud,
            } => {
                if self.locked {
                    return Err("session is locked".into());
                }
                let label = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Thư mục được phép");
                let broker = self.broker.clone();
                let mut guard = broker.lock().map_err(|_| "broker unavailable")?;
                guard.grant_root(
                    truncate_utf8(label, 128),
                    &path,
                    RootPermissions {
                        read: can_read,
                        create_new: can_create,
                        cloud_disclosure: allow_cloud,
                    },
                    now_ms()?,
                )?;
                self.authority_changed();
                drop(guard);
                self.refresh_roots()?;
            }
            UiCommand::RevokeFolder { id } => {
                let broker = self.broker.clone();
                let mut guard = broker.lock().map_err(|_| "broker unavailable")?;
                guard.revoke_root(&id, now_ms()?)?;
                self.authority_changed();
                drop(guard);
                self.refresh_roots()?;
            }
            UiCommand::RegisterApp { path, label } => {
                if self.locked {
                    return Err("session is locked".into());
                }
                let broker = self.broker.clone();
                let mut guard = broker.lock().map_err(|_| "broker unavailable")?;
                guard.register_app(&label, &path, now_ms()?)?;
                self.authority_changed();
                drop(guard);
                self.refresh_roots()?;
            }
            UiCommand::RevokeApp { id } => {
                let broker = self.broker.clone();
                let mut guard = broker.lock().map_err(|_| "broker unavailable")?;
                guard.revoke_app(&id, now_ms()?)?;
                self.authority_changed();
                drop(guard);
                self.refresh_roots()?;
            }
            UiCommand::Approve { preview_id } => {
                if self.locked {
                    return Err("session is locked".into());
                }
                self.active
                    .as_ref()
                    .ok_or("no active goal")?
                    .input
                    .try_send(GoalInput::Approve { preview_id })?;
            }
            UiCommand::Reject { preview_id } => {
                self.active
                    .as_ref()
                    .ok_or("no active goal")?
                    .input
                    .try_send(GoalInput::Reject { preview_id })?;
            }
            UiCommand::RefreshConfig => {
                if self.active.is_some() {
                    return Err("a goal is using this configuration".into());
                }
                refresh_config(&self.config, &mut self.snapshot);
            }
            UiCommand::WindowHidden => {
                self.visible = false;
                self.audio.cancel_recording();
                self.snapshot.mic = MicState::Off;
                self.release_voice_hold();
            }
            UiCommand::WindowShown => self.visible = true,
            UiCommand::NoticeShown { id } => {
                if self.visible && !self.locked {
                    self.acknowledge(&id, false);
                }
            }
        }
        self.publish_snapshot();
        Ok(())
    }
    fn submit(&mut self, text: String) -> Result<()> {
        self.submit_with_voice_reply(text, None)
    }
    fn submit_with_voice_reply(
        &mut self,
        text: String,
        voice_reply_goal: Option<&str>,
    ) -> Result<()> {
        if text.trim().is_empty()
            || text.len() > MAX_INPUT_BYTES
            || self.locked
            || self.audio.recording()
            || self.snapshot.mic == MicState::Transcribing
        {
            return Err("request is not admissible".into());
        }
        let client = ResponsesClient::new(ApiConfig::load(&self.config.llm_config)?)?;
        client.validate_disclosure_text(&text)?;
        drop(client);
        if let Some(active) = &self.active {
            let phase = self
                .snapshot
                .goals
                .iter()
                .find(|g| g.id == active.id)
                .map(|g| g.status);
            if phase != Some(GoalPhase::WaitingInput)
                && !(phase == Some(GoalPhase::Paused)
                    && voice_reply_goal == Some(active.id.as_str()))
            {
                return Err("another goal is active".into());
            }
            use sha2::{Digest, Sha256};
            let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
            let store = self.store.lock().map_err(|_| "goal store unavailable")?;
            store.record_pending_input(&active.id, &digest, now_ms()?)?;
            if active
                .input
                .try_send(GoalInput::UserText { text: text.clone() })
                .is_err()
            {
                store.consume_pending_input(&active.id, &digest)?;
                return Err("goal input queue unavailable".into());
            }
            drop(store);
            let id = active.id.clone();
            self.withdraw_questions(&id);
            self.user_message(text);
            self.phase(
                &id,
                GoalPhase::Running,
                "Đã nhận thông tin bổ sung; đang cập nhật bước tiếp theo.",
            );
            return Ok(());
        }
        self.start_goal(new_id("goal")?, text.clone())?;
        self.user_message(text);
        Ok(())
    }
    fn start_goal(&mut self, id: String, objective: String) -> Result<()> {
        if self.active.is_some() {
            return Err("only one orchestrator is admitted".into());
        }
        {
            let store = self.store.lock().map_err(|_| "goal store unavailable")?;
            match store.goal(&id) {
                Ok(g) if g.objective == objective => {}
                Ok(_) => return Err("goal identity conflict".into()),
                Err(nora_store::StoreError::NotFound(_)) => {
                    store.create_goal(&id, &objective, now_ms()?)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        let control = Arc::new(GoalControl::default());
        control.session_locked.store(self.locked, Ordering::Release);
        control
            .public_research
            .store(self.config.public_research, Ordering::Release);
        let (input_tx, input_rx) = mpsc::sync_channel(16);
        let request = RunRequest {
            goal_id: id.clone(),
            session_id: self.session_id.clone(),
            objective: objective.clone(),
            llm_config: self.config.llm_config.clone(),
            public_research: self.config.public_research,
        };
        let broker = self.broker.clone();
        let store = self.store.clone();
        let worker_control = control.clone();
        let events = self.runner_tx.clone();
        let thread = thread::Builder::new()
            .name("nora-goal".into())
            .spawn(move || {
                runner::run_goal(
                    request,
                    broker,
                    store,
                    worker_control,
                    input_rx,
                    events,
                    Arc::new(launch_approved),
                )
            })?;
        if let Some(goal) = self.snapshot.goals.iter_mut().find(|g| g.id == id) {
            goal.status = GoalPhase::Queued;
        } else {
            self.snapshot.goals.insert(
                0,
                GoalView::new(&id, truncate_utf8(&objective, 256), GoalPhase::Queued),
            );
            self.snapshot.goals.truncate(MAX_GOALS);
        }
        self.snapshot.active_goal = Some(id.clone());
        self.snapshot.connection = ConnectionState::Busy;
        self.snapshot.status_text = "Đã nhận yêu cầu.".into();
        self.active = Some(Active {
            id,
            control,
            input: input_tx,
            thread,
        });
        self.publish_snapshot();
        Ok(())
    }
    fn refresh_roots(&mut self) -> Result<()> {
        let broker = self.broker.lock().map_err(|_| "broker unavailable")?;
        refresh_grants(&broker, &mut self.snapshot)
    }
    fn runner_event(&mut self, event: RunnerEvent) {
        match event {
            RunnerEvent::Phase {
                goal_id,
                phase,
                detail,
            } => self.phase(&goal_id, phase, &detail),
            RunnerEvent::AssistantText { goal_id, text } => {
                self.assistant_message(text.clone());
                if self
                    .snapshot
                    .goals
                    .iter()
                    .any(|g| g.id == goal_id && g.status == GoalPhase::WaitingInput)
                {
                    self.notice(&goal_id, NoticeKind::Question, &text, None);
                }
            }
            RunnerEvent::Preview(preview) => {
                self.notice_internal(
                    &preview.goal_id,
                    NoticeKind::Approval,
                    &format!(
                        "{} Kiểm tra đề xuất trong cửa sổ và xác nhận để tiếp tục.",
                        preview.title
                    ),
                    None,
                    None,
                    Some(format!("approval-{}", preview.id)),
                );
                self.publish(RuntimeEvent::Preview(preview));
            }
            RunnerEvent::ClearPreview { preview_id } => {
                self.withdraw_notice(&format!("approval-{preview_id}"));
                self.publish(RuntimeEvent::ClearPreview { id: preview_id })
            }
            RunnerEvent::Notice {
                goal_id,
                kind,
                text,
            } => self.notice(&goal_id, kind, &text, None),
            // Reap/join is the authoritative final boundary. A channel event
            // can arrive just before the worker actually exits.
            RunnerEvent::Finished(_) => {}
        }
    }
    fn reap(&mut self) {
        if self.active.as_ref().is_some_and(|a| a.thread.is_finished()) {
            let active = self.active.take().expect("finished goal exists");
            match active.thread.join() {
                Ok(outcome) => self.finished(&outcome),
                Err(_) => {
                    self.withdraw_questions(&active.id);
                    self.phase(
                        &active.id,
                        GoalPhase::NeedsReconciliation,
                        "Goal đã dừng bất thường. Cần kiểm tra lại trước khi tiếp tục.",
                    );
                    self.snapshot.active_goal = None;
                    self.snapshot.connection = ConnectionState::Error;
                    self.publish_snapshot();
                }
            }
        }
    }
    fn finished(&mut self, outcome: &RunOutcome) {
        self.withdraw_questions(&outcome.goal_id);
        let already = self.snapshot.goals.iter().any(|g| {
            g.id == outcome.goal_id
                && g.status == outcome.phase
                && self.snapshot.active_goal.as_deref() != Some(&outcome.goal_id)
        });
        self.phase(&outcome.goal_id, outcome.phase, &outcome.summary);
        self.snapshot.active_goal = None;
        self.snapshot.connection = ConnectionState::Ready;
        if !already && outcome.phase == GoalPhase::Completed {
            self.notice(
                &outcome.goal_id,
                NoticeKind::Completion,
                &outcome.summary,
                None,
            );
        } else if !already
            && matches!(
                outcome.phase,
                GoalPhase::Failed | GoalPhase::NeedsReconciliation | GoalPhase::WaitingInput
            )
        {
            self.notice(
                &outcome.goal_id,
                NoticeKind::Blocker,
                &outcome.summary,
                None,
            );
        }
        self.publish_snapshot();
    }
    fn release_voice_hold(&mut self) {
        if self.voice_hold {
            self.voice_hold = false;
            if let Some(active) = &self.active {
                active.control.paused.store(false, Ordering::Release);
                let _ = active.input.try_send(GoalInput::Resume);
            }
        }
    }
    fn capture_voice_reply_goal(&mut self) {
        self.voice_reply_goal = self
            .active
            .as_ref()
            .filter(|a| {
                self.snapshot
                    .goals
                    .iter()
                    .any(|g| g.id == a.id && g.status == GoalPhase::WaitingInput)
            })
            .map(|a| a.id.clone());
    }
    fn audio_event(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::Transcript(text) => {
                self.snapshot.mic = MicState::Off;
                self.snapshot.voice_status =
                    "Microphone đang tắt. Giữ Ctrl+Shift+Space để nói.".into();
                self.publish(RuntimeEvent::Transcript { text: text.clone() });
                let reply_goal = self.voice_reply_goal.take();
                let control = self.voice_control(&text);
                self.release_voice_hold();
                if !control
                    && self
                        .submit_with_voice_reply(text.clone(), reply_goal.as_deref())
                        .is_err()
                {
                    self.publish(RuntimeEvent::InputRejected{text,message:"Câu nói chưa được gửi. Kiểm tra trạng thái mục tiêu hoặc gửi lại phần thông tin bổ sung.".into()});
                }
            }
            AudioEvent::Speaking { notice_id: _ } => self.snapshot.speaking = true,
            AudioEvent::SpeechFinished { notice_id } => {
                self.snapshot.speaking = false;
                self.acknowledge(&notice_id, true);
            }
            AudioEvent::SpeechSkipped { notice_id } => self.skip_speech(&notice_id),
            AudioEvent::Error(message) => {
                self.voice_reply_goal = None;
                self.snapshot.mic = MicState::Off;
                self.snapshot.speaking = false;
                self.snapshot.voice_status =
                    "Microphone đang tắt. Bạn có thể thử lại hoặc nhập văn bản.".into();
                self.release_voice_hold();
                self.drop_pending_speech();
                self.error(&message);
            }
            AudioEvent::Transcribing => self.snapshot.mic = MicState::Transcribing,
            AudioEvent::RecordingStarted => self.snapshot.mic = MicState::Recording,
            AudioEvent::Idle => {}
        }
        self.publish_snapshot();
    }
    fn voice_control(&mut self, text: &str) -> bool {
        let text = normalized_control(text);
        if matches!(text.as_str(), "dừng nói" | "im lặng" | "stop speaking") {
            self.audio.stop_speech();
            self.snapshot.speaking = false;
            self.drop_pending_speech();
            return true;
        }
        let recognized = matches!(
            text.as_str(),
            "hủy goal"
                | "huỷ goal"
                | "hủy tác vụ"
                | "huỷ tác vụ"
                | "hủy nhiệm vụ"
                | "hủy mục tiêu"
                | "huỷ mục tiêu"
                | "tạm dừng"
                | "tạm dừng goal"
                | "tạm dừng mục tiêu"
                | "tiếp tục"
                | "tiếp tục goal"
                | "tiếp tục mục tiêu"
                | "đến đâu rồi"
                | "đang làm đến đâu"
                | "trạng thái"
                | "xong chưa"
        );
        if !recognized {
            return false;
        }
        let candidates: Vec<_> = self
            .snapshot
            .goals
            .iter()
            .filter(|g| g.status.can_resume())
            .collect();
        let id = if let Some(active) = &self.active {
            active.id.clone()
        } else if candidates.len() == 1 {
            candidates[0].id.clone()
        } else {
            self.assistant_message(if candidates.is_empty() {
                "Chưa có mục tiêu đang chờ tiếp tục.".into()
            } else {
                "Có nhiều mục tiêu đang chờ. Hãy chọn mục tiêu trong cửa sổ để tiếp tục hoặc hủy."
                    .into()
            });
            return true;
        };
        let command = match text.as_str() {
            "hủy goal" | "huỷ goal" | "hủy tác vụ" | "huỷ tác vụ" | "hủy nhiệm vụ"
            | "hủy mục tiêu" | "huỷ mục tiêu" => Some(UiCommand::CancelGoal {
                goal_id: id.clone(),
            }),
            "tạm dừng" | "tạm dừng goal" | "tạm dừng mục tiêu" => {
                Some(UiCommand::PauseGoal {
                    goal_id: id.clone(),
                })
            }
            "tiếp tục" | "tiếp tục goal" | "tiếp tục mục tiêu" => {
                Some(UiCommand::ResumeGoal {
                    goal_id: id.clone(),
                })
            }
            "đến đâu rồi" | "đang làm đến đâu" | "trạng thái" | "xong chưa" => {
                let status = self
                    .snapshot
                    .goals
                    .iter()
                    .find(|g| g.id == id)
                    .map(|g| format!("{}: {}", g.status.label(), g.detail))
                    .unwrap_or_else(|| "Đang kiểm tra trạng thái.".into());
                self.assistant_message(status.clone());
                self.notice_with_kind(
                    &id,
                    NoticeKind::Milestone,
                    &status,
                    None,
                    Some(EventKind::StatusRequested),
                );
                return true;
            }
            _ => None,
        };
        if let Some(command) = command {
            if matches!(
                command,
                UiCommand::PauseGoal { .. } | UiCommand::CancelGoal { .. }
            ) {
                self.voice_hold = false;
            }
            let _ = self.command(command);
            true
        } else {
            false
        }
    }
    fn session_check(&mut self) {
        if self.last_session_check.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_session_check = Instant::now();
        #[cfg(windows)]
        let locked = nora_platform::session_is_locked().unwrap_or(true);
        #[cfg(not(windows))]
        let locked = false;
        if locked != self.locked {
            self.locked = locked;
            if let Some(active) = &self.active {
                active
                    .control
                    .session_locked
                    .store(locked, Ordering::Release);
                if locked {
                    active.control.abort_request.store(true, Ordering::Release);
                }
            }
            if locked {
                self.audio.cancel_recording();
                self.audio.stop_speech();
                self.snapshot.mic = MicState::Off;
                self.snapshot.speaking = false;
                self.drop_pending_speech();
            }
            self.publish_snapshot();
        }
    }
    fn notice(&mut self, goal_id: &str, kind: NoticeKind, text: &str, reminder: Option<String>) {
        self.notice_with_kind(goal_id, kind, text, reminder, None);
    }
    fn notice_with_kind(
        &mut self,
        goal_id: &str,
        kind: NoticeKind,
        text: &str,
        reminder: Option<String>,
        override_kind: Option<EventKind>,
    ) {
        self.notice_internal(goal_id, kind, text, reminder, override_kind, None);
    }
    fn notice_internal(
        &mut self,
        goal_id: &str,
        kind: NoticeKind,
        text: &str,
        reminder: Option<String>,
        override_kind: Option<EventKind>,
        notice_id: Option<String>,
    ) {
        if text.trim().is_empty() || self.pending.len() >= 64 {
            return;
        }
        let Ok(now) = now_ms() else {
            return;
        };
        let event_kind = override_kind.unwrap_or(match kind {
            NoticeKind::Question => EventKind::InputRequired,
            NoticeKind::Approval => EventKind::PermissionRequired,
            NoticeKind::Completion => EventKind::GoalCompleted,
            NoticeKind::Permission => EventKind::PermissionRequired,
            NoticeKind::Blocker => EventKind::BlockerMaterial,
            NoticeKind::Finding => EventKind::FindingMaterial,
            NoticeKind::Milestone => EventKind::WorkerStarted,
            NoticeKind::Reminder => EventKind::StatusRequested,
        });
        use sha2::{Digest, Sha256};
        let digest = format!(
            "{:x}",
            Sha256::digest(format!("{goal_id}\0{kind:?}\0{text}").as_bytes())
        );
        let id = if let Some(id) = notice_id {
            id
        } else if override_kind == Some(EventKind::StatusRequested)
            || matches!(kind, NoticeKind::Question | NoticeKind::Approval)
        {
            new_id("status").unwrap_or_else(|_| format!("status-{now}"))
        } else {
            reminder
                .as_ref()
                .map(|r| format!("reminder-{r}"))
                .unwrap_or_else(|| format!("notice-{}", &digest[..24]))
        };
        if self.pending.contains_key(&id) {
            return;
        }
        let event = Event {
            goal_id: goal_id.into(),
            event_id: id.clone(),
            kind: event_kind,
            occurred_at: now as f64 / 1000.0,
            summary: truncate_utf8(text, 4096).into(),
            milestone_key: id.clone(),
            verified: true,
            evidence_refs: vec![format!("trusted-runtime:{goal_id}")],
            read_source_ids: Vec::new(),
        };
        let key = event.notice_key();
        let delivery = match self
            .store
            .lock()
            .ok()
            .and_then(|s| s.delivery_state(goal_id, &key).ok())
        {
            Some(d) => d,
            None => return,
        };
        let context = Context {
            now: now as f64 / 1000.0,
            quiet: self.snapshot.quiet || self.locked,
            user_speaking: self.audio.recording(),
            ..Default::default()
        };
        let Ok(decision) = notifications::decide(&event, &context, delivery) else {
            return;
        };
        if decision.surface == Surface::None && decision.speech != Speech::Ready {
            return;
        }
        let needs_speech = decision.speech == Speech::Ready;
        if decision.surface != Surface::None {
            self.publish(RuntimeEvent::Notice(UserNotice::new(
                &id,
                kind,
                match kind {
                    NoticeKind::Question => "Cần thêm thông tin",
                    NoticeKind::Approval => "Cần bạn xác nhận",
                    NoticeKind::Completion => "Hoàn thành",
                    NoticeKind::Permission => "Cần quyền truy cập",
                    NoticeKind::Blocker => "Cần bạn xem lại",
                    NoticeKind::Finding => "Thông tin mới",
                    NoticeKind::Reminder => "Đến giờ nhắc việc",
                    NoticeKind::Milestone => "Tiến độ",
                },
                &event.summary,
            )));
        }
        if needs_speech {
            self.audio.enqueue(id.clone(), &event.summary);
        }
        self.pending.insert(
            id,
            PendingNotice {
                goal_id: goal_id.into(),
                kind,
                key,
                surface: decision.surface == Surface::None,
                speech: !needs_speech,
                speech_required: needs_speech,
                reminder,
            },
        );
    }
    fn acknowledge(&mut self, id: &str, speech: bool) {
        let Some(pending) = self.pending.get_mut(id) else {
            return;
        };
        let channel = if speech { "speech" } else { "surface" };
        let acknowledged = now_ms()
            .ok()
            .and_then(|now| {
                self.store.lock().ok().and_then(|store| {
                    store
                        .acknowledge(&pending.goal_id, &pending.key, channel, now as f64 / 1000.0)
                        .ok()
                })
            })
            .is_some();
        if !acknowledged {
            return;
        }
        if speech {
            pending.speech = true;
        } else {
            pending.surface = true;
        }
        // A hidden tray window must not retain successfully spoken notices
        // forever. Retire on the selected channel, without forging the other.
        if pending.delivered() {
            if let Some(reminder) = pending.reminder.take()
                && let Ok(mut broker) = self.broker.lock()
            {
                let _ = broker.dismiss_reminder(&reminder);
            }
            self.pending.remove(id);
        }
    }
    fn withdraw_notice(&mut self, id: &str) {
        self.audio.cancel_notice(id);
        self.pending.remove(id);
    }
    fn withdraw_questions(&mut self, goal_id: &str) {
        let ids: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, p)| {
                p.goal_id == goal_id
                    && matches!(p.kind, NoticeKind::Question | NoticeKind::Approval)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.withdraw_notice(&id);
        }
    }
    fn drop_pending_speech(&mut self) {
        let ids: Vec<_> = self.pending.keys().cloned().collect();
        for id in ids {
            self.skip_speech(&id);
        }
    }
    fn skip_speech(&mut self, id: &str) {
        if let Some(pending) = self.pending.get_mut(id) {
            pending.speech_required = false;
            if pending.surface {
                if let Some(reminder) = pending.reminder.take()
                    && let Ok(mut broker) = self.broker.lock()
                {
                    let _ = broker.dismiss_reminder(&reminder);
                }
                self.pending.remove(id);
            }
        }
    }
    fn reminders(&mut self) {
        let Ok(now) = now_ms() else {
            return;
        };
        let due = self
            .broker
            .lock()
            .ok()
            .and_then(|b| b.due_reminders(now).ok())
            .unwrap_or_default();
        for reminder in due.into_iter().take(8) {
            if self.locked {
                continue;
            }
            self.notice(
                &reminder.goal_id,
                NoticeKind::Reminder,
                &reminder.text,
                Some(reminder.id),
            );
        }
    }
}

fn normalized_control(text: &str) -> String {
    let text = text.trim().trim_end_matches(['.', '!', '?']).to_lowercase();
    text.strip_prefix("nora ơi, ")
        .or_else(|| text.strip_prefix("nora ơi "))
        .or_else(|| text.strip_prefix("nora, "))
        .or_else(|| text.strip_prefix("nora "))
        .unwrap_or(&text)
        .trim()
        .to_owned()
}

fn refresh_config(config: &AppConfig, snapshot: &mut AppSnapshot) {
    match ApiConfig::load(&config.llm_config) {
        Ok(config) => {
            let summary = config.summary();
            snapshot.model = summary.model;
            snapshot.reasoning_effort = summary.reasoning_effort;
            snapshot.connection = ConnectionState::Ready;
        }
        Err(_) => {
            snapshot.connection = ConnectionState::Unconfigured;
            snapshot.status_text =
                "Điền API key và model trong file cấu hình local, sau đó tải lại cấu hình.".into();
        }
    }
}
fn refresh_grants(broker: &Broker, snapshot: &mut AppSnapshot) -> Result<()> {
    snapshot.grants = broker
        .roots()?
        .into_iter()
        .filter(|g| !g.revoked)
        .map(|g| GrantView {
            id: g.id,
            label: g.label,
            path: g.path.to_string_lossy().into_owned(),
            can_read: g.permissions.read,
            can_create: g.permissions.create_new,
            allow_cloud: g.permissions.cloud_disclosure,
        })
        .collect();
    snapshot.apps = broker
        .apps()?
        .into_iter()
        .filter(|a| !a.revoked)
        .map(|a| AppView {
            id: a.id,
            label: a.label,
            path: a.executable.to_string_lossy().into_owned(),
        })
        .collect();
    Ok(())
}
fn goal_phase(status: GoalStatus) -> GoalPhase {
    match status {
        GoalStatus::Queued => GoalPhase::Queued,
        GoalStatus::Running => GoalPhase::Running,
        GoalStatus::WaitingInput => GoalPhase::WaitingInput,
        GoalStatus::WaitingPermission => GoalPhase::WaitingPermission,
        GoalStatus::WaitingApproval => GoalPhase::WaitingApproval,
        GoalStatus::Verifying => GoalPhase::Verifying,
        GoalStatus::NeedsReconciliation => GoalPhase::NeedsReconciliation,
        GoalStatus::Completed => GoalPhase::Completed,
        GoalStatus::Failed => GoalPhase::Failed,
        GoalStatus::Cancelled => GoalPhase::Cancelled,
    }
}

fn launch_approved(launch: ApprovedLaunch) -> Result<Value> {
    #[cfg(windows)]
    if nora_platform::session_is_locked()? {
        return Err("session is locked".into());
    }
    let mut command = Command::new(&launch.executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("NORA_LLM_API_KEY")
        .env_remove("OPENAI_API_KEY");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0100_0200);
    }
    let mut child = command.spawn()?;
    let pid = child.id();
    thread::sleep(Duration::from_millis(40));
    if child.try_wait()?.is_some() {
        return Err(
            "application launch was accepted but running state needs reconciliation".into(),
        );
    }
    Ok(
        json!({"pid":pid,"spawn_verified":true,"verification":"process_handle_alive","user_application_outside_nora_job":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_speech_retires_without_fabricating_surface_delivery() {
        let mut p = PendingNotice {
            goal_id: "g".into(),
            kind: NoticeKind::Reminder,
            key: "k".into(),
            surface: false,
            speech: false,
            speech_required: true,
            reminder: None,
        };
        assert!(!p.delivered());
        p.surface = true;
        assert!(
            !p.delivered(),
            "visible text must not retire audio still in flight"
        );
        p.surface = false;
        p.speech = true;
        assert!(
            p.delivered(),
            "spoken reminder must not wait for a hidden window"
        );
        assert!(!p.surface);
        p.speech_required = false;
        assert!(
            !p.delivered(),
            "quiet/skipped audio needs actual surface acknowledgement"
        );
        p.surface = true;
        assert!(p.delivered());
    }
}
