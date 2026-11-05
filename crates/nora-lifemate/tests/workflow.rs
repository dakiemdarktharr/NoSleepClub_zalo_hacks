//! Exercise the real orchestration and Python transport with original toy
//! inputs. Scripted speech/decisions deliberately measure controller behavior,
//! never ASR quality, model accuracy or competition scores.
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
        valid=name in ('toy.lookup','toy.pending','toy.fail','sys.ask_missing_info') or (
            name=='toy.set' and args.get('id')==7 and isinstance(args.get('value'),int))
        return {'ok':valid,'error':None if valid else 'invalid arguments'}
    def call(self,name,args):
        self.calls.append({'name':name,'arguments':args})
        if name=='toy.lookup': return {'ok':True,'rows':[{'id':7}]}
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

#[test]
fn v2_trace_retains_executed_prefix_when_call_budget_ends_a_block() {
    let mut fixture = Fixture::new(1, false);
    fixture.config.trace_directory = Some(fixture.config.root.join("traces"));
    // Four blocks execute 28 calls. The fifth executes four before the fixed
    // 32-call budget rejects the remainder; all four results must survive.
    let replies = (0..5)
        .map(|block| {
            decision(
                json!(
                    (0..7)
                        .map(|i| json!({
                            "name":"toy.set", "arguments":{"id":7,"value":6+block*7+i}
                        }))
                        .collect::<Vec<_>>()
                ),
                false,
            )
        })
        .collect();
    let mut model = V2Actor(ScriptedModel::new(replies));
    run(&fixture.config, &mut ToySpeech::default(), &mut model).unwrap();
    let metric = &fixture.metrics()[0];
    assert_eq!(metric.status, "failed");
    assert_eq!(metric.error.as_deref(), Some("sample call budget exceeded"));
    assert_eq!(metric.executed_calls, nora_lifemate::MAX_TOOL_CALLS);
    assert_eq!(metric.model_turns, 5);
    let record = &fixture.records()[0];
    assert_eq!(record["state_hash"], format!("{:064x}", 37));
    let trace: Value = serde_json::from_slice(
        &std::fs::read(fixture.config.root.join("traces/toy_0.json")).unwrap(),
    )
    .unwrap();
    let logged = trace["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| serde_json::from_str::<Value>(m["content"].as_str().unwrap()).unwrap())
        .flat_map(|m| {
            m["tool_observations"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|o| o["executed"] == true)
        .map(|o| o["executed_tool"].clone())
        .collect::<Vec<_>>();
    assert_eq!(json!(logged), record["tool_calls"]);
    let last: Value = serde_json::from_str(
        trace["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(last["tool_observations"].as_array().unwrap().len(), 4);
}

#[test]
fn v2_write_success_without_done_does_not_skip_the_rest_of_a_finished_block() {
    let fixture = Fixture::new(1, false);
    let split = fixture.config.root.join("train");
    std::fs::write(split.join("zaic_tools/__init__.py"), SDK.replace(
        "if name=='toy.set': self.state=args['value']",
        "if name=='toy.set': self.state=args['value'];return {'ok':True,'item':{'value':self.state}}"
    )).unwrap();
    let path = split.join("train_tools_catalog.json");
    let mut catalog: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    catalog["apps"][0]["tools"][1]["kind"] = json!("write");
    std::fs::write(path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let mut model = V2Actor(ScriptedModel::new(vec![decision(
        json!([
            {"name":"toy.set","arguments":{"id":7,"value":8}},
            {"name":"toy.set","arguments":{"id":7,"value":9}}
        ]),
        true,
    )]));
    run(&fixture.config, &mut ToySpeech::default(), &mut model).unwrap();
    assert_eq!(fixture.metrics()[0].status, "complete");
    assert_eq!(fixture.metrics()[0].model_turns, 1);
    assert_eq!(
        fixture.records()[0]["tool_calls"].as_array().unwrap().len(),
        2
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 9));
}

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
        let tools = ["toy.lookup", "toy.set", "toy.pending", "toy.fail"];
        let spec = |name: &str| json!({"name":name,"description":"Original toy tool","kind":"action","parameters":{"type":"object"}});
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

struct ScriptedCompactActor {
    inner: ScriptedModel,
    few_shot: bool,
}

struct V2Actor(ScriptedModel);
impl DecisionModel for V2Actor {
    fn actor_v2(&self) -> bool {
        true
    }
    fn compact_prompt(&self) -> bool {
        true
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        self.0.complete(messages, schema, remaining)
    }
}

struct V2Progressive(ScriptedModel);
impl DecisionModel for V2Progressive {
    fn actor_v2(&self) -> bool {
        true
    }
    fn progressive_catalog(&self) -> bool {
        true
    }
    fn compact_prompt(&self) -> bool {
        true
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        self.0.complete(messages, schema, remaining)
    }
}

#[test]
fn v2_catalog_inspection_never_becomes_an_sdk_effect_or_evidence_index() {
    let fixture = Fixture::new(1, false);
    let mut actor = V2Progressive(ScriptedModel::new(vec![
        decision(
            json!([{"name":"nora.inspect_tools","arguments":{"names":["toy.lookup","toy.set"]}}]),
            false,
        ),
        decision(json!([{"name":"toy.lookup","arguments":{}}]), false),
        decision(
            json!([{"name":"toy.set","arguments":{"id":{"$source":"/observations/0/result/rows/0/id"},"value":19}}]),
            true,
        ),
    ]));
    run(&fixture.config, &mut ToySpeech::default(), &mut actor).unwrap();
    assert_eq!(
        fixture.records()[0]["tool_calls"].as_array().unwrap().len(),
        2
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 19));
    assert_eq!(fixture.metrics()[0].catalog_requests, 1);
    assert_eq!(fixture.metrics()[0].status, "complete");
}

#[test]
fn v2_incomplete_effect_pauses_block_and_keeps_actual_receipt() {
    let fixture = Fixture::new(1, false);
    let mut actor = V2Actor(ScriptedModel::new(vec![
        decision(
            json!([{"name":"toy.pending","arguments":{}},{"name":"toy.set","arguments":{"id":7,"value":19}}]),
            true,
        ),
        decision(json!([]), true),
    ]));
    run(&fixture.config, &mut ToySpeech::default(), &mut actor).unwrap();
    let rows = fixture.records();
    assert_eq!(rows[0]["tool_calls"].as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["tool_calls"][0]["name"], "toy.pending");
    let feedback: Value =
        serde_json::from_str(actor.0.seen[1].last().unwrap()["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        feedback["tool_observations"][0]["progress"]["outcome"],
        "incomplete"
    );
}

#[test]
fn v2_binds_a_previous_observation_without_rewriting_receipts() {
    let fixture = Fixture::new(1, false);
    let mut actor = V2Actor(ScriptedModel::new(vec![
        decision(json!([{"name":"toy.lookup","arguments":{}}]), false),
        decision(
            json!([{"name":"toy.set","arguments":{"id":{"$source":"/observations/0/result/rows/0/id"},"value":19}}]),
            true,
        ),
    ]));
    run(&fixture.config, &mut ToySpeech::default(), &mut actor).unwrap();
    let rows = fixture.records();
    assert_eq!(rows[0]["tool_calls"][1]["arguments"]["id"], 7);
    assert_eq!(rows[0]["state_hash"], format!("{:064x}", 19));
}

#[test]
fn v2_cannot_predict_future_binding_within_block() {
    let fixture = Fixture::new(1, false);
    let mut actor = V2Actor(ScriptedModel::new(vec![
        decision(
            json!([{"name":"toy.lookup","arguments":{}},{"name":"toy.set","arguments":{"id":{"$source":"/observations/0/result/rows/0/id"},"value":19}}]),
            true,
        ),
        decision(json!([]), true),
    ]));
    run(&fixture.config, &mut ToySpeech::default(), &mut actor).unwrap();
    assert_eq!(
        fixture.records()[0]["tool_calls"].as_array().unwrap().len(),
        1
    );
}
impl DecisionModel for ScriptedCompactActor {
    fn compact_prompt(&self) -> bool {
        true
    }
    fn few_shot_prompt(&self) -> bool {
        self.few_shot
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        self.inner.complete(messages, schema, remaining)
    }
}

#[test]
fn few_shot_actor_is_opt_in_and_preserves_actual_context_and_sdk_receipt() {
    let baseline = Fixture::new(1, false);
    let experimental = Fixture::new(1, false);
    let make = |few_shot| ScriptedCompactActor {
        inner: ScriptedModel::new(vec![decision(json!([]), true)]),
        few_shot,
    };
    let mut original = make(false);
    let mut variant = make(true);
    run(&baseline.config, &mut ToySpeech::default(), &mut original).unwrap();
    run(
        &experimental.config,
        &mut ToySpeech::default(),
        &mut variant,
    )
    .unwrap();
    let before = &original.inner.seen[0];
    let after = &variant.inner.seen[0];
    assert_eq!(before[1], after[1]);
    let system_before = before[0]["content"].as_str().unwrap();
    let system_after = after[0]["content"].as_str().unwrap();
    assert!(!system_before.contains("SYNTHETIC EXAMPLES ONLY"));
    assert!(system_after.starts_with(system_before));
    assert!(system_after.contains("SYNTHETIC EXAMPLES ONLY"));
    assert!(!after[1].to_string().contains("toy_light_742"));
    assert_eq!(baseline.records(), experimental.records());
    assert_eq!(baseline.metrics()[0].executed_calls, 0);
    assert_eq!(experimental.metrics()[0].executed_calls, 0);
}
fn decision(calls: Value, finished: bool) -> std::result::Result<Value, &'static str> {
    Ok(json!({"tool_calls":calls,"finished":finished}))
}

fn select_cohort(fixture: &mut Fixture, ids: Value) {
    use sha2::{Digest, Sha256};
    let path = fixture._directory.path().join("frozen-cohort.json");
    let bytes = serde_json::to_vec(&ids).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    fixture.config.cohort = Some(
        nora_lifemate::cohort::EvaluationCohort::from_file(
            &path,
            &format!("{:x}", Sha256::digest(&bytes)),
        )
        .unwrap(),
    );
}

#[test]
fn pinned_cohort_preserves_dataset_order_and_model_messages() {
    let all = Fixture::new(3, false);
    let mut selected = Fixture::new(3, false);
    select_cohort(&mut selected, json!(["toy_2", "toy_0"]));
    let mut all_model = ScriptedModel::new(vec![decision(json!([]), true); 3]);
    let mut selected_model = ScriptedModel::new(vec![decision(json!([]), true); 2]);
    assert_eq!(
        run(&all.config, &mut ToySpeech::default(), &mut all_model).unwrap(),
        3
    );
    let mut speech = ToySpeech::default();
    assert_eq!(
        run(&selected.config, &mut speech, &mut selected_model).unwrap(),
        2
    );
    assert_eq!(speech.paths.len(), 2);
    let ids: Vec<_> = selected
        .records()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["toy_0", "toy_2"]);
    assert_eq!(selected_model.seen[0], all_model.seen[0]);
    assert_eq!(selected_model.seen[1], all_model.seen[2]);
    let prompt = serde_json::to_string(&selected_model.seen).unwrap();
    assert!(!prompt.contains("toy_0") && !prompt.contains("toy_2"));
}

#[test]
fn cohort_cannot_silently_shrink_or_select_public_submissions() {
    let mut fixture = Fixture::new(1, false);
    select_cohort(&mut fixture, json!(["toy_0"]));
    fixture.config.limit = Some(1);
    assert!(fixture.config.validate().is_err());
    fixture.config.limit = None;
    fixture.config.start_index = 1;
    assert!(fixture.config.validate().is_err());
    fixture.config.start_index = 0;
    fixture.config.split = "public_test".into();
    assert!(fixture.config.validate().is_err());
    assert!(!fixture.config.output.exists());
}

struct ScriptedPlanModel(ScriptedModel);
impl DecisionModel for ScriptedPlanModel {
    fn typed_plan(&self) -> bool {
        true
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        assert_eq!(schema["properties"]["finished"]["type"], "boolean");
        self.0.complete(messages, schema, remaining)
    }
}

struct ScriptedProgressiveModel {
    inner: ScriptedModel,
    exposures: Vec<Vec<String>>,
}
impl DecisionModel for ScriptedProgressiveModel {
    fn progressive_catalog(&self) -> bool {
        true
    }
    fn begin_sample(&mut self, tools: &[nora_lifemate::protocol::ToolSpec]) -> Result<()> {
        self.exposures
            .push(tools.iter().map(|tool| tool.name.clone()).collect());
        Ok(())
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        self.inner.complete(messages, schema, remaining)
    }
}

#[test]
fn progressive_inspections_recover_new_tools_without_entering_sdk_receipts() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedProgressiveModel {
        inner: ScriptedModel::new(vec![
            decision(
                json!([{"name":"nora.inspect_tools","arguments":{"names":["toy.lookup"]}}]),
                true,
            ),
            decision(json!([{"name":"toy.lookup","arguments":{}}]), false),
            decision(
                json!([{"name":"nora.inspect_tools","arguments":{"names":["toy.set"]}}]),
                false,
            ),
            decision(
                json!([{"name":"toy.set","arguments":{"id":7,"value":9}}]),
                true,
            ),
        ]),
        exposures: Vec::new(),
    };
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([
            {"name":"toy.lookup","arguments":{}},{"name":"toy.set","arguments":{"id":7,"value":9}}
        ])
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 9));
    assert_eq!(fixture.metrics()[0].catalog_requests, 2);
    assert_eq!(model.exposures[0], vec!["nora.inspect_tools"]);
    assert!(model.exposures[1].contains(&"toy.lookup".to_string()));
    assert!(!model.exposures[1].contains(&"toy.set".to_string()));
    assert!(model.exposures[2].contains(&"toy.set".to_string()));
    for seen in &model.inner.seen {
        assert!(seen[0]["content"].as_str().unwrap().contains("toy.set"));
    }
}

#[test]
fn undisclosed_or_mixed_progressive_proposals_never_reach_the_sdk() {
    for mixed in [false, true] {
        let fixture = Fixture::new(1, false);
        let mut speech = ToySpeech::default();
        let write = json!({"name":"toy.set","arguments":{"id":7,"value":9}});
        let inspect = json!({"name":"nora.inspect_tools","arguments":{"names":["toy.set"]}});
        let rejected = if mixed {
            json!([inspect, write])
        } else {
            json!([write])
        };
        let mut model = ScriptedProgressiveModel {
            inner: ScriptedModel::new(vec![
                decision(rejected, true),
                decision(json!([inspect]), false),
                decision(json!([write]), true),
            ]),
            exposures: Vec::new(),
        };
        assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
        assert_eq!(fixture.records()[0]["tool_calls"], json!([write]));
        assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 9));
        assert_eq!(fixture.metrics()[0].catalog_requests, 1);
        assert_eq!(model.inner.seen.len(), 3);
    }
}

#[test]
fn progressive_empty_completion_rechecks_inspection_and_preserves_pending_failures() {
    for pending in [false, true] {
        let fixture = Fixture::new(1, false);
        let mut speech = ToySpeech::default();
        let name = if pending { "toy.pending" } else { "toy.set" };
        let mut replies = vec![decision(
            json!([{"name":"nora.inspect_tools","arguments":{"names":[name]}}]),
            false,
        )];
        if !pending {
            replies.push(decision(json!([]), true));
        }
        replies.push(decision(json!([{"name":name,"arguments":if pending {json!({})} else {json!({"id":7,"value":9})}}]), !pending));
        if pending {
            replies.push(decision(json!([]), true));
        }
        let mut model = ScriptedProgressiveModel {
            inner: ScriptedModel::new(replies),
            exposures: Vec::new(),
        };
        assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
        assert_eq!(
            fixture.records()[0]["tool_calls"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            fixture.metrics()[0].status,
            if pending { "failed" } else { "complete" }
        );
        assert_eq!(model.inner.seen.len(), 3);
    }
}

#[test]
fn typed_plan_false_gate_never_reaches_sdk_but_read_receipt_is_preserved() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedPlanModel(ScriptedModel::new(vec![Ok(json!({
        "steps":[
            {"op":"call","id":"read","tool":"toy.lookup","arguments":{}},
            {"op":"call","id":"effect","tool":"toy.set","arguments":{
                "id":{"kind":"ref","step":"read","pointer":"/rows/0/id"},
                "value":{"kind":"literal","value":9}},
             "when":{"kind":"equals","left":{"kind":"ref","step":"read","pointer":"/rows/0/id"},
                "right":{"kind":"literal","value":8}}}
        ],"finished":true
    }))]));
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    let records = fixture.records();
    assert_eq!(
        records[0]["tool_calls"],
        json!([{"name":"toy.lookup","arguments":{}}])
    );
    assert_eq!(records[0]["state_hash"], format!("{:064x}", 5));
    assert_eq!(fixture.metrics()[0].model_turns, 1);
}

#[test]
fn a_completed_plan_fragment_is_followed_by_the_next_model_fragment() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedPlanModel(ScriptedModel::new(vec![
        Ok(
            json!({"steps":[{"op":"call","id":"read","tool":"toy.lookup","arguments":{}}],"finished":false}),
        ),
        Ok(
            json!({"steps":[{"op":"call","id":"write","tool":"toy.set","arguments":{
            "id":{"kind":"literal","value":7},"value":{"kind":"literal","value":9}}}],"finished":true}),
        ),
    ]));
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    let records = fixture.records();
    assert_eq!(records[0]["tool_calls"].as_array().unwrap().len(), 2);
    assert_eq!(records[0]["state_hash"], format!("{:064x}", 9));
    assert_eq!(fixture.metrics()[0].model_turns, 2);
    assert!(
        serde_json::to_string(&model.0.seen[1])
            .unwrap()
            .contains("plan_execution")
    );
}

#[test]
fn dependent_calls_observe_results_and_history_is_not_reexecuted_or_labelled() {
    let fixture = Fixture::new(1, true);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![
        decision(json!([{"name":"toy.lookup","arguments":{}}]), false),
        decision(
            json!([{"name":"toy.set","arguments":{"id":7,"value":9}}]),
            true,
        ),
    ]);
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(speech.paths, ["earlier.wav", "latest.wav"]);
    let records = fixture.records();
    assert_eq!(records[0]["tool_calls"].as_array().unwrap().len(), 2);
    assert_eq!(records[0]["state_hash"], format!("{:064x}", 9));
    let prompt = serde_json::to_string(&model.seen[0]).unwrap();
    assert!(!prompt.contains("EXCLUDED"));
    assert!(!prompt.contains("hidden.forbidden"));
    let observations: Value =
        serde_json::from_str(model.seen[1].last().unwrap()["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        observations["tool_observations"][0]["result"]["rows"][0]["id"],
        7
    );
    assert_eq!(fixture.metrics()[0].status, "complete");
}

#[test]
fn a_schema_refusal_can_be_corrected_without_polluting_executed_calls() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![
        decision(
            json!([{"name":"toy.set","arguments":{"id":7,"value":"invalid"}}]),
            true,
        ),
        decision(
            json!([{"name":"toy.set","arguments":{"id":7,"value":8}}]),
            true,
        ),
    ]);
    run(&fixture.config, &mut speech, &mut model).unwrap();
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([{"name":"toy.set","arguments":{"id":7,"value":8}}])
    );
    assert_eq!(fixture.metrics()[0].schema_refusals, 1);
}

#[test]
fn an_empty_unfinished_decision_is_repaired_before_the_sdk_receipt_is_completed() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![
        decision(json!([]), false),
        decision(
            json!([{"name":"toy.set","arguments":{"id":7,"value":9}}]),
            true,
        ),
    ]);
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(model.seen.len(), 2);
    assert!(model.replies.is_empty());
    let correction = model.seen[1].last().unwrap();
    assert_eq!(correction["role"], "user");
    assert!(
        correction["content"]
            .as_str()
            .unwrap()
            .contains("finished=false")
    );
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([{"name":"toy.set","arguments":{"id":7,"value":9}}])
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 9));
    let metric = &fixture.metrics()[0];
    assert_eq!(metric.status, "complete");
    assert_eq!(metric.model_turns, 2);
    assert_eq!(metric.executed_calls, 1);
    assert_eq!(metric.schema_refusals, 0);
}

#[test]
fn sdk_receipts_use_declared_arguments_while_the_native_journal_keeps_proposals() {
    let mut fixture = Fixture::new(1, false);
    fixture.config.trace_directory = Some(fixture.config.root.join("traces"));
    let catalog_path = fixture.config.root.join("train/train_tools_catalog.json");
    let mut catalog: Value =
        serde_json::from_slice(&std::fs::read(&catalog_path).unwrap()).unwrap();
    catalog["apps"][0]["tools"][0]["parameters"] = json!({"type":"object","properties":{}});
    catalog["apps"][0]["tools"][1]["parameters"] = json!({"type":"object","properties":{
        "id":{"type":"integer"},"value":{"type":"integer"},"name":{"type":"string"},
        "payload":{"type":"object"}},"required":["id","value"]});
    std::fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let lookup_proposal = json!({"name":"toy.lookup","arguments":{"name":"envelope mistake","tool_name":"toy.lookup"}});
    let nested = json!({"name":"keep this nested name","tool_name":"keep this nested tool_name"});
    let set_proposal = json!({"name":"toy.set","arguments":{"id":7,"value":9,
        "name":"keep the declared name","payload":nested,"tool_name":"toy.set"}});
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![
        decision(json!([lookup_proposal]), false),
        decision(json!([set_proposal]), true),
    ]);
    run(&fixture.config, &mut speech, &mut model).unwrap();
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([
            {"name":"toy.lookup","arguments":{}},
            {"name":"toy.set","arguments":{"id":7,"value":9,"name":"keep the declared name","payload":nested}}
        ])
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 9));
    assert_eq!(fixture.metrics()[0].discarded_unknown_arguments, 3);
    assert_eq!(fixture.metrics()[0].schema_refusals, 0);
    let trace: Value = serde_json::from_slice(
        &std::fs::read(fixture.config.root.join("traces/toy_0.json")).unwrap(),
    )
    .unwrap();
    let journal = trace["messages"].as_array().unwrap();
    let observations: Value =
        serde_json::from_str(journal[3]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        observations["tool_observations"][0]["tool"],
        lookup_proposal
    );
    assert_eq!(
        observations["tool_observations"][0]["executed_tool"],
        json!({"name":"toy.lookup","arguments":{}})
    );
    assert_eq!(
        observations["tool_observations"][0]["discarded_argument_keys"],
        json!(["name", "tool_name"])
    );
    // Native tool results still bind to the exact original proposal, while
    // only the normalized call appears in the authoritative SDK receipt.
    let native = nora_lifemate::native_tools::messages(journal).unwrap();
    let native_proposal = native
        .iter()
        .find(|message| message.get("tool_calls").is_some())
        .unwrap();
    let arguments: Value = serde_json::from_str(
        native_proposal["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(arguments, lookup_proposal["arguments"]);
}

#[test]
fn an_executed_failure_is_logged_and_done_false_does_not_end_the_task() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![
        decision(json!([{"name":"toy.fail","arguments":{}}]), true),
        decision(json!([{"name":"toy.pending","arguments":{}}]), true),
        decision(
            json!([{"name":"toy.set","arguments":{"id":7,"value":10}}]),
            true,
        ),
    ]);
    run(&fixture.config, &mut speech, &mut model).unwrap();
    assert_eq!(
        fixture.records()[0]["tool_calls"].as_array().unwrap().len(),
        3
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 10));
    assert_eq!(fixture.metrics()[0].model_turns, 3);
}

#[test]
fn a_model_error_preserves_prior_effects_and_does_not_discard_later_samples() {
    let fixture = Fixture::new(2, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![
        decision(
            json!([{"name":"toy.set","arguments":{"id":7,"value":11}}]),
            false,
        ),
        Err("original toy model failure"),
        decision(json!([]), true),
    ]);
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 2);
    let records = fixture.records();
    assert_eq!(records[0]["state_hash"], format!("{:064x}", 11));
    assert_eq!(records[0]["tool_calls"].as_array().unwrap().len(), 1);
    assert_eq!(records[1]["tool_calls"], json!([]));
    let metrics = fixture.metrics();
    assert_eq!(metrics[0].status, "failed");
    assert_eq!(metrics[1].status, "complete");
}

#[test]
fn a_speech_failure_still_returns_the_unchanged_sdk_receipt() {
    let fixture = Fixture::new(2, false);
    let mut speech = ToySpeech {
        fail_first: true,
        ..ToySpeech::default()
    };
    let mut model = ScriptedModel::new(vec![decision(json!([]), true)]);
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 2);
    assert_eq!(fixture.records()[0]["tool_calls"], json!([]));
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 5));
    assert_eq!(fixture.metrics()[0].status, "failed");
    assert_eq!(model.seen.len(), 1);
}

#[test]
fn typed_plan_pending_effect_cannot_be_erased_by_an_empty_finish() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut replies = vec![Ok(
        json!({"steps":[{"op":"call","id":"pending","tool":"toy.pending","arguments":{}}],"finished":true}),
    )];
    replies.extend(
        (1..nora_lifemate::MAX_MODEL_TURNS).map(|_| Ok(json!({"steps":[],"finished":true}))),
    );
    let mut model = ScriptedPlanModel(ScriptedModel::new(replies));
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([{"name":"toy.pending","arguments":{}}])
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 6));
    assert_eq!(fixture.metrics()[0].status, "failed");
}

#[test]
fn typed_plan_ask_suspends_before_a_later_effect() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedPlanModel(ScriptedModel::new(vec![Ok(json!({"steps":[
        {"op":"call","id":"ask","tool":"sys.ask_missing_info","arguments":{}},
        {"op":"call","id":"later","tool":"toy.set","arguments":{
            "id":{"kind":"literal","value":7},"value":{"kind":"literal","value":9}}}
    ],"finished":true}))]));
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([{"name":"sys.ask_missing_info","arguments":{}}])
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 5));
    assert_eq!(model.0.seen.len(), 1);
}

#[test]
fn typed_plan_transport_failure_never_replans_an_uncertain_effect() {
    let fixture = Fixture::new(1, false);
    let sdk = fixture.config.root.join("train/zaic_tools/__init__.py");
    std::fs::write(
        sdk,
        SDK.replace(
            "if name=='toy.lookup': return {'ok':True,'rows':[{'id':7}]}",
            "if name=='toy.lookup': raise RuntimeError('original toy transport failure')",
        ),
    )
    .unwrap();
    let mut speech = ToySpeech::default();
    let mut model = ScriptedPlanModel(ScriptedModel::new(vec![Ok(json!({"steps":[
        {"op":"call","id":"uncertain","tool":"toy.lookup","arguments":{}}
    ],"finished":false}))]));
    assert!(run(&fixture.config, &mut speech, &mut model).is_err());
    assert_eq!(model.0.seen.len(), 1);
    assert!(fixture.records().is_empty());
}

#[test]
fn typed_plan_failed_ask_suspends_effects_but_keeps_failed_classification() {
    let fixture = Fixture::new(1, false);
    let sdk = fixture.config.root.join("train/zaic_tools/__init__.py");
    std::fs::write(sdk, SDK.replace("if name=='toy.set': self.state=args['value']",
        "if name=='sys.ask_missing_info': return {'ok':False,'error':'original ask failure'}\n        if name=='toy.set': self.state=args['value']")).unwrap();
    let mut speech = ToySpeech::default();
    let mut model = ScriptedPlanModel(ScriptedModel::new(vec![Ok(json!({"steps":[
        {"op":"call","id":"ask","tool":"sys.ask_missing_info","arguments":{}},
        {"op":"call","id":"later","tool":"toy.pending","arguments":{}}
    ],"finished":true}))]));
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([{"name":"sys.ask_missing_info","arguments":{}}])
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 5));
    assert_eq!(fixture.metrics()[0].status, "failed");
    assert_eq!(model.0.seen.len(), 1);
}

#[test]
fn typed_plan_receipt_uses_the_normalized_actual_call() {
    let fixture = Fixture::new(1, false);
    let catalog_path = fixture.config.root.join("train/train_tools_catalog.json");
    let mut catalog: Value =
        serde_json::from_slice(&std::fs::read(&catalog_path).unwrap()).unwrap();
    catalog["apps"][0]["tools"][1]["parameters"] = json!({"type":"object","properties":{
        "id":{"type":"integer"},"value":{"type":"integer"}},"required":["id","value"],"additionalProperties":false});
    std::fs::write(catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let mut speech = ToySpeech::default();
    let mut model = ScriptedPlanModel(ScriptedModel::new(vec![Ok(json!({"steps":[
        {"op":"call","id":"set","tool":"toy.set","arguments":{
            "id":{"kind":"literal","value":7},"value":{"kind":"literal","value":9},
            "unrequested_extra":{"kind":"literal","value":"discard"}}}
    ],"finished":true}))]));
    assert_eq!(run(&fixture.config, &mut speech, &mut model).unwrap(), 1);
    assert_eq!(
        fixture.records()[0]["tool_calls"],
        json!([{"name":"toy.set","arguments":{"id":7,"value":9}}])
    );
    assert_eq!(fixture.metrics()[0].discarded_unknown_arguments, 1);
}

#[test]
fn asking_for_missing_info_stops_without_executing_a_later_batched_effect() {
    let fixture = Fixture::new(1, false);
    let mut speech = ToySpeech::default();
    let mut model = ScriptedModel::new(vec![decision(
        json!([
            {"name":"sys.ask_missing_info","arguments":{"tool_name":"toy.set","params":["value"]}},
            {"name":"toy.set","arguments":{"id":7,"value":999}}
        ]),
        false,
    )]);
    run(&fixture.config, &mut speech, &mut model).unwrap();
    assert_eq!(
        fixture.records()[0]["tool_calls"].as_array().unwrap().len(),
        1
    );
    assert_eq!(fixture.records()[0]["state_hash"], format!("{:064x}", 5));
}

// Opt-in experiment must guard the real execution boundary even if a model
// transport ignores the constrained decoder entirely.
struct ClarificationActor(ScriptedModel);
impl DecisionModel for ClarificationActor {
    fn compact_prompt(&self) -> bool {
        true
    }
    fn schema_bound_clarification(&self) -> bool {
        true
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        self.0.complete(messages, schema, remaining)
    }
}
#[test]
fn invented_clarification_fields_never_reach_sdk_but_valid_ask_still_suspends() {
    let f = Fixture::new(1, false);
    let catalog_path = f.config.root.join("train/train_tools_catalog.json");
    let mut catalog: Value =
        serde_json::from_slice(&std::fs::read(&catalog_path).unwrap()).unwrap();
    catalog["apps"][0]["tools"][1]["parameters"] = json!({"type":"object","properties":{
        "id":{"type":"integer"},"value":{"type":"integer"}},"required":["id","value"]});
    std::fs::write(catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    let mut model = ClarificationActor(ScriptedModel::new(vec![
        decision(
            json!([{"name":"sys.ask_missing_info","arguments":{"tool_name":"toy.set","params":["invented_field"]}}]),
            false,
        ),
        decision(
            json!([{"name":"sys.ask_missing_info","arguments":{"tool_name":"toy.set","params":["value"]}}]),
            false,
        ),
    ]));
    run(&f.config, &mut ToySpeech::default(), &mut model).unwrap();
    let row: Value =
        serde_json::from_str(&std::fs::read_to_string(&f.config.output).unwrap()).unwrap();
    let metric: SampleMetric =
        serde_json::from_str(&std::fs::read_to_string(&f.config.metrics).unwrap()).unwrap();
    assert_eq!(row["tool_calls"].as_array().unwrap().len(), 1);
    assert_eq!(
        row["tool_calls"][0]["arguments"]["params"],
        json!(["value"])
    );
    assert_eq!(metric.schema_refusals, 1);
    assert_eq!(metric.model_turns, 2);
    assert_eq!(metric.status, "complete");
}
