//! UI-facing DTOs. Never put API keys, raw provider responses or reasoning here.
//! Bound channel capacities at construction in the launcher.

use std::{
    fmt,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender},
    },
};

use serde::{Deserialize, Serialize};

pub const COMMAND_CAPACITY: usize = 32;
pub const EVENT_CAPACITY: usize = 64;
pub const MAX_INPUT_BYTES: usize = 16_384;
pub const MAX_MESSAGES: usize = 200;
pub const MAX_MESSAGE_BYTES: usize = 16_384;
pub const MAX_HISTORY_BYTES: usize = 524_288;
pub const MAX_NOTICES: usize = 32;
pub const MAX_PREVIEWS: usize = 16;
pub const MAX_PREVIEW_CONTENT_BYTES: usize = 65_536;
pub const MAX_GOALS: usize = 128;
pub const MAX_GRANTS: usize = 64;
pub const MAX_APPS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCommand {
    Submit {
        text: String,
    },
    StartRecording,
    StopRecording,
    ImportAudio {
        path: PathBuf,
    },
    StopSpeech,
    PauseGoal {
        goal_id: String,
    },
    ResumeGoal {
        goal_id: String,
    },
    CancelGoal {
        goal_id: String,
    },
    SetQuiet {
        enabled: bool,
    },
    SetPublicResearch {
        enabled: bool,
    },
    AddFolder {
        path: PathBuf,
        can_read: bool,
        can_create: bool,
        allow_cloud: bool,
    },
    RevokeFolder {
        id: String,
    },
    RegisterApp {
        path: PathBuf,
        label: String,
    },
    RevokeApp {
        id: String,
    },
    Approve {
        preview_id: String,
    },
    Reject {
        preview_id: String,
    },
    RefreshConfig,
    /// Stop/discard capture; hiding is not a request to submit an utterance.
    WindowHidden,
    WindowShown,
    NoticeShown {
        id: String,
    },
    Quit,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Unconfigured,
    Ready,
    Busy,
    Offline,
    Error,
}

impl ConnectionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unconfigured => "Cần cấu hình",
            Self::Ready => "Sẵn sàng",
            Self::Busy => "Đang làm việc",
            Self::Offline => "Mất kết nối",
            Self::Error => "Cần kiểm tra kết nối",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MicState {
    #[default]
    Off,
    Recording,
    Transcribing,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalPhase {
    #[default]
    Queued,
    Running,
    WaitingInput,
    WaitingPermission,
    WaitingApproval,
    Paused,
    Verifying,
    NeedsReconciliation,
    Completed,
    Failed,
    Cancelled,
}

impl GoalPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Đang chờ",
            Self::Running => "Đang thực hiện",
            Self::WaitingInput => "Cần bổ sung thông tin",
            Self::WaitingPermission => "Cần cấp quyền",
            Self::WaitingApproval => "Chờ bạn xác nhận",
            Self::Paused => "Đã tạm dừng",
            Self::Verifying => "Đang kiểm tra kết quả",
            Self::NeedsReconciliation => "Cần kiểm tra trạng thái",
            Self::Completed => "Đã hoàn thành",
            Self::Failed => "Chưa hoàn thành",
            Self::Cancelled => "Đã hủy",
        }
    }

    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_pause(self) -> bool {
        matches!(self, Self::Running | Self::Queued)
    }

    pub fn can_resume(self) -> bool {
        matches!(
            self,
            Self::Paused | Self::WaitingInput | Self::WaitingPermission | Self::NeedsReconciliation
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalView {
    pub id: String,
    pub title: String,
    pub status: GoalPhase,
    pub detail: String,
}

impl GoalView {
    pub fn new(id: impl Into<String>, title: impl Into<String>, status: GoalPhase) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status,
            detail: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrantView {
    pub id: String,
    pub label: String,
    pub path: String,
    pub can_read: bool,
    pub can_create: bool,
    pub allow_cloud: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppView {
    pub id: String,
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSnapshot {
    pub model: String,
    pub reasoning_effort: String,
    pub connection: ConnectionState,
    pub mic: MicState,
    pub speaking: bool,
    pub quiet: bool,
    pub public_research: bool,
    pub status_text: String,
    pub voice_status: String,
    pub resource_summary: Option<String>,
    pub active_goal: Option<String>,
    pub goals: Vec<GoalView>,
    pub grants: Vec<GrantView>,
    pub apps: Vec<AppView>,
}

impl Default for AppSnapshot {
    fn default() -> Self {
        Self {
            model: "gpt-6-luna".into(),
            reasoning_effort: "xhigh".into(),
            connection: ConnectionState::Unconfigured,
            mic: MicState::Off,
            speaking: false,
            quiet: false,
            public_research: false,
            status_text: "Sẵn sàng khi bạn cần.".into(),
            voice_status: "Microphone đang tắt.".into(),
            resource_summary: None,
            active_goal: None,
            goals: Vec::new(),
            grants: Vec::new(),
            apps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub text: String,
}

impl ChatMessage {
    pub fn new(id: impl Into<String>, role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    Milestone,
    Finding,
    Blocker,
    Permission,
    Completion,
    Reminder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserNotice {
    pub id: String,
    pub kind: NoticeKind,
    pub title: String,
    pub detail: String,
}

impl UserNotice {
    pub fn new(
        id: impl Into<String>,
        kind: NoticeKind,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalPreview {
    pub id: String,
    pub goal_id: String,
    pub title: String,
    pub description: String,
    pub target: String,
    pub content: Option<String>,
    pub expires_at_ms: i64,
    /// Runtime-authoritative permission plus local UI completeness/expiry checks.
    pub can_approve: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    Snapshot(Box<AppSnapshot>),
    Message(ChatMessage),
    Notice(UserNotice),
    Preview(ApprovalPreview),
    ClearPreview {
        id: String,
    },
    Transcript {
        text: String,
    },
    Error {
        message: String,
    },
    /// UI-local recovery only: rejected user input may contain a credential.
    /// The typed in-process channel carries it; generic serialization omits it.
    InputRejected {
        #[serde(skip_serializing)]
        text: String,
        message: String,
    },
    Shutdown,
}

impl fmt::Debug for RuntimeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Snapshot(_) => "RuntimeEvent::Snapshot(..)",
            Self::Message(_) => "RuntimeEvent::Message(..)",
            Self::Notice(_) => "RuntimeEvent::Notice(..)",
            Self::Preview(_) => "RuntimeEvent::Preview(..)",
            Self::ClearPreview { .. } => "RuntimeEvent::ClearPreview(..)",
            Self::Transcript { .. } => "RuntimeEvent::Transcript(..)",
            Self::Error { .. } => "RuntimeEvent::Error(..)",
            Self::InputRejected { .. } => "RuntimeEvent::InputRejected(<redacted>)",
            Self::Shutdown => "RuntimeEvent::Shutdown",
        })
    }
}

/// The UI publishes this once initialized. Clone it into runtime producers;
/// call wake after delivering an event. It carries no native window handle.
#[derive(Clone)]
pub struct UiWake(Arc<dyn Fn() + Send + Sync>);

impl UiWake {
    pub fn new(callback: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }
    pub fn wake(&self) {
        (self.0)();
    }
}

impl fmt::Debug for UiWake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UiWake(..)")
    }
}

pub struct UiChannels {
    pub commands: SyncSender<UiCommand>,
    pub events: Receiver<RuntimeEvent>,
    pub wake_ready: Option<SyncSender<UiWake>>,
}

impl UiChannels {
    pub fn new(commands: SyncSender<UiCommand>, events: Receiver<RuntimeEvent>) -> Self {
        Self {
            commands,
            events,
            wake_ready: None,
        }
    }
    pub fn with_wake_sender(mut self, wake_ready: SyncSender<UiWake>) -> Self {
        self.wake_ready = Some(wake_ready);
        self
    }
}
