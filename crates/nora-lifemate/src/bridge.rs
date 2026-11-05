use crate::{MAX_JSON_BYTES, Result, protocol::ToolCall};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};

pub(crate) fn clean_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command.env_clear();
    for name in [
        "SystemRoot",
        "WINDIR",
        "SystemDrive",
        "LOCALAPPDATA",
        "TEMP",
        "TMP",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

/// CUDA containers supply driver search paths and a writable kernel cache.
/// Only native GPU children receive these variables; the SDK bridge does not
/// inherit them, cloud credentials, proxies, Python paths or user configuration.
pub(crate) fn gpu_command(executable: &Path) -> Command {
    #[cfg(unix)]
    {
        let mut command = clean_command(executable);
        for name in [
            "LD_LIBRARY_PATH",
            "CUDA_VISIBLE_DEVICES",
            "CUDA_CACHE_PATH",
            "NVIDIA_VISIBLE_DEVICES",
            "NVIDIA_DRIVER_CAPABILITIES",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
    }
    #[cfg(not(unix))]
    {
        clean_command(executable)
    }
}

pub struct EnvironmentBridge {
    child: Child,
    input: ChildStdin,
    responses: Receiver<std::result::Result<Value, String>>,
    reader: Option<JoinHandle<()>>,
}

#[derive(Debug, Deserialize)]
pub struct CallOutcome {
    pub executed: bool,
    pub validation: Value,
    pub result: Option<Value>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub tool_calls: Vec<ToolCall>,
    pub state_hash: String,
}

impl EnvironmentBridge {
    pub fn spawn(python: &Path, script: &Path, root: &Path, split: &str) -> Result<Self> {
        let mut child = clean_command(python)
            .args(["-I", "-B", "-u"])
            .arg(script)
            .arg(root)
            .arg(split)
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().ok_or("missing bridge stdin")?;
        let output = child.stdout.take().ok_or("missing bridge stdout")?;
        let (sender, responses) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let result = (|| -> std::result::Result<Value, String> {
                    let n = output
                        .by_ref()
                        .take((MAX_JSON_BYTES + 1) as u64)
                        .read_until(b'\n', &mut line)
                        .map_err(|_| "bridge read failed")?;
                    if n == 0 {
                        return Err("bridge closed its output".into());
                    }
                    if line.len() > MAX_JSON_BYTES {
                        return Err("bridge output exceeds bound".into());
                    }
                    serde_json::from_slice(&line).map_err(|_| "bridge returned invalid JSON".into())
                })();
                let failed = result.is_err();
                if sender.send(result).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            responses,
            reader: Some(reader),
        })
    }
    fn request(&mut self, value: &Value, remaining: Duration) -> Result<Value> {
        if remaining.is_zero() {
            return Err("sample deadline exceeded".into());
        }
        let encoded = serde_json::to_vec(value)?;
        if encoded.len() > MAX_JSON_BYTES {
            return Err("bridge request exceeds bound".into());
        }
        self.input.write_all(&encoded)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        self.responses.recv_timeout(remaining)?.map_err(Into::into)
    }
    pub fn begin(&mut self, id: &str, remaining: Duration) -> Result<()> {
        let result = self.request(&json!({"op":"begin","id":id}), remaining)?;
        if result.get("ready").and_then(Value::as_bool) != Some(true) {
            return Err("bridge did not initialise the sample".into());
        }
        Ok(())
    }
    pub fn call(&mut self, call: &ToolCall, remaining: Duration) -> Result<CallOutcome> {
        Ok(serde_json::from_value(self.request(
            &json!({"op":"call","name":call.name,"arguments":call.arguments}),
            remaining,
        )?)?)
    }
    pub fn finish(&mut self, expected: &[ToolCall], remaining: Duration) -> Result<Receipt> {
        let receipt: Receipt =
            serde_json::from_value(self.request(&json!({"op":"finish"}), remaining)?)?;
        if receipt.tool_calls != expected
            || receipt.state_hash.len() != 64
            || !receipt.state_hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("authoritative call log or state hash is inconsistent".into());
        }
        Ok(receipt)
    }
}
impl Drop for EnvironmentBridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // The reader never blocks on a queue after the receiver is dropped.
        // Do not join while a malicious child output could fill that queue.
        self.reader.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn python() -> std::path::PathBuf {
        for executable in ["python", "python3"] {
            if let Ok(result) = Command::new(executable)
                .args(["-c", "import sys;print(sys.executable)"])
                .output()
                && result.status.success()
            {
                return std::path::PathBuf::from(String::from_utf8(result.stdout).unwrap().trim());
            }
        }
        panic!("Python 3.10+ is required for the official environment adapter tests");
    }
    fn fixture(hash: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("train/zaic_tools");
        std::fs::create_dir_all(&package).unwrap();
        let source = format!(
            r#"
class Env:
    def __init__(self): self.calls=[]
    def validate(self,name,args): return {{"ok": name=="toy.set" and isinstance(args.get("x"),int),"error":None}}
    def call(self,name,args): self.calls.append({{"name":name,"arguments":args}});return {{"ok":True,"done":True}}
    def get_tool_calls(self): return self.calls
    def get_state_hash(self): return "{hash}"
def ToolEnv(split,sample_id,root): return Env()
"#
        );
        std::fs::write(package.join("__init__.py"), source).unwrap();
        directory
    }
    #[test]
    fn authoritative_receipt_survives_rejected_proposals_and_sessions_cannot_reset() {
        let root = fixture(&"a".repeat(64));
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py");
        let mut bridge =
            EnvironmentBridge::spawn(&python(), &script, root.path(), "train").unwrap();
        let timeout = Duration::from_secs(10);
        bridge.begin("toy_1", timeout).unwrap();
        let good = ToolCall {
            name: "toy.set".into(),
            arguments: json!({"x":1}),
        };
        assert!(bridge.call(&good, timeout).unwrap().executed);
        let bad = ToolCall {
            name: "toy.set".into(),
            arguments: json!({"x":"not an integer"}),
        };
        assert!(!bridge.call(&bad, timeout).unwrap().executed);
        let receipt = bridge.finish(&[good], timeout).unwrap();
        assert_eq!(receipt.tool_calls.len(), 1);
        assert!(bridge.begin("toy_1", timeout).is_err());
    }
    #[test]
    fn a_transport_cannot_supply_a_forged_or_incomplete_receipt() {
        let root = fixture("invalid");
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py");
        let mut bridge =
            EnvironmentBridge::spawn(&python(), &script, root.path(), "train").unwrap();
        let timeout = Duration::from_secs(10);
        bridge.begin("toy_2", timeout).unwrap();
        assert!(bridge.finish(&[], timeout).is_err());
    }
}
