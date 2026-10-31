//! Opt-in billable integration regression using the documented synthetic
//! "Xin chào, tôi là Nora" WAV. Never opens a microphone or grants any root.
use crate::{Result, backend::AppSession, config::AppConfig, new_id};
use nora_desktop::contracts::{GoalPhase, RuntimeEvent, UiCommand};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
pub fn run(config: &AppConfig, wav: &Path) -> Result<Value> {
    let started = Instant::now();
    let mut isolated = config.clone();
    isolated.state_dir = config.state_dir.join(new_id("voice-followup-check")?);
    isolated.quiet = true;
    isolated.public_research = false;
    let driver = AppSession::prepare(isolated.clone())?.into_driver()?;
    driver.commands.send(UiCommand::Submit{text:"Hãy hỏi tôi tên tôi là gì bằng ask_user, đợi tôi trả lời rồi chào tôi theo tên đó. Chưa biết tên thì không tự đoán.".into()})?;
    let mut original = None;
    let mut imported = false;
    let mut transcript_matches = false;
    let mut completed = false;
    while started.elapsed() < Duration::from_secs(180) {
        match driver
            .channels
            .events
            .recv_timeout(Duration::from_millis(200))
        {
            Ok(RuntimeEvent::Snapshot(s)) => {
                if s.goals.len() > 1 {
                    return Err("spoken follow-up created an unintended second goal".into());
                }
                if let Some(id) = &s.active_goal {
                    if original.as_ref().is_some_and(|old| old != id) {
                        return Err("spoken follow-up changed goal identity".into());
                    }
                    original = Some(id.clone());
                }
                if let Some(id) = &original
                    && let Some(goal) = s.goals.iter().find(|g| &g.id == id)
                {
                    if goal.status == GoalPhase::WaitingInput && !imported {
                        driver.commands.send(UiCommand::ImportAudio {
                            path: wav.to_owned(),
                        })?;
                        imported = true;
                    }
                    if goal.status == GoalPhase::Completed && s.active_goal.is_none() {
                        completed = true;
                        break;
                    }
                    if matches!(
                        goal.status,
                        GoalPhase::Failed | GoalPhase::NeedsReconciliation | GoalPhase::Cancelled
                    ) {
                        return Err("spoken follow-up regression did not complete".into());
                    }
                }
            }
            Ok(RuntimeEvent::Transcript { text }) => {
                let text = text.to_lowercase();
                transcript_matches = text.contains("xin chào") && text.contains("nora");
            }
            Ok(RuntimeEvent::Error { .. } | RuntimeEvent::InputRejected { .. }) => {
                return Err("spoken follow-up was rejected".into());
            }
            Ok(RuntimeEvent::Preview(p)) => {
                driver
                    .commands
                    .send(UiCommand::Reject { preview_id: p.id })?;
                return Err("voice check refused an unexpected action preview".into());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("voice check supervisor stopped".into());
            }
            _ => {}
        }
    }
    let id = original.ok_or("voice check did not admit a goal")?;
    if !completed || !imported || !transcript_matches {
        return Err("voice check did not verify expected synthetic follow-up".into());
    }
    let store = nora_store::Store::open(isolated.state_dir.join("goals.sqlite"))?;
    let goal = store.goal(&id)?;
    let result = store
        .result(&id)?
        .ok_or("voice check result is not durable")?;
    if !result.to_lowercase().contains("nora") || store.has_pending_input(&id)? {
        return Err("voice check did not consume the trusted correction".into());
    }
    driver.shutdown()?;
    Ok(
        json!({"passed":true,"synthetic_only":true,"microphone_opened":false,"original_goal_preserved":true,"asr_fixture_transcript_matches":transcript_matches,"trusted_correction_marker_consumed":true,"goal_status":goal.status,"usage":goal.checkpoint.get("usage"),"wall_ms":started.elapsed().as_secs_f64()*1000.0}),
    )
}
