//! Explicit opt-in, billable integration checks on newly-created synthetic
//! data. Automatic approval is restricted to the exact test-only note below.
use crate::{
    Result,
    backend::{AppDriver, AppSession},
    config::AppConfig,
    new_id,
};
use nora_desktop::contracts::{GoalPhase, RuntimeEvent, UiCommand};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

pub fn run(config: &AppConfig) -> Result<Value> {
    let run = config.state_dir.join(new_id("live-check")?);
    fs::create_dir_all(&run)?;
    let fixture = run.join("synthetic-documents");
    fs::create_dir(&fixture)?;
    let mut isolated = config.clone();
    isolated.state_dir = run.join("state");
    isolated.quiet = true;
    isolated.public_research = false;
    let driver = AppSession::prepare(isolated.clone())?.into_driver()?;
    driver.commands.send(UiCommand::AddFolder {
        path: fixture.clone(),
        can_read: true,
        can_create: true,
        allow_cloud: true,
    })?;
    let ready = Instant::now();
    loop {
        if ready.elapsed() > Duration::from_secs(10) {
            return Err("live check grant setup timed out".into());
        }
        if let Ok(RuntimeEvent::Snapshot(s)) = driver
            .channels
            .events
            .recv_timeout(Duration::from_millis(200))
            && s.grants.len() == 1
        {
            break;
        }
    }
    driver.commands.send(UiCommand::Submit {
        text: "Hãy dùng công cụ máy tính để tính (17*23)+11 và cho biết kết quả bằng chữ số."
            .into(),
    })?;
    let calc = wait(&driver, false, false)?;
    let store = nora_store::Store::open(isolated.state_dir.join("goals.sqlite"))?;
    let calc_text = store
        .result(&calc)?
        .ok_or("calculator result was not durable")?;
    if !calc_text.contains("402") {
        return Err("calculator result mismatch".into());
    }
    driver.commands.send(UiCommand::Submit{text:"Tạo file mới nora-live-check.md trong thư mục đã cấp quyền synthetic-documents, nội dung chính xác là KIEM_CHUNG_NORA. Không thêm tiêu đề hay nội dung khác. Đề xuất để tôi duyệt.".into()})?;
    let note = wait(&driver, true, false)?;
    if fs::read_to_string(fixture.join("nora-live-check.md"))?.trim() != "KIEM_CHUNG_NORA" {
        return Err("verified note readback mismatch".into());
    }
    driver.commands.send(UiCommand::Submit{text:"Nhắc tôi làm việc. Tôi chưa chọn thời gian, hãy hỏi tôi cần nhắc lúc nào; không tự chọn thời gian và chưa tạo lời nhắc.".into()})?;
    let cancelled = wait(&driver, false, true)?;
    let mut cases = vec![
        ("calculator", calc),
        ("approved_note", note),
        ("missing_input_then_cancel", cancelled),
    ];
    if config.public_research {
        driver
            .commands
            .send(UiCommand::SetPublicResearch { enabled: true })?;
        let id = research_and_note(&driver, &fixture)?;
        cases.push(("public_research_then_approved_note", id));
    }
    let rows=cases.into_iter().map(|(kind,id)|{
        let goal=store.goal(&id)?;Ok(json!({"scenario":kind,"goal_id":id,"status":goal.status,"usage":goal.checkpoint.get("usage"),"tools_used":goal.checkpoint.get("tools_used"),"durable_result":store.result(&goal.id)?.is_some()}))
    }).collect::<Result<Vec<_>>>()?;
    driver.shutdown()?;
    let reopened = AppSession::prepare(isolated)?.into_driver()?;
    let mut recovered = false;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if let Ok(RuntimeEvent::Snapshot(s)) = reopened
            .channels
            .events
            .recv_timeout(Duration::from_millis(200))
        {
            recovered = s
                .goals
                .iter()
                .any(|g| g.status == GoalPhase::Completed && g.detail.contains("402"));
            break;
        }
    }
    reopened.shutdown()?;
    if !recovered {
        return Err("desktop restart did not recover final answer".into());
    }
    Ok(
        json!({"passed":true,"synthetic_only":true,"model":"exact_local_configuration","scenarios":rows,"restarted_answer_recovered":recovered,"note_readback_verified":true,"api_requests_are_billable":true,"voice_gui_and_gpu_not_covered_by_this_check":true}),
    )
}
fn research_and_note(driver: &AppDriver, fixture: &Path) -> Result<String> {
    const TOPIC: &str =
        "Find the official documentation for Rust whisper-rs and the meaning of GGML_AVX2.";
    driver.commands.send(UiCommand::Submit{text:format!("Dùng research_public với topic chính xác: {TOPIC}\nSau khi có nguồn, tạo file mới nora-research-check.md trong thư mục synthetic-documents chứa tóm tắt ngắn bằng tiếng Việt và các URL nguồn. Đề xuất để tôi duyệt, không tự nhận đã ghi nếu chưa có receipt.")})?;
    let start = Instant::now();
    let mut id = None;
    let mut query_approved = false;
    let mut note_approved = false;
    while start.elapsed() < Duration::from_secs(240) {
        match driver
            .channels
            .events
            .recv_timeout(Duration::from_millis(200))
        {
            Ok(RuntimeEvent::Snapshot(s)) => {
                if let Some(active) = &s.active_goal {
                    id = Some(active.clone());
                }
                if let Some(id) = &id
                    && let Some(goal) = s.goals.iter().find(|g| &g.id == id)
                    && s.active_goal.is_none()
                {
                    if goal.status == GoalPhase::Completed && query_approved && note_approved {
                        let content = fs::read_to_string(fixture.join("nora-research-check.md"))?;
                        if content.contains("https://") {
                            return Ok(id.clone());
                        }
                        return Err("research note lacks source links".into());
                    }
                    if matches!(
                        goal.status,
                        GoalPhase::Completed
                            | GoalPhase::Failed
                            | GoalPhase::Cancelled
                            | GoalPhase::NeedsReconciliation
                    ) {
                        return Err("research workflow did not verify completion".into());
                    }
                }
            }
            Ok(RuntimeEvent::Preview(p)) => {
                let query = p.id.starts_with("research-preview-")
                    && p.content.as_deref() == Some(TOPIC)
                    && p.can_approve;
                let note = Path::new(&p.target)
                    .file_name()
                    .is_some_and(|s| s == "nora-research-check.md")
                    && p.content
                        .as_ref()
                        .is_some_and(|s| s.len() <= 8000 && s.contains("https://"))
                    && p.can_approve;
                if !query && !note {
                    driver
                        .commands
                        .send(UiCommand::Reject { preview_id: p.id })?;
                    return Err("research check refused unexpected proposal".into());
                }
                driver
                    .commands
                    .send(UiCommand::Approve { preview_id: p.id })?;
                query_approved |= query;
                note_approved |= note;
            }
            Ok(RuntimeEvent::Error { .. } | RuntimeEvent::InputRejected { .. }) => {
                return Err("research check runtime error".into());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("research check supervisor stopped".into());
            }
            _ => {}
        }
    }
    Err("research workflow deadline exceeded".into())
}
fn wait(driver: &AppDriver, approve_note: bool, cancel_on_input: bool) -> Result<String> {
    let start = Instant::now();
    let mut id = None;
    let mut approved = false;
    let mut cancel_sent = false;
    while start.elapsed() < Duration::from_secs(240) {
        match driver
            .channels
            .events
            .recv_timeout(Duration::from_millis(200))
        {
            Ok(RuntimeEvent::Snapshot(s)) => {
                if let Some(active) = &s.active_goal {
                    id = Some(active.clone());
                }
                if let Some(id) = &id
                    && let Some(goal) = s.goals.iter().find(|g| &g.id == id)
                {
                    if goal.status == GoalPhase::WaitingInput && cancel_on_input && !cancel_sent {
                        driver.commands.send(UiCommand::CancelGoal {
                            goal_id: id.clone(),
                        })?;
                        cancel_sent = true;
                    }
                    if s.active_goal.is_none() {
                        if (goal.status == GoalPhase::Completed && (!approve_note || approved))
                            || (goal.status == GoalPhase::Cancelled && cancel_sent)
                        {
                            return Ok(id.clone());
                        }
                        if matches!(
                            goal.status,
                            GoalPhase::Failed
                                | GoalPhase::NeedsReconciliation
                                | GoalPhase::WaitingPermission
                        ) {
                            return Err("live check goal failed or needs reconciliation".into());
                        }
                    }
                }
            }
            Ok(RuntimeEvent::Preview(p)) => {
                let exact = approve_note
                    && Path::new(&p.target)
                        .file_name()
                        .is_some_and(|s| s == "nora-live-check.md")
                    && p.content
                        .as_deref()
                        .is_some_and(|s| s.trim() == "KIEM_CHUNG_NORA")
                    && p.can_approve;
                if !exact {
                    driver
                        .commands
                        .send(UiCommand::Reject { preview_id: p.id })?;
                    return Err("live check refused unexpected preview".into());
                }
                driver
                    .commands
                    .send(UiCommand::Approve { preview_id: p.id })?;
                approved = true;
            }
            Ok(RuntimeEvent::Error { .. } | RuntimeEvent::InputRejected { .. }) => {
                return Err("live check runtime rejected operation".into());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("live check runtime stopped".into());
            }
            _ => {}
        }
    }
    Err("live check exceeded scenario deadline".into())
}
