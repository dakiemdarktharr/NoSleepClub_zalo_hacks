//! Real Windows cleanup test using only this tiny test binary as supervisor,
//! child and grandchild. No model, user application, elevated right or API key.
#![cfg(windows)]

use std::{
    io,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};

fn spawn_helper(mode: &str, directory: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "helper_process",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("NORA_JOB_TEST_MODE", mode)
        .env("NORA_JOB_TEST_DIRECTORY", directory)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

fn sleep_until_killed() -> ! {
    loop {
        thread::sleep(Duration::from_secs(30));
    }
}

#[test]
#[allow(
    clippy::zombie_processes,
    reason = "Descendants deliberately remain alive until Windows Job cleanup is tested"
)]
fn helper_process() {
    let Ok(mode) = std::env::var("NORA_JOB_TEST_MODE") else {
        return;
    };
    let directory = PathBuf::from(std::env::var_os("NORA_JOB_TEST_DIRECTORY").unwrap());
    if mode == "leaf" {
        sleep_until_killed();
    }
    if mode == "middle" {
        let child = spawn_helper("leaf", &directory);
        std::fs::write(directory.join("grandchild.pid"), child.id().to_string()).unwrap();
        sleep_until_killed();
    }
    let job = nora_platform::BudgetJob::install_contest().unwrap();
    assert_eq!(job.snapshot().unwrap().limit_bytes, 64_000_000_000);
    drop(job);
    // Dropping ordinary references must not kill the supervisor prematurely.
    let job = nora_platform::BudgetJob::install_contest().unwrap();
    let child = spawn_helper("middle", &directory);
    std::fs::write(directory.join("child.pid"), child.id().to_string()).unwrap();
    wait_for_file(&directory.join("grandchild.pid"));
    drop(job);
    std::fs::write(directory.join("ready"), b"ready").unwrap();
    if mode == "normal" {
        // Parent releases this gate after it holds handles to both descendants.
        wait_for_file(&directory.join("exit"));
        return;
    }
    sleep_until_killed();
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.is_file() {
        assert!(Instant::now() < deadline, "helper did not become ready");
        thread::sleep(Duration::from_millis(10));
    }
}

fn open_wait_handle(path: &Path) -> OwnedHandle {
    let pid = std::fs::read_to_string(path).unwrap().parse().unwrap();
    // SAFETY: recorded PID belongs to this test's child, query-only wait access,
    // no inheritance. Holding the handle prevents PID-reuse ambiguity.
    let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    assert!(!raw.is_null(), "{}", io::Error::last_os_error());
    // SAFETY: successful OpenProcess returned one owned handle.
    unsafe { OwnedHandle::from_raw_handle(raw) }
}

fn exercise(mode: &str) {
    struct TestSupervisor(Child);
    impl Drop for TestSupervisor {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let mut supervisor = TestSupervisor(spawn_helper(mode, directory.path()));
    let started = Instant::now();
    while !directory.path().join("ready").is_file() {
        assert!(
            supervisor.0.try_wait().unwrap().is_none(),
            "supervisor exited before ready"
        );
        if started.elapsed() > Duration::from_secs(20) {
            let _ = supervisor.0.kill();
            let _ = supervisor.0.wait();
            panic!("supervisor startup timed out");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let child = open_wait_handle(&directory.path().join("child.pid"));
    let grandchild = open_wait_handle(&directory.path().join("grandchild.pid"));
    // SAFETY: both handles stay owned and live throughout each wait.
    assert_eq!(
        unsafe { WaitForSingleObject(child.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    assert_eq!(
        unsafe { WaitForSingleObject(grandchild.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    if mode == "normal" {
        std::fs::write(directory.path().join("exit"), b"exit").unwrap();
        assert!(supervisor.0.wait().unwrap().success());
    } else {
        supervisor.0.kill().unwrap();
        supervisor.0.wait().unwrap();
    }
    // SAFETY: owned query-only handles; bounded wait, no PID-based killing.
    assert_eq!(
        unsafe { WaitForSingleObject(child.as_raw_handle(), 5000) },
        WAIT_OBJECT_0
    );
    assert_eq!(
        unsafe { WaitForSingleObject(grandchild.as_raw_handle(), 5000) },
        WAIT_OBJECT_0
    );
}

#[test]
fn contest_job_kills_owned_descendants_after_normal_exit() {
    exercise("normal");
}

#[test]
fn contest_job_kills_owned_descendants_after_abrupt_exit() {
    exercise("abrupt");
}
