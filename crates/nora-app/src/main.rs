#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use nora_runtime::{Result, config::AppConfig};
use serde_json::json;
use std::{path::PathBuf, sync::atomic::AtomicBool};

fn main() {
    if let Err(error) = run() {
        eprintln!("NORA: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "--voice-worker") {
        if args.len() != 2 || !nora_platform::sandbox::is_app_container()? {
            return Err("native voice worker requires the NORA AppContainer launcher".into());
        }
        return nora_runtime::audio_worker::run_audio_worker(&PathBuf::from(&args[1]));
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "--sandbox-probe") {
        if args.len() != 2 {
            return Err("invalid confinement probe arguments".into());
        }
        return nora_runtime::diagnostics::sandbox_child_probe(&PathBuf::from(&args[1]));
    }
    let executable = std::env::current_exe()?;
    let mut config = AppConfig::defaults(&executable)?;
    let mut command = None;
    let mut metrics = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--metrics" if index + 1 < args.len() => {
                if metrics.replace(PathBuf::from(&args[index + 1])).is_some() {
                    return Err("select one metrics destination".into());
                }
                index += 2;
            }
            "--config" if index + 1 < args.len() => {
                config = AppConfig::load(&PathBuf::from(&args[index + 1]))?;
                index += 2;
            }
            "--doctor" | "--api-probe" | "--live-check" | "--resource-probe"
            | "--sandbox-check" => {
                if command.replace(args[index].as_str()).is_some() {
                    return Err("select one command".into());
                }
                index += 1;
            }
            "--help" => {
                println!(
                    "NORA Windows desktop assistant\n  nora-app [--config APP.json] [--metrics NEW.json]\n  nora-app --doctor [--config APP.json]\n  nora-app --api-probe [--config APP.json]\n  nora-app --live-check [--config APP.json]\n  nora-app --sandbox-check [--config APP.json]\n  nora-app --resource-probe\nAPI probe/live check are explicit billable requests."
                );
                return Ok(());
            }
            _ => return Err("unsupported command or missing arguments".into()),
        }
    }
    #[cfg(windows)]
    let memory_guard = std::sync::Arc::new(nora_platform::BudgetJob::install(
        if command == Some("--resource-probe") {
            64_000_000
        } else {
            nora_platform::RAM_LIMIT_BYTES
        },
    )?);
    #[cfg(windows)]
    let _measurement = metrics
        .as_deref()
        .map(|path| nora_platform::Measurement::start(memory_guard.clone(), path))
        .transpose()?;
    #[cfg(not(windows))]
    if metrics.is_some() {
        return Err("Windows process metrics required".into());
    }
    match command {
        Some("--live-check") => {
            println!("{}", nora_runtime::live_check::run(&config)?);
            Ok(())
        }
        Some("--sandbox-check") => {
            println!(
                "{}",
                nora_runtime::diagnostics::sandbox_check(
                    &executable,
                    &config.state_dir.join("confinement-tests")
                )?
            );
            Ok(())
        }
        Some("--resource-probe") => {
            let mut allocation = Vec::<u8>::new();
            let allocation_refused = allocation.try_reserve_exact(96_000_000).is_err();
            #[cfg(windows)]
            println!(
                "{}",
                json!({"allocation_refused":allocation_refused,"job":memory_guard.snapshot()?,"process":nora_platform::process_memory(std::process::id())?,"note":"PeakJobMemoryUsed may include a rejected allocation attempt; actual process counters are reported separately."})
            );
            if !allocation_refused {
                return Err("OS memory cap negative control failed".into());
            }
            Ok(())
        }
        Some("--api-probe") => {
            let cfg = nora_llm::ApiConfig::load(&config.llm_config)?;
            let summary = cfg.summary();
            let client = nora_llm::ResponsesClient::new(cfg)?;
            let response = client.respond(
                &nora_llm::TurnRequest::user(
                    "This is a synthetic connectivity check. Reply exactly NORA_OK, without tools.",
                    "NORA_OK",
                ),
                &AtomicBool::new(false),
            )?;
            println!(
                "{}",
                json!({"config":summary,"status":response.status,"usage":response.usage,"expected_marker":response.text.trim()=="NORA_OK","tools_returned":response.tool_calls.len(),"opaque_response_not_logged":true})
            );
            if response.status != nora_llm::ResponseStatus::Completed
                || response.text.trim() != "NORA_OK"
            {
                return Err("live LLM probe did not produce the expected complete marker".into());
            }
            Ok(())
        }
        Some("--doctor") => {
            let api = nora_llm::ApiConfig::load(&config.llm_config).map(|c| c.summary());
            let assets = nora_runtime::assets::VoiceAssets::new(config.asset_root.clone());
            println!(
                "{}",
                json!({"windows_first":cfg!(windows),"model_configuration":api.as_ref().ok(),"llm_configuration_valid":api.is_ok(),"tts_assets_verified":assets.verify_tts().is_ok(),"asr_assets_verified":assets.verify_asr().is_ok(),"vad_assets_verified":assets.verify_vad().is_ok(),"ram_limit_bytes":nora_platform::RAM_LIMIT_BYTES,"vram_limit_bytes":nora_platform::VRAM_LIMIT_BYTES,"disk_limit_bytes":nora_platform::DISK_LIMIT_BYTES,"network_called":false})
            );
            Ok(())
        }
        None => {
            #[cfg(windows)]
            let _instance = nora_platform::SingleInstance::acquire()?;
            nora_runtime::backend::AppSession::prepare(config)?.run()
        }
        _ => Err("unsupported command".into()),
    }
}
