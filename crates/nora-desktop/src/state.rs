//! Portable UI state and bounds. These are presentation guards, not authority.

use std::collections::{BTreeSet, VecDeque};

use crate::contracts::*;

const MAX_ID_BYTES: usize = 256;
const MAX_LABEL_BYTES: usize = 1024;
const MAX_DETAIL_BYTES: usize = 4096;
const MAX_DECISION_IDS: usize = 128;

pub struct DesktopState {
    pub snapshot: AppSnapshot,
    pub messages: VecDeque<ChatMessage>,
    pub notices: VecDeque<UserNotice>,
    pub previews: VecDeque<ApprovalPreview>,
    pub input: String,
    pub recovered_draft: Option<String>,
    pub last_transcript: Option<String>,
    pub local_error: Option<String>,
    pub should_close: bool,
    decided: BTreeSet<String>,
    decision_order: VecDeque<String>,
    shown_notices: BTreeSet<String>,
    shown_notice_order: VecDeque<String>,
    message_bytes: usize,
}

impl DesktopState {
    pub fn new(initial: AppSnapshot) -> Self {
        Self {
            snapshot: bound_snapshot(initial),
            messages: VecDeque::new(),
            notices: VecDeque::new(),
            previews: VecDeque::new(),
            input: String::new(),
            recovered_draft: None,
            last_transcript: None,
            local_error: None,
            should_close: false,
            decided: BTreeSet::new(),
            decision_order: VecDeque::new(),
            shown_notices: BTreeSet::new(),
            shown_notice_order: VecDeque::new(),
            message_bytes: 0,
        }
    }

    pub fn apply(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Snapshot(snapshot) => self.snapshot = bound_snapshot(*snapshot),
            RuntimeEvent::Message(mut message) => {
                truncate(&mut message.id, MAX_ID_BYTES);
                truncate_visible(&mut message.text, MAX_MESSAGE_BYTES);
                // A repeated authoritative message ID updates in place.
                if let Some(existing) = self.messages.iter_mut().find(|m| m.id == message.id) {
                    self.message_bytes -= existing.text.len();
                    self.message_bytes += message.text.len();
                    *existing = message;
                } else {
                    self.message_bytes += message.text.len();
                    self.messages.push_back(message);
                }
                while self.messages.len() > MAX_MESSAGES || self.message_bytes > MAX_HISTORY_BYTES {
                    if let Some(message) = self.messages.pop_front() {
                        self.message_bytes -= message.text.len();
                    }
                }
            }
            RuntimeEvent::Notice(mut notice) => {
                if notice.id.is_empty() || notice.id.len() > MAX_ID_BYTES {
                    self.local_error =
                        Some("Không thể hiển thị thông báo có mã không hợp lệ.".into());
                    return;
                }
                truncate(&mut notice.title, MAX_LABEL_BYTES);
                truncate(&mut notice.detail, MAX_DETAIL_BYTES);
                if let Some(existing) = self.notices.iter_mut().find(|n| n.id == notice.id) {
                    *existing = notice;
                } else {
                    self.notices.push_back(notice);
                    while self.notices.len() > MAX_NOTICES {
                        self.notices.pop_front();
                    }
                }
            }
            RuntimeEvent::Preview(mut preview) => {
                // Preserve exact action identities; truncating an ID could
                // redirect a decision to another pending action.
                if preview.id.is_empty()
                    || preview.id.len() > MAX_ID_BYTES
                    || preview.goal_id.is_empty()
                    || preview.goal_id.len() > MAX_ID_BYTES
                {
                    self.local_error = Some(
                        "Yêu cầu xác nhận không hợp lệ. NORA cần chuẩn bị lại hành động.".into(),
                    );
                    return;
                }
                // A truncated approval preview must never remain approvable.
                if preview.title.len() > MAX_LABEL_BYTES
                    || preview.description.len() > MAX_DETAIL_BYTES
                    || preview.target.len() > MAX_DETAIL_BYTES
                    || preview
                        .content
                        .as_ref()
                        .is_some_and(|c| c.len() > MAX_PREVIEW_CONTENT_BYTES)
                {
                    preview.can_approve = false;
                }
                truncate(&mut preview.title, MAX_LABEL_BYTES);
                truncate(&mut preview.description, MAX_DETAIL_BYTES);
                truncate(&mut preview.target, MAX_DETAIL_BYTES);
                if let Some(content) = &mut preview.content {
                    truncate(content, MAX_PREVIEW_CONTENT_BYTES);
                }
                if let Some(existing) = self.previews.iter_mut().find(|p| p.id == preview.id) {
                    *existing = preview;
                } else if self.previews.len() < MAX_PREVIEWS {
                    self.previews.push_back(preview);
                } else {
                    // Keep the earlier decisions visible rather than approve
                    // an unseen card that displaced them.
                    self.local_error = Some(
                        "Có quá nhiều yêu cầu xác nhận. Hãy xử lý các yêu cầu đang hiển thị."
                            .into(),
                    );
                }
            }
            RuntimeEvent::ClearPreview { id } => self.previews.retain(|p| p.id != id),
            RuntimeEvent::Transcript { mut text } => {
                truncate(&mut text, MAX_INPUT_BYTES);
                self.last_transcript = Some(text);
            }
            RuntimeEvent::Error { mut message } => {
                truncate(&mut message, MAX_DETAIL_BYTES);
                self.local_error = Some(message);
            }
            RuntimeEvent::InputRejected {
                mut text,
                mut message,
            } => {
                truncate(&mut text, MAX_INPUT_BYTES);
                truncate(&mut message, MAX_DETAIL_BYTES);
                if self.input.is_empty() {
                    self.input = text;
                } else if self.input != text && !text.is_empty() {
                    self.recovered_draft = Some(text);
                }
                self.local_error = Some(message);
            }
            RuntimeEvent::Shutdown => self.should_close = true,
        }
    }

    pub fn can_submit(&self) -> bool {
        matches!(
            self.snapshot.connection,
            ConnectionState::Ready | ConnectionState::Busy
        ) && !matches!(
            self.snapshot.mic,
            MicState::Recording | MicState::Transcribing
        ) && !self.input.trim().is_empty()
            && self.input.len() <= MAX_INPUT_BYTES
            && self.goal_accepts_input()
    }

    pub fn can_start_recording(&self) -> bool {
        // Local status/stop/pause/cancel voice commands remain available during
        // a running goal or network outage. The runtime holds the goal before
        // capture and admits only valid continuations/new goals after ASR.
        matches!(self.snapshot.mic, MicState::Off | MicState::Unavailable)
    }
    pub fn can_import_audio(&self) -> bool {
        matches!(self.snapshot.mic, MicState::Off | MicState::Unavailable)
    }

    pub fn goal_accepts_input(&self) -> bool {
        self.snapshot.active_goal.as_ref().is_none_or(|id| {
            self.snapshot.goals.iter().any(|g| {
                &g.id == id && (g.status == GoalPhase::WaitingInput || g.status.terminal())
            })
        })
    }

    pub fn can_decide(&self, id: &str, approve: bool, now_ms: i64) -> bool {
        !self.decided.contains(id)
            && self
                .previews
                .iter()
                .any(|p| p.id == id && (!approve || (p.can_approve && p.expires_at_ms > now_ms)))
    }

    /// Called only after try_send succeeds. A full channel does not consume the
    /// draft/approval or falsely show that the runtime performed the action.
    pub fn accepted_command(&mut self, command: &UiCommand) {
        match command {
            UiCommand::Submit { .. } => self.input.clear(),
            UiCommand::Approve { preview_id } | UiCommand::Reject { preview_id }
                if self.decided.insert(preview_id.clone()) =>
            {
                self.decision_order.push_back(preview_id.clone());
                while self.decision_order.len() > MAX_DECISION_IDS {
                    if let Some(old) = self.decision_order.pop_front() {
                        self.decided.remove(&old);
                    }
                }
            }
            UiCommand::NoticeShown { id } if self.shown_notices.insert(id.clone()) => {
                self.shown_notice_order.push_back(id.clone());
                while self.shown_notice_order.len() > MAX_DECISION_IDS {
                    if let Some(old) = self.shown_notice_order.pop_front() {
                        self.shown_notices.remove(&old);
                    }
                }
            }
            _ => {}
        }
        if !matches!(
            command,
            UiCommand::NoticeShown { .. } | UiCommand::WindowShown | UiCommand::WindowHidden
        ) {
            self.local_error = None;
        }
    }

    pub fn command_rejected(&mut self, disconnected: bool) {
        self.local_error = Some(if disconnected {
            "Phiên làm việc đã kết thúc. Hãy khởi động lại NORA.".into()
        } else {
            "NORA đang nhận nhiều yêu cầu. Hãy thử lại trong giây lát.".into()
        });
    }

    pub fn decision_pending(&self, id: &str) -> bool {
        self.decided.contains(id)
    }
    pub fn notice_was_shown(&self, id: &str) -> bool {
        self.shown_notices.contains(id)
    }

    pub fn bound_input(&mut self) {
        truncate(&mut self.input, MAX_INPUT_BYTES);
    }

    pub fn history_bytes(&self) -> usize {
        self.message_bytes
    }
    /// Explicit recovery cannot overwrite a draft the user is still editing.
    pub fn restore_recovered_draft(&mut self) -> bool {
        if !self.input.is_empty() {
            return false;
        }
        if let Some(text) = self.recovered_draft.take() {
            self.input = text;
            true
        } else {
            false
        }
    }
}

fn bound_snapshot(mut snapshot: AppSnapshot) -> AppSnapshot {
    truncate(&mut snapshot.model, MAX_LABEL_BYTES);
    truncate(&mut snapshot.reasoning_effort, 32);
    truncate(&mut snapshot.status_text, MAX_DETAIL_BYTES);
    truncate(&mut snapshot.voice_status, MAX_DETAIL_BYTES);
    if let Some(value) = &mut snapshot.resource_summary {
        truncate(value, MAX_LABEL_BYTES);
    }
    if let Some(value) = &mut snapshot.active_goal {
        truncate(value, MAX_ID_BYTES);
    }
    snapshot.goals.truncate(MAX_GOALS);
    snapshot.grants.truncate(MAX_GRANTS);
    snapshot.apps.truncate(MAX_APPS);
    snapshot
        .goals
        .retain(|g| !g.id.is_empty() && g.id.len() <= MAX_ID_BYTES);
    snapshot
        .grants
        .retain(|g| !g.id.is_empty() && g.id.len() <= MAX_ID_BYTES);
    snapshot
        .apps
        .retain(|a| !a.id.is_empty() && a.id.len() <= MAX_ID_BYTES);
    for goal in &mut snapshot.goals {
        truncate(&mut goal.title, MAX_LABEL_BYTES);
        truncate(&mut goal.detail, MAX_DETAIL_BYTES);
    }
    for grant in &mut snapshot.grants {
        truncate(&mut grant.label, MAX_LABEL_BYTES);
        truncate(&mut grant.path, MAX_DETAIL_BYTES);
    }
    for app in &mut snapshot.apps {
        truncate(&mut app.label, MAX_LABEL_BYTES);
        truncate(&mut app.path, MAX_DETAIL_BYTES);
    }
    snapshot
}

pub fn truncate(text: &mut String, limit: usize) {
    if text.len() > limit {
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}

fn truncate_visible(text: &mut String, limit: usize) {
    const SUFFIX: &str = "\n… [Đoạn này đã được rút gọn.]";
    if text.len() > limit {
        truncate(text, limit.saturating_sub(SUFFIX.len()));
        text.push_str(SUFFIX);
    }
}

/// Edge latch for hold-to-talk. Key release follows the accepted Start command,
/// not a microphone snapshot that may arrive after the physical key release.
#[derive(Default)]
#[cfg(any(windows, test))]
pub(crate) struct PushToTalkState {
    pressed: bool,
    armed: bool,
}

#[cfg(any(windows, test))]
impl PushToTalkState {
    pub(crate) fn next(&mut self, pressed: bool, can_start: bool) -> Option<UiCommand> {
        let just_pressed = pressed && !self.pressed;
        self.pressed = pressed;
        if !pressed && self.armed {
            Some(UiCommand::StopRecording)
        } else if just_pressed && can_start {
            Some(UiCommand::StartRecording)
        } else {
            None
        }
    }
    pub(crate) fn started(&mut self) {
        self.armed = true;
    }
    pub(crate) fn stopped(&mut self) {
        self.armed = false;
    }
    pub(crate) fn needs_stop(&self) -> bool {
        self.armed && !self.pressed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview() -> ApprovalPreview {
        ApprovalPreview {
            id: "approval-1".into(),
            goal_id: "goal".into(),
            title: "Tạo ghi chú".into(),
            target: "notes/review.md".into(),
            description: "Tạo tệp mới".into(),
            content: Some("Nội dung".into()),
            expires_at_ms: 100,
            can_approve: true,
        }
    }

    #[test]
    fn bounded_history_keeps_latest_and_replaces_duplicate_ids() {
        let mut state = DesktopState::new(AppSnapshot::default());
        for i in 0..1000 {
            state.apply(RuntimeEvent::Message(ChatMessage::new(
                i.to_string(),
                MessageRole::Assistant,
                "x".repeat(MAX_MESSAGE_BYTES),
            )));
        }
        assert!(state.messages.len() <= MAX_MESSAGES);
        assert!(state.history_bytes() <= MAX_HISTORY_BYTES);
        assert_eq!(state.messages.back().unwrap().id, "999");
        let count = state.messages.len();
        state.apply(RuntimeEvent::Message(ChatMessage::new(
            "999",
            MessageRole::Assistant,
            "Updated",
        )));
        assert_eq!(state.messages.len(), count);
        assert_eq!(state.messages.back().unwrap().text, "Updated");
    }

    #[test]
    fn utf8_bounds_do_not_split_vietnamese() {
        let mut text = "Tiếng Việt".repeat(4000);
        truncate(&mut text, 17);
        assert!(text.len() <= 17);
        assert!(std::str::from_utf8(text.as_bytes()).is_ok());
        let mut state = DesktopState::new(AppSnapshot::default());
        state.input = "ấ".repeat(MAX_INPUT_BYTES);
        state.bound_input();
        assert!(state.input.len() <= MAX_INPUT_BYTES);
    }

    #[test]
    fn approvals_expire_and_are_used_once_in_ui() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.apply(RuntimeEvent::Preview(preview()));
        assert!(state.can_decide("approval-1", true, 99));
        assert!(!state.can_decide("approval-1", true, 100));
        assert!(!state.can_decide("model-fabricated", true, 0));
        state.accepted_command(&UiCommand::Approve {
            preview_id: "approval-1".into(),
        });
        assert!(!state.can_decide("approval-1", true, 0));
        state.apply(RuntimeEvent::ClearPreview {
            id: "approval-1".into(),
        });
        state.apply(RuntimeEvent::Preview(preview()));
        assert!(!state.can_decide("approval-1", true, 0));
    }

    #[test]
    fn truncated_preview_is_never_approvable() {
        let mut state = DesktopState::new(AppSnapshot::default());
        let mut value = preview();
        value.content = Some("x".repeat(MAX_PREVIEW_CONTENT_BYTES + 1));
        state.apply(RuntimeEvent::Preview(value));
        assert!(!state.can_decide("approval-1", true, 0));
        assert!(state.can_decide("approval-1", false, 0));
    }

    #[test]
    fn failed_channel_delivery_preserves_draft_and_decision() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.input = "Tạo ghi chú".into();
        state.apply(RuntimeEvent::Preview(preview()));
        state.command_rejected(false);
        assert_eq!(state.input, "Tạo ghi chú");
        assert!(state.can_decide("approval-1", true, 0));
        state.accepted_command(&UiCommand::Submit {
            text: state.input.clone(),
        });
        assert!(state.input.is_empty());
    }

    #[test]
    fn mic_send_controls_follow_authoritative_state() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.input = "Xin chào".into();
        assert!(!state.can_submit());
        assert!(state.can_start_recording());
        state.snapshot.connection = ConnectionState::Ready;
        assert!(state.can_submit());
        assert!(state.can_start_recording());
        state.snapshot.mic = MicState::Recording;
        assert!(!state.can_submit());
        assert!(!state.can_start_recording());
        assert!(!state.can_import_audio());
        state.snapshot.mic = MicState::Transcribing;
        assert!(!state.can_start_recording());
        assert!(!state.can_import_audio());
        state.snapshot.mic = MicState::Unavailable;
        assert!(state.can_submit());
        assert!(state.can_start_recording());
        assert!(state.can_import_audio());
    }

    #[test]
    fn snapshots_notices_previews_and_transcripts_are_bounded() {
        let initial = AppSnapshot {
            goals: vec![GoalView::new("goal", "A", GoalPhase::Queued); MAX_GOALS + 1],
            grants: vec![
                GrantView {
                    id: "grant".into(),
                    ..Default::default()
                };
                MAX_GRANTS + 1
            ],
            apps: vec![
                AppView {
                    id: "app".into(),
                    ..Default::default()
                };
                MAX_APPS + 1
            ],
            ..Default::default()
        };
        let mut state = DesktopState::new(initial);
        for i in 0..100 {
            let mut p = preview();
            p.id = i.to_string();
            state.apply(RuntimeEvent::Preview(p));
            state.apply(RuntimeEvent::Notice(UserNotice::new(
                i.to_string(),
                NoticeKind::Finding,
                "A",
                "B",
            )));
        }
        state.apply(RuntimeEvent::Transcript {
            text: "a".repeat(MAX_INPUT_BYTES + 1),
        });
        assert_eq!(state.snapshot.goals.len(), MAX_GOALS);
        assert_eq!(state.snapshot.grants.len(), MAX_GRANTS);
        assert_eq!(state.snapshot.apps.len(), MAX_APPS);
        assert_eq!(state.previews.len(), MAX_PREVIEWS);
        assert_eq!(state.notices.len(), MAX_NOTICES);
        assert_eq!(state.last_transcript.unwrap().len(), MAX_INPUT_BYTES);
    }

    #[test]
    fn invalid_action_identity_is_rejected_instead_of_truncated() {
        let mut state = DesktopState::new(AppSnapshot::default());
        let mut value = preview();
        value.id = "p".repeat(MAX_ID_BYTES + 1);
        state.apply(RuntimeEvent::Preview(value));
        assert!(state.previews.is_empty());
        let snapshot = AppSnapshot {
            goals: vec![GoalView::new(
                "g".repeat(MAX_ID_BYTES + 1),
                "invalid",
                GoalPhase::Running,
            )],
            ..Default::default()
        };
        state.apply(RuntimeEvent::Snapshot(Box::new(snapshot)));
        assert!(state.snapshot.goals.is_empty());
    }

    #[test]
    fn waiting_input_allows_continuation_but_running_blocks_new_goal() {
        assert!(GoalPhase::NeedsReconciliation.can_resume());
        let initial = AppSnapshot {
            connection: ConnectionState::Ready,
            active_goal: Some("goal".into()),
            goals: vec![GoalView::new("goal", "A", GoalPhase::Running)],
            ..Default::default()
        };
        let mut state = DesktopState::new(initial);
        state.input = "Thông tin bổ sung".into();
        assert!(!state.can_submit());
        assert!(state.can_start_recording());
        state.snapshot.goals[0].status = GoalPhase::WaitingInput;
        assert!(state.can_submit());
        assert!(state.can_start_recording());
        state.snapshot.goals[0].status = GoalPhase::Completed;
        assert!(state.can_submit());
        state.snapshot.goals.clear();
        assert!(!state.can_submit());
    }

    #[test]
    fn ptt_release_stops_before_recording_snapshot_and_retries_backpressure() {
        let mut ptt = PushToTalkState::default();
        assert_eq!(ptt.next(true, true), Some(UiCommand::StartRecording));
        ptt.started();
        assert_eq!(ptt.next(true, true), None);
        assert_eq!(ptt.next(false, false), Some(UiCommand::StopRecording));
        assert!(ptt.needs_stop());
        // A full channel did not call stopped: release remains pending.
        assert_eq!(ptt.next(false, false), Some(UiCommand::StopRecording));
        ptt.stopped();
        assert_eq!(ptt.next(false, true), None);
    }

    #[test]
    fn notice_ack_is_exact_and_only_consumed_after_channel_acceptance() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.apply(RuntimeEvent::Notice(UserNotice::new(
            "notice",
            NoticeKind::Completion,
            "Done",
            "Verified",
        )));
        state.local_error = Some("Keep this error visible".into());
        assert!(!state.notice_was_shown("notice"));
        state.accepted_command(&UiCommand::NoticeShown {
            id: "notice".into(),
        });
        assert!(state.notice_was_shown("notice"));
        assert!(state.local_error.is_some());
        let count = state.notices.len();
        state.apply(RuntimeEvent::Notice(UserNotice::new(
            "n".repeat(257),
            NoticeKind::Finding,
            "bad",
            "bad",
        )));
        assert_eq!(state.notices.len(), count);
    }

    #[test]
    fn rejected_input_restores_empty_draft_without_adding_chat_history() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.apply(RuntimeEvent::InputRejected {
            text: "Yêu cầu gốc".into(),
            message: "Chưa gửi được".into(),
        });
        assert_eq!(state.input, "Yêu cầu gốc");
        assert!(state.messages.is_empty());
        assert!(state.recovered_draft.is_none());
    }
    #[test]
    fn rejected_input_preserves_new_draft_until_explicit_safe_recovery() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.input = "Bản nháp mới".into();
        state.apply(RuntimeEvent::InputRejected {
            text: "Yêu cầu chưa gửi".into(),
            message: "Chưa gửi được".into(),
        });
        assert_eq!(state.input, "Bản nháp mới");
        assert_eq!(state.recovered_draft.as_deref(), Some("Yêu cầu chưa gửi"));
        assert!(!state.restore_recovered_draft());
        assert_eq!(state.input, "Bản nháp mới");
        state.input.clear();
        assert!(state.restore_recovered_draft());
        assert_eq!(state.input, "Yêu cầu chưa gửi");
        assert!(state.recovered_draft.is_none());
    }
    #[test]
    fn rejected_drafts_are_bounded_and_omitted_from_debug_and_serialization() {
        let mut state = DesktopState::new(AppSnapshot::default());
        state.input = "Giữ lại".into();
        let event = RuntimeEvent::InputRejected {
            text: "ấ".repeat(MAX_INPUT_BYTES),
            message: "x".repeat(MAX_DETAIL_BYTES + 1),
        };
        assert!(!format!("{event:?}").contains('ấ'));
        assert!(serde_json::to_value(&event).unwrap().get("text").is_none());
        state.apply(event);
        assert_eq!(state.input, "Giữ lại");
        let recovered = state.recovered_draft.as_ref().unwrap();
        assert!(
            recovered.len() <= MAX_INPUT_BYTES && std::str::from_utf8(recovered.as_bytes()).is_ok()
        );
        assert!(state.local_error.as_ref().unwrap().len() <= MAX_DETAIL_BYTES);
    }
}
