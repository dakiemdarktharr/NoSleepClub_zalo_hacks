//! Bounded nonblocking UI delivery. Modal native pickers can suspend rendering;
//! permission previews/results must survive a temporarily full UI channel.

use nora_desktop::contracts::RuntimeEvent;
use std::{
    collections::VecDeque,
    sync::mpsc::{SyncSender, TrySendError},
};

const MAX_PENDING: usize = 128;
pub(crate) struct EventOutbox {
    sender: SyncSender<RuntimeEvent>,
    pending: VecDeque<RuntimeEvent>,
}
impl EventOutbox {
    pub fn new(sender: SyncSender<RuntimeEvent>) -> Self {
        Self {
            sender,
            pending: VecDeque::new(),
        }
    }
    pub fn push(&mut self, event: RuntimeEvent) -> bool {
        match &event {
            RuntimeEvent::Snapshot(_) => self
                .pending
                .retain(|e| !matches!(e, RuntimeEvent::Snapshot(_))),
            RuntimeEvent::Preview(preview) => self
                .pending
                .retain(|e| !matches!(e,RuntimeEvent::Preview(p) if p.id==preview.id)),
            RuntimeEvent::ClearPreview { id } => self
                .pending
                .retain(|e| !matches!(e,RuntimeEvent::Preview(p) if p.id==*id)),
            RuntimeEvent::Transcript { .. } => self
                .pending
                .retain(|e| !matches!(e, RuntimeEvent::Transcript { .. })),
            RuntimeEvent::Error { message } => self
                .pending
                .retain(|e| !matches!(e,RuntimeEvent::Error{message:m} if m==message)),
            _ => {}
        }
        if self.pending.len() >= MAX_PENDING {
            if let Some(index) = self
                .pending
                .iter()
                .position(|e| matches!(e, RuntimeEvent::Notice(_)))
            {
                self.pending.remove(index);
            } else {
                return false;
            }
        }
        self.pending.push_back(event);
        self.flush().is_ok()
    }
    pub fn flush(&mut self) -> Result<usize, ()> {
        let mut delivered = 0;
        while let Some(event) = self.pending.pop_front() {
            match self.sender.try_send(event) {
                Ok(()) => delivered += 1,
                Err(TrySendError::Full(event)) => {
                    self.pending.push_front(event);
                    break;
                }
                Err(TrySendError::Disconnected(_)) => return Err(()),
            }
        }
        Ok(delivered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nora_desktop::contracts::{AppSnapshot, ApprovalPreview, ChatMessage, MessageRole};
    #[test]
    fn modal_backpressure_retains_preview_and_answer_until_consumer_returns() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let mut outbox = EventOutbox::new(tx);
        assert!(outbox.push(RuntimeEvent::Snapshot(Box::default())));
        assert!(outbox.push(RuntimeEvent::Preview(ApprovalPreview {
            id: "approval".into(),
            ..Default::default()
        })));
        assert!(outbox.push(RuntimeEvent::Message(ChatMessage::new(
            "answer",
            MessageRole::Assistant,
            "Đã kiểm chứng"
        ))));
        assert!(matches!(rx.recv().unwrap(), RuntimeEvent::Snapshot(_)));
        outbox.flush().unwrap();
        assert!(matches!(rx.recv().unwrap(),RuntimeEvent::Preview(p) if p.id=="approval"));
        outbox.flush().unwrap();
        assert!(matches!(rx.recv().unwrap(),RuntimeEvent::Message(m) if m.id=="answer"));
    }
    #[test]
    fn snapshots_coalesce_and_expired_queued_preview_is_never_reintroduced() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let mut outbox = EventOutbox::new(tx);
        outbox.push(RuntimeEvent::Snapshot(Box::default()));
        for n in 0..1000 {
            outbox.push(RuntimeEvent::Snapshot(Box::new(AppSnapshot {
                status_text: n.to_string(),
                ..Default::default()
            })));
        }
        assert_eq!(outbox.pending.len(), 1);
        outbox.push(RuntimeEvent::Preview(ApprovalPreview {
            id: "old".into(),
            ..Default::default()
        }));
        outbox.push(RuntimeEvent::ClearPreview { id: "old".into() });
        rx.recv().unwrap();
        outbox.flush().unwrap();
        assert!(matches!(rx.recv().unwrap(),RuntimeEvent::Snapshot(s) if s.status_text=="999"));
        outbox.flush().unwrap();
        assert!(matches!(rx.recv().unwrap(),RuntimeEvent::ClearPreview{id} if id=="old"));
    }
}
