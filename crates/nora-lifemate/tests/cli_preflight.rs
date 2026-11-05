//! Bad evaluation bounds must fail before trying to resolve or load a model.
#![cfg(feature = "whisper")]
use std::{path::Path, process::Command};

#[test]
fn invalid_deadline_placement_and_public_traces_fail_before_model_startup() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_nora-lifemate");
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py");
    for (split, flag, value, expected_error) in [
        (
            "train",
            "--deadline-ms",
            "30001",
            "invalid contest run configuration",
        ),
        (
            "public_test",
            "--trace",
            "must-not-exist",
            "invalid contest run configuration",
        ),
        ("train", "--gpu-layers", "-1", "invalid GPU layer count"),
        ("train", "--gpu-layers", "257", "invalid GPU layer count"),
    ] {
        let output = directory.path().join(format!("{split}.jsonl"));
        let metrics = directory.path().join(format!("{split}-metrics.jsonl"));
        // No server, model, device or ASR options: resolving those first would
        // produce a different error and fail this control, without a model load.
        let result = Command::new(executable)
            .arg("--root")
            .arg(directory.path())
            .args(["--split", split, "--python", executable, "--bridge"])
            .arg(&bridge)
            .arg("--output")
            .arg(&output)
            .arg("--metrics")
            .arg(&metrics)
            .args([flag, value])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains(expected_error)
        );
        assert!(!output.exists());
        assert!(!metrics.exists());
        assert!(!metrics.with_extension("memory.json").exists());
    }
}

#[test]
fn cohort_options_require_both_inputs_and_verified_bytes_before_model_startup() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_nora-lifemate");
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py");
    let cohort = directory.path().join("ids.json");
    std::fs::write(&cohort, br#"["toy_0"]"#).unwrap();
    for (flags, expected_error) in [
        (
            vec!["--cohort-ids", cohort.to_str().unwrap()],
            "must be supplied together",
        ),
        (vec!["--cohort-sha256", "bad"], "must be supplied together"),
        (
            vec![
                "--cohort-ids",
                cohort.to_str().unwrap(),
                "--cohort-sha256",
                "bad",
            ],
            "64 hexadecimal",
        ),
    ] {
        let output = directory.path().join("submission.jsonl");
        let metrics = directory.path().join("metrics.jsonl");
        let result = Command::new(executable)
            .arg("--root")
            .arg(directory.path())
            .args(["--split", "train", "--python", executable, "--bridge"])
            .arg(&bridge)
            .arg("--output")
            .arg(&output)
            .arg("--metrics")
            .arg(&metrics)
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains(expected_error)
        );
        assert!(!output.exists() && !metrics.exists());
    }
}

#[test]
fn adapter_and_prefill_options_fail_before_model_startup_or_output_creation() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_nora-lifemate");
    let bridge = Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py");
    let adapter = directory.path().join("adapter with spaces.gguf");
    std::fs::write(&adapter, b"GGUF\x03\x00\x00\x00fixture").unwrap();
    let path = adapter.to_str().unwrap();
    let wrong_pin = "0".repeat(64);
    for (flags, expected_error) in [
        (
            vec!["--lora", path],
            "LoRA adapter and SHA256 must be supplied together",
        ),
        (
            vec!["--lora-sha256", "bad"],
            "LoRA adapter and SHA256 must be supplied together",
        ),
        (
            vec!["--lora", path, "--lora-sha256", "bad"],
            "64 hexadecimal SHA256 pin",
        ),
        (
            vec!["--lora", path, "--lora-sha256", wrong_pin.as_str()],
            "LoRA asset hash differs",
        ),
        (
            vec!["--prefill-batch", "128"],
            "prefill batch and microbatch must be supplied together",
        ),
        (
            vec!["--prefill-microbatch", "32"],
            "prefill batch and microbatch must be supplied together",
        ),
        (
            vec!["--prefill-batch", "128", "--prefill-microbatch", "256"],
            "microbatch <= batch",
        ),
        (
            vec!["--prefill-batch", "33", "--prefill-microbatch", "32"],
            "powers of two",
        ),
        (
            vec![
                "--prefill-batch",
                "128",
                "--prefill-microbatch",
                "32",
                "--lora",
                path,
                "--lora-sha256",
                wrong_pin.as_str(),
            ],
            "LoRA asset hash differs",
        ),
    ] {
        let output = directory.path().join("submission.jsonl");
        let metrics = directory.path().join("metrics.jsonl");
        let result = Command::new(executable)
            .arg("--root")
            .arg(directory.path())
            .args(["--split", "train", "--python", executable, "--bridge"])
            .arg(&bridge)
            .arg("--output")
            .arg(&output)
            .arg("--metrics")
            .arg(&metrics)
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.contains(expected_error), "{error}");
        assert!(!output.exists() && !metrics.exists());
        assert!(!metrics.with_extension("memory.json").exists());
    }
}
