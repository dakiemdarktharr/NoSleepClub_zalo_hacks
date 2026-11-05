#![allow(dead_code)]
// Exercise the real orchestration and Python transport with original toy
// inputs. Scripted speech/decisions deliberately measure controller behavior,
// never ASR quality, model accuracy or competition scores.
use nora_lifemate::{
    Result,
    runner::{DecisionModel, RunConfig, SampleMetric, Speech, TrainPartition, run},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const SDK: &str = r#"
created=set()
class Env:
    def __init__(self): self.calls=[];self.state=5
    def validate(self,name,args):
        valid=name in ('toy.search','toy.pending','toy.fail','sys.ask_missing_info') or (
            name=='toy.set' and args.get('id')=='id_7' and isinstance(args.get('value'),int))
        return {'ok':valid,'error':None if valid else 'invalid arguments'}
    def call(self,name,args):
        self.calls.append({'name':name,'arguments':args})
        if name=='toy.search': return {'ok':True,'rows':[{'id':'id_7'}]}
        if name=='toy.set': self.state=args['value']
        if name=='toy.pending': self.state+=1;return {'ok':True,'done':False}
        if name=='toy.fail': return {'ok':False,'error':'original toy failure'}
        return {'ok':True,'done':True}
    def get_tool_calls(self): return self.calls
    def get_state_hash(self): return format(self.state,'064x')
def ToolEnv(split,sample_id,root):
    assert sample_id not in created,'opened twice'
    created.add(sample_id)
    return Env()
"#;

fn python() -> PathBuf {
    for executable in ["python", "python3"] {
        if let Ok(result) = Command::new(executable)
            .args(["-c", "import sys;print(sys.executable)"])
            .output()
            && result.status.success()
        {
            return PathBuf::from(String::from_utf8(result.stdout).unwrap().trim());
        }
    }
    panic!("Python 3.10+ is required for environment integration tests");
}

struct Fixture {
    _directory: tempfile::TempDir,
    config: RunConfig,
}
impl Fixture {
    fn new(count: usize, history: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let split = directory.path().join("train");
        std::fs::create_dir_all(split.join("zaic_tools")).unwrap();
        std::fs::create_dir(split.join("audio")).unwrap();
        std::fs::write(split.join("zaic_tools/__init__.py"), SDK).unwrap();
        for audio in ["latest.wav", "earlier.wav"] {
            std::fs::write(split.join("audio").join(audio), b"toy speech input").unwrap();
        }
        let tools = ["toy.search", "toy.set", "toy.pending", "toy.fail"];
        let spec = |name: &str| {
            if name == "toy.set" {
                json!({"name":name,"description":"Original toy tool","kind":"action","parameters":{"type":"object","properties":{"id":{"type":"string","x-ref":"toy.rows"},"value":{"type":"integer"}},"required":["id","value"]}})
            } else {
                json!({"name":name,"description":"Original toy tool","kind":"action","parameters":{"type":"object"}})
            }
        };
        let catalog = json!({"schema_version":"toy-v1","apps":[
            {"id":"toy","tools":tools.iter().map(|t|spec(t)).collect::<Vec<_>>()},
            {"id":"sys","tools":[spec("sys.ask_missing_info")]},
            {"id":"hidden","tools":[spec("hidden.forbidden")]}
        ]});
        std::fs::write(
            split.join("train_tools_catalog.json"),
            serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap();
        let mut lines = String::new();
        for index in 0..count {
            let row = json!({"id":format!("toy_{index}"),"audio":"audio/latest.wav",
                "audio_text":"EXCLUDED_GOLD_TRANSCRIPT", "label":{"tool_calls":"EXCLUDED_GOLD_CALLS"},
                "context":{"current_time":"2026-11-02T09:00:00+07:00","timezone":"Asia/Ho_Chi_Minh",
                    "installed_apps":{"toy":{"tools":tools,"state":"EXCLUDED_STATE"}}},
                "history":if history {json!([
                    {"role":"user","audio":"audio/earlier.wav","audio_text":"EXCLUDED_HISTORY_TRANSCRIPT"},
                    {"role":"assistant","text":"Previous change applied", "tool_calls":[{"name":"toy.set","arguments":{"id":7,"value":5}}],"tool_results":[{"ok":true}]}
                ])} else {json!([])}});
            lines.push_str(&serde_json::to_string(&row).unwrap());
            lines.push('\n');
        }
        std::fs::write(split.join("train.jsonl"), lines).unwrap();
        let config = RunConfig {
            root: directory.path().to_owned(),
            split: "train".into(),
            python: python(),
            bridge_script: Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py"),
            output: directory.path().join("submission.jsonl"),
            metrics: directory.path().join("metrics.jsonl"),
            trace_directory: None,
            limit: None,
            start_index: 0,
            deadline_ms: 30_000,
            partition: TrainPartition::All,
            cohort: None,
        };
        Self {
            _directory: directory,
            config,
        }
    }
    fn records(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.config.output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn metrics(&self) -> Vec<SampleMetric> {
        std::fs::read_to_string(&self.config.metrics)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

#[derive(Default)]
struct ToySpeech {
    paths: Vec<String>,
    fail_first: bool,
}
impl Speech for ToySpeech {
    fn transcribe(&mut self, path: &Path, deadline: Instant) -> Result<String> {
        assert!(deadline > Instant::now());
        self.paths
            .push(path.file_name().unwrap().to_str().unwrap().into());
        if self.fail_first && self.paths.len() == 1 {
            return Err("original toy speech failure".into());
        }
        Ok(format!(
            "Recognized {}",
            path.file_name().unwrap().to_str().unwrap()
        ))
    }
}

struct ScriptedModel {
    replies: VecDeque<std::result::Result<Value, &'static str>>,
    seen: Vec<Vec<Value>>,
}
impl ScriptedModel {
    fn new(replies: Vec<std::result::Result<Value, &'static str>>) -> Self {
        Self {
            replies: replies.into(),
            seen: Vec::new(),
        }
    }
}
impl DecisionModel for ScriptedModel {
    fn complete(
        &mut self,
        messages: &[Value],
        _schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        assert!(!remaining.is_zero());
        self.seen.push(messages.to_vec());
        let decision = self
            .replies
            .pop_front()
            .expect("unexpected planning turn")?;
        Ok(
            json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::to_string(&decision)?}}]}),
        )
    }
}

struct Actor {
    inner: ScriptedModel,
    aliases: bool,
    schemas: Vec<Value>,
}
impl DecisionModel for Actor {
    fn compact_prompt(&self) -> bool {
        true
    }
    fn schema_aliases(&self) -> bool {
        self.aliases
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        self.schemas.push(schema.clone());
        self.inner.complete(messages, schema, remaining)
    }
}
fn decision(calls: Value, finished: bool) -> std::result::Result<Value, &'static str> {
    Ok(json!({"tool_calls":calls,"finished":finished}))
}
fn actor(aliases: bool, replies: Vec<std::result::Result<Value, &'static str>>) -> Actor {
    Actor {
        inner: ScriptedModel::new(replies),
        aliases,
        schemas: vec![],
    }
}

#[test]
fn alias_and_baseline_yield_identical_sdk_calls_receipt_and_grounded_id() {
    let a = Fixture::new(1, false);
    let b = Fixture::new(1, false);
    let make = |enabled| {
        actor(
            enabled,
            vec![
                decision(
                    json!([{"name":if enabled {"toy.search_toy"} else {"toy.search"},"arguments":{}}]),
                    false,
                ),
                decision(
                    json!([{"name":"toy.set","arguments":{"id":"id_7","value":9}}]),
                    true,
                ),
            ],
        )
    };
    let mut first = make(false);
    let mut second = make(true);
    run(&a.config, &mut ToySpeech::default(), &mut first).unwrap();
    run(&b.config, &mut ToySpeech::default(), &mut second).unwrap();
    assert_eq!(a.records(), b.records());
    assert_eq!(
        b.records()[0]["tool_calls"],
        json!([{"name":"toy.search","arguments":{}},{"name":"toy.set","arguments":{"id":"id_7","value":9}}])
    );
    assert_eq!(a.metrics()[0].status, "complete");
    assert_eq!(b.metrics()[0].status, "complete");
    assert!(first.schemas[0].to_string().contains("toy.search"));
    assert!(second.schemas[0].to_string().contains("toy.search_toy"));
    let obs: Value = serde_json::from_str(
        second.inner.seen[1].last().unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        obs["tool_observations"][0]["executed_tool"]["name"],
        "toy.search_toy"
    );
    assert_eq!(
        obs["tool_observations"][0]["result"]["rows"][0]["id"],
        "id_7"
    );
}

#[test]
fn sys_ask_uses_original_tool_name_in_sdk_receipt_and_keeps_param_names() {
    let a = Fixture::new(1, false);
    let b = Fixture::new(1, false);
    let make = |enabled| {
        actor(
            enabled,
            vec![decision(
                json!([{"name":"sys.ask_missing_info","arguments":{"tool_name":if enabled {"toy.search_toy"} else {"toy.search"},"params":["query"]}}]),
                false,
            )],
        )
    };
    let mut first = make(false);
    let mut second = make(true);
    run(&a.config, &mut ToySpeech::default(), &mut first).unwrap();
    run(&b.config, &mut ToySpeech::default(), &mut second).unwrap();
    assert_eq!(a.records(), b.records());
    assert_eq!(
        b.records()[0]["tool_calls"][0]["arguments"],
        json!({"tool_name":"toy.search","params":["query"]})
    );
}

#[test]
fn guessed_id_from_a_schema_alias_does_not_bypass_grounding() {
    let f = Fixture::new(1, false);
    let mut model = actor(
        true,
        vec![
            decision(
                json!([{"name":"toy.set","arguments":{"id":"id_7","value":9}}]),
                false,
            ),
            decision(json!([]), true),
        ],
    );
    run(&f.config, &mut ToySpeech::default(), &mut model).unwrap();
    assert_eq!(f.records()[0]["tool_calls"], json!([]));
    assert_eq!(f.metrics()[0].provenance_refusals, 1);
}

#[test]
fn unavailable_alias_or_ask_target_cannot_reach_sdk() {
    let f = Fixture::new(1, false);
    let mut model = actor(
        true,
        vec![decision(
            json!([{"name":"sys.ask_missing_info","arguments":{"tool_name":"hidden.forbidden","params":["id"]}}]),
            true,
        )],
    );
    run(&f.config, &mut ToySpeech::default(), &mut model).unwrap();
    assert_eq!(f.records()[0]["tool_calls"], json!([]));
    assert_eq!(f.metrics()[0].status, "failed");
}

fn copy_python_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for item in std::fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        let path = item.path();
        let typ = item.file_type().unwrap();
        if typ.is_dir() {
            if item.file_name() != "__pycache__" {
                copy_python_tree(&path, &destination.join(item.file_name()));
            }
        } else if typ.is_file() && path.extension().is_some_and(|v| v == "py") {
            std::fs::copy(&path, destination.join(item.file_name())).unwrap();
        }
    }
}
fn official_synthetic_fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let split = directory.path().join("train");
    let sdk_root = PathBuf::from(
        std::env::var("NORA_ALIAS_TEST_CATALOG_ROOT").expect("private official SDK test root"),
    )
    .join("train");
    std::fs::create_dir_all(split.join("audio")).unwrap();
    std::fs::write(split.join("audio/latest.wav"), b"toy speech input").unwrap();
    copy_python_tree(&sdk_root.join("zaic_tools"), &split.join("zaic_tools"));
    std::fs::copy(
        sdk_root.join("train_tools_catalog.json"),
        split.join("train_tools_catalog.json"),
    )
    .unwrap();
    let sample = json!({"id":"toy_0","audio":"audio/latest.wav","context":{"current_time":"2026-11-02T09:00:00+07:00","timezone":"Asia/Ho_Chi_Minh","installed_apps":{"contacts":{"tools":["contacts.search"]},"sms":{"tools":["sms.send"]}}},"history":[]});
    std::fs::write(split.join("train.jsonl"), format!("{}\n", sample)).unwrap();
    let state_dir = split.join("train_environment/toy_0");
    std::fs::create_dir_all(&state_dir).unwrap();
    let state = json!({"_meta":{"tools":["contacts.search","sms.send"],"current_time":"2026-11-02T09:00:00+07:00","city":"Hanoi"},"contacts":{"contacts":[{"id":"ct_toy_7","name":"An","phone":"0900000001"}]},"sms":{"messages":[]}});
    std::fs::write(state_dir.join("state.json"), state.to_string()).unwrap();
    let config = RunConfig {
        root: directory.path().to_owned(),
        split: "train".into(),
        python: python(),
        bridge_script: Path::new(env!("CARGO_MANIFEST_DIR")).join("python/bridge.py"),
        output: directory.path().join("submission.jsonl"),
        metrics: directory.path().join("metrics.jsonl"),
        trace_directory: None,
        limit: None,
        start_index: 0,
        deadline_ms: 30_000,
        partition: TrainPartition::All,
        cohort: None,
    };
    Fixture {
        _directory: directory,
        config,
    }
}
#[test]
#[ignore = "requires local official SDK and catalog; synthetic state only"]
fn alias_and_baseline_match_in_unchanged_official_sdk_with_synthetic_state() {
    let a = official_synthetic_fixture();
    let b = official_synthetic_fixture();
    let make = |enabled| {
        actor(
            enabled,
            vec![
                decision(
                    json!([{"name":if enabled {"contacts.search_contact"} else {"contacts.search"},"arguments":{"query":"An"}}]),
                    false,
                ),
                decision(
                    json!([{"name":if enabled {"sms.send_sms"} else {"sms.send"},"arguments":{"contact_id":"ct_toy_7","content":"Toy message"}}]),
                    true,
                ),
            ],
        )
    };
    let mut first = make(false);
    let mut second = make(true);
    run(&a.config, &mut ToySpeech::default(), &mut first).unwrap();
    run(&b.config, &mut ToySpeech::default(), &mut second).unwrap();
    assert_eq!(a.records(), b.records());
    assert_eq!(a.metrics()[0].status, "complete");
    assert_eq!(b.metrics()[0].status, "complete");
    assert_eq!(
        b.records()[0]["tool_calls"],
        json!([{"name":"contacts.search","arguments":{"query":"An"}},{"name":"sms.send","arguments":{"contact_id":"ct_toy_7","content":"Toy message"}}])
    );
    let obs: Value = serde_json::from_str(
        second.inner.seen[1].last().unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        obs["tool_observations"][0]["executed_tool"]["name"],
        "contacts.search_contact"
    );
    assert_eq!(
        obs["tool_observations"][0]["result"]["contacts"][0]["id"],
        "ct_toy_7"
    );
}
#[test]
#[ignore = "requires local official SDK and catalog; synthetic state only"]
fn official_sdk_clarification_target_is_the_original_permitted_name() {
    let a = official_synthetic_fixture();
    let b = official_synthetic_fixture();
    let make = |enabled| {
        actor(
            enabled,
            vec![decision(
                json!([{"name":"sys.ask_missing_info","arguments":{"tool_name":if enabled {"sms.send_sms"} else {"sms.send"},"params":["content"]}}]),
                false,
            )],
        )
    };
    let mut first = make(false);
    let mut second = make(true);
    run(&a.config, &mut ToySpeech::default(), &mut first).unwrap();
    run(&b.config, &mut ToySpeech::default(), &mut second).unwrap();
    assert_eq!(a.records(), b.records());
    assert_eq!(b.metrics()[0].status, "complete");
    assert_eq!(
        b.records()[0]["tool_calls"],
        json!([{"name":"sys.ask_missing_info","arguments":{"tool_name":"sms.send","params":["content"]}}])
    );
}
