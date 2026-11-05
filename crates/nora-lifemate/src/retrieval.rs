//! Opt-in, train-only worked-example retrieval. No SDK calls, generated labels,
//! network access, hidden state, or claim that lexical similarity proves intent.
use crate::{Result, cohort::EvaluationCohort, protocol::ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::Path,
};
use unicode_normalization::UnicodeNormalization;

const MAX_BYTES: u64 = 32_000_000;
const MAX_EXAMPLE_BYTES: usize = 4_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    scope: String,
    records: Vec<Record>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    sample_id: String,
    latest_request: String,
    tools: Vec<String>,
    example: Value,
}
pub struct ExampleIndex {
    records: Vec<Record>,
    counts: Vec<BTreeMap<String, usize>>,
    frequencies: BTreeMap<String, usize>,
    average_length: f64,
    pub sha256: String,
    example_before_request: bool,
    #[cfg(feature = "semantic-retrieval")]
    semantic: Option<crate::semantic_retrieval::SemanticRetriever>,
}
fn words(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    // Canonically equivalent Vietnamese spellings must not split into different
    // lexical terms. Preserve diacritics and never modify the original payload.
    let normalized: String = text.nfc().collect();
    for word in normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
    {
        *counts.entry(word.to_lowercase()).or_default() += 1;
    }
    counts
}
impl ExampleIndex {
    pub fn from_file(path: &Path, expected_sha: &str, train: &EvaluationCohort) -> Result<Self> {
        if fs::metadata(path)?.len() > MAX_BYTES {
            return Err("training example corpus exceeds bound".into());
        }
        let mut bytes = Vec::new();
        File::open(path)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES || format!("{:x}", Sha256::digest(&bytes)) != expected_sha
        {
            return Err("training example corpus hash changed".into());
        }
        let corpus: Corpus = serde_json::from_slice(&bytes)?;
        if corpus.scope != "internal-train-worked-examples-v1"
            || corpus.records.len() != train.len()
        {
            return Err("training example scope/coverage mismatch".into());
        }
        let mut ids = BTreeSet::new();
        let mut counts = Vec::new();
        let mut frequencies = BTreeMap::new();
        let mut total = 0;
        for row in &corpus.records {
            if !train.contains(&row.sample_id)
                || !ids.insert(row.sample_id.clone())
                || row.latest_request.is_empty()
                || row.latest_request.len() > 65_536
                || !row.example.is_object()
                || row.example["latest_request"] != row.latest_request
                || row.tools.len() > 64
                || row.example.as_object().is_some_and(|v| {
                    v.keys().any(|k| {
                        !["latest_request", "context", "history", "steps"].contains(&k.as_str())
                    })
                })
                || !row.example["context"].is_object()
                || !row.example["history"].is_array()
                || row.example["steps"].as_array().is_none_or(|v| v.len() > 12)
            {
                return Err("invalid training example".into());
            }
            let terms = words(&row.latest_request);
            if terms.is_empty() {
                return Err("empty retrieval terms".into());
            }
            total += terms.values().sum::<usize>();
            for term in terms.keys() {
                *frequencies.entry(term.clone()).or_default() += 1;
            }
            counts.push(terms);
        }
        Ok(Self {
            average_length: total as f64 / corpus.records.len() as f64,
            records: corpus.records,
            counts,
            frequencies,
            sha256: expected_sha.into(),
            example_before_request: false,
            #[cfg(feature = "semantic-retrieval")]
            semantic: None,
        })
    }
    /// Experimental presentation only. The current task remains the last
    /// initial user payload; the worked example is a separate data-only turn.
    pub fn with_example_before_request(mut self) -> Self {
        self.example_before_request = true;
        self
    }
    #[cfg(feature = "semantic-retrieval")]
    pub fn training_requests(&self) -> Vec<(&str, &str)> {
        self.records
            .iter()
            .map(|row| (row.sample_id.as_str(), row.latest_request.as_str()))
            .collect()
    }
    #[cfg(feature = "semantic-retrieval")]
    pub fn with_semantic_assets(
        mut self,
        index_path: &Path,
        index_sha256: &str,
        assets_path: &Path,
        assets_sha256: &str,
    ) -> Result<Self> {
        use crate::{
            embedding::{E5Encoder, EmbeddingAssets},
            semantic_retrieval::{SemanticIndex, SemanticRetriever},
        };
        let index = SemanticIndex::read(
            index_path,
            index_sha256,
            &self.sha256,
            &self.training_requests(),
        )?;
        let encoder = E5Encoder::load(EmbeddingAssets::read(assets_path, assets_sha256)?)?;
        self.semantic = Some(SemanticRetriever::new(index, encoder)?);
        Ok(self)
    }
    pub fn check_disjoint(&self, evaluation: &EvaluationCohort) -> Result<()> {
        if self
            .records
            .iter()
            .any(|r| evaluation.contains(&r.sample_id))
        {
            return Err("training examples overlap evaluation cohort".into());
        }
        Ok(())
    }
    /// Preserve the exact original request, history, context and current journal.
    /// Add one explicitly marked data example to the initial user JSON only.
    /// Oversized examples are ineligible; no current-task data is shortened.
    pub fn augment(
        &mut self,
        messages: &[Value],
        tools: &[ToolSpec],
    ) -> Result<(Vec<Value>, Value)> {
        let mut wire = messages.to_vec();
        let allowed = tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<BTreeSet<_>>();
        let mut initial = None;
        for (i, msg) in messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m["role"] == "user")
        {
            if let Some(data) = msg["content"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .filter(|v| v.get("latest_request").is_some())
                && initial.replace((i, data)).is_some()
            {
                return Err("multiple initial task payloads".into());
            }
        }
        let (index, mut data) = initial.ok_or("retrieval requires original request")?;
        let request = data["latest_request"]
            .as_str()
            .ok_or("request must be text")?;
        let query = words(request);
        #[cfg(feature = "semantic-retrieval")]
        let query_sha256 = format!("{:x}", Sha256::digest(request.as_bytes()));
        let n = self.records.len() as f64;
        let mut best: Option<(usize, f64)> = None;
        #[cfg(feature = "semantic-retrieval")]
        let semantic_scores = self
            .semantic
            .as_mut()
            .map(|s| s.scores(request))
            .transpose()?
            .flatten();
        #[cfg(feature = "semantic-retrieval")]
        let mut fusion_candidates = Vec::new();
        for (i, row) in self.records.iter().enumerate() {
            if row.tools.iter().any(|t| !allowed.contains(t.as_str()))
                || serde_json::to_vec(&row.example)?.len() > MAX_EXAMPLE_BYTES
            {
                continue;
            }
            let terms = &self.counts[i];
            let length = terms.values().sum::<usize>() as f64;
            let mut score = 0.0;
            for term in query.keys() {
                if let Some(&count) = terms.get(term) {
                    let df = self.frequencies[term] as f64;
                    let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                    let tf = count as f64;
                    score +=
                        idf * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * length / self.average_length));
                }
            }
            #[cfg(feature = "semantic-retrieval")]
            if let Some(scores) = &semantic_scores {
                fusion_candidates.push((i, row.sample_id.as_str(), score, scores[i]));
            }
            if score > 0.0
                && best.is_none_or(|(j, old)| {
                    score > old || (score == old && row.sample_id < self.records[j].sample_id)
                })
            {
                best = Some((i, score));
            }
        }
        let mut receipt = json!({"corpus_sha256":self.sha256,"selected":false,"method":"BM25-k1=1.2-b=0.75-one-example","current_context_truncated":false});
        receipt["presentation"] = if self.example_before_request {
            "data_turn_before_current_request"
        } else {
            "field_in_current_request"
        }
        .into();
        #[cfg(feature = "semantic-retrieval")]
        if let Some(semantic) = &self.semantic {
            receipt["semantic_index_sha256"] = semantic.index_sha256().into();
            if semantic_scores.is_some() {
                best = crate::semantic_retrieval::fused_ranking(&fusion_candidates)
                    .first()
                    .copied();
                receipt["method"] = "BM25+E5-RRF-k60-one-example".into();
            } else {
                receipt["semantic_fallback"] = "complete_query_exceeds_512_tokens".into();
            }
        }
        if let Some((i, score)) = best {
            let row = &self.records[i];
            if data.get("reference_example").is_some() {
                return Err("reference example already present".into());
            }
            let example = json!({
                "scope":"TRAINING EXAMPLE ONLY, a different task. Learn the workflow. Its values, IDs and observations are NOT evidence for the current task and must not be copied into current actions. The original latest_request, context, history and current tool observations remain authoritative.",
                "worked_task":row.example
            });
            if self.example_before_request {
                wire.insert(index, json!({"role":"user","content":serde_json::to_string(&json!({"training_example_only":example}))?}));
            } else {
                data["reference_example"] = example;
                wire[index]["content"] = serde_json::to_string(&data)?.into();
            }
            receipt["selected"] = true.into();
            receipt["sample_id"] = row.sample_id.clone().into();
            receipt["score"] = score.into();
            #[cfg(feature = "semantic-retrieval")]
            if let Some(scores) = &semantic_scores {
                receipt["selected_cosine"] = serde_json::to_value(scores[i])?;
                receipt["query_sha256"] = query_sha256.into();
            }
        }
        Ok((wire, receipt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cohort(dir: &Path, name: &str, ids: &[&str]) -> EvaluationCohort {
        let path = dir.join(name);
        let bytes = serde_json::to_vec(ids).unwrap();
        fs::write(&path, &bytes).unwrap();
        EvaluationCohort::from_file(&path, &format!("{:x}", Sha256::digest(bytes))).unwrap()
    }
    fn row(id: &str, request: &str, tool: &str) -> Value {
        json!({"sample_id":id,"latest_request":request,"tools":[tool],
            "example":{"latest_request":request,"context":{},"history":[],"steps":[]}})
    }
    fn index(dir: &Path, rows: &[Value], train: &EvaluationCohort) -> Result<ExampleIndex> {
        let path = dir.join("corpus.json");
        let bytes = serde_json::to_vec(
            &json!({"scope":"internal-train-worked-examples-v1","records":rows}),
        )
        .unwrap();
        fs::write(&path, &bytes).unwrap();
        ExampleIndex::from_file(&path, &format!("{:x}", Sha256::digest(bytes)), train)
    }
    fn tool(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: String::new(),
            kind: "read".into(),
            parameters: json!({}),
            returns: Value::Null,
        }
    }
    fn journal(request: &str) -> Vec<Value> {
        vec![
            json!({"role":"system","content":"Original policy"}),
            json!({"role":"user","content":json!({"latest_request":request,"context":{"current_time":"NOW"},"history":[{"role":"user","text":"Original earlier request"}]}).to_string()}),
            json!({"role":"assistant","content":"Original decision"}),
            json!({"role":"user","content":json!({"tool_observations":[{"result":{"id":"actual_001"}}]}).to_string()}),
        ]
    }
    #[test]
    fn canonical_accents_rank_identically_without_changing_user_bytes() {
        let composed = "Tìm lịch hẹn, gửi mẹ 50.000 đồng";
        let decomposed: String = composed.nfd().collect();
        assert_ne!(composed, decomposed);
        assert_eq!(words(composed), words(&decomposed));
        assert_ne!(words("mẹ"), words("me"));
        let dir = tempfile::tempdir().unwrap();
        let train = cohort(dir.path(), "train.json", &["t1", "t2"]);
        let mut idx = index(
            dir.path(),
            &[
                row("t1", "Tìm lịch hẹn", "calendar.list"),
                row("t2", "Tắt đèn", "light.off"),
            ],
            &train,
        )
        .unwrap()
        .with_example_before_request();
        let tools = [tool("calendar.list"), tool("light.off")];
        let (_, reference) = idx.augment(&journal(composed), &tools).unwrap();
        let original = journal(&decomposed);
        let (wire, actual) = idx.augment(&original, &tools).unwrap();
        assert_eq!(actual["sample_id"], reference["sample_id"]);
        assert_eq!(actual["score"], reference["score"]);
        assert_eq!(wire[2..], original[1..]);
    }
    #[test]
    fn preceding_example_keeps_current_request_and_observations_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let train = cohort(dir.path(), "train.json", &["t1"]);
        let rows = [row("t1", "Tìm lịch hẹn", "calendar.list")];
        let mut inline = index(dir.path(), &rows, &train).unwrap();
        let mut before = index(dir.path(), &rows, &train)
            .unwrap()
            .with_example_before_request();
        let original = journal(" Tìm lịch hẹn; đừng đổi lịch.\n");
        let tools = [tool("calendar.list")];
        let (_, old) = inline.augment(&original, &tools).unwrap();
        let (wire, receipt) = before.augment(&original, &tools).unwrap();
        assert_eq!(wire.len(), original.len() + 1);
        assert_eq!(wire[0], original[0]);
        assert_eq!(wire[2..], original[1..]);
        assert_eq!(receipt["sample_id"], old["sample_id"]);
        assert_eq!(receipt["score"], old["score"]);
        assert_eq!(wire[1]["role"], "user");
        let data: Value = serde_json::from_str(wire[1]["content"].as_str().unwrap()).unwrap();
        assert!(data.get("latest_request").is_none());
        assert_eq!(
            data["training_example_only"]["worked_task"],
            rows[0]["example"]
        );
        assert_eq!(
            before.augment(&journal("unrelated"), &tools).unwrap().0,
            journal("unrelated")
        );
    }
    #[test]
    fn example_is_data_and_never_replaces_current_evidence_or_request() {
        let dir = tempfile::tempdir().unwrap();
        let train = cohort(dir.path(), "train.json", &["t1"]);
        let mut index = index(
            dir.path(),
            &[row("t1", "Tìm lịch hẹn", "calendar.list")],
            &train,
        )
        .unwrap();
        let original = journal("  Tìm lịch hẹn\n{\"text\":\"giữ nguyên\"}  ");
        let (wire, receipt) = index.augment(&original, &[tool("calendar.list")]).unwrap();
        assert_eq!(receipt["sample_id"], "t1");
        assert_eq!(wire[0], original[0]);
        assert_eq!(wire[2..], original[2..]);
        let mut data: Value = serde_json::from_str(wire[1]["content"].as_str().unwrap()).unwrap();
        assert!(
            data.as_object_mut()
                .unwrap()
                .remove("reference_example")
                .is_some()
        );
        assert_eq!(
            data,
            serde_json::from_str::<Value>(original[1]["content"].as_str().unwrap()).unwrap()
        );
    }
    #[test]
    fn training_coverage_hash_and_evaluation_disjointness_are_required() {
        let dir = tempfile::tempdir().unwrap();
        let train = cohort(dir.path(), "train.json", &["t1"]);
        assert!(
            index(
                dir.path(),
                &[row("foreign", "Lịch hẹn", "calendar.list")],
                &train
            )
            .is_err()
        );
        let idx = index(
            dir.path(),
            &[row("t1", "Lịch hẹn", "calendar.list")],
            &train,
        )
        .unwrap();
        assert!(idx.check_disjoint(&train).is_err());
        let evaluation = cohort(dir.path(), "eval.json", &["v1"]);
        idx.check_disjoint(&evaluation).unwrap();
        assert!(
            ExampleIndex::from_file(&dir.path().join("corpus.json"), &"0".repeat(64), &train)
                .is_err()
        );
    }
    #[test]
    fn unavailable_or_oversized_examples_are_skipped_without_dropping_task_data() {
        let dir = tempfile::tempdir().unwrap();
        let train = cohort(dir.path(), "train.json", &["t1", "t2"]);
        let mut huge = row("t2", "Tìm lịch hẹn", "calendar.list");
        huge["example"]["history"] = json!([{"text":"x".repeat(MAX_EXAMPLE_BYTES)}]);
        let mut idx = index(
            dir.path(),
            &[row("t1", "Tìm lịch hẹn", "unavailable.call"), huge],
            &train,
        )
        .unwrap();
        let original = journal("Tìm lịch hẹn");
        let (wire, receipt) = idx.augment(&original, &[tool("calendar.list")]).unwrap();
        assert_eq!(wire, original);
        assert_eq!(receipt["selected"], false);
    }
    #[test]
    fn ties_are_stable_and_zero_overlap_has_no_example() {
        let dir = tempfile::tempdir().unwrap();
        let train = cohort(dir.path(), "train.json", &["t1", "t2"]);
        let mut idx = index(
            dir.path(),
            &[
                row("t2", "Lịch hẹn", "calendar.list"),
                row("t1", "Lịch hẹn", "calendar.list"),
            ],
            &train,
        )
        .unwrap();
        let (_, receipt) = idx
            .augment(&journal("lịch hẹn"), &[tool("calendar.list")])
            .unwrap();
        assert_eq!(receipt["sample_id"], "t1");
        let original = journal("xyz");
        assert_eq!(
            idx.augment(&original, &[tool("calendar.list")]).unwrap().0,
            original
        );
    }
}
