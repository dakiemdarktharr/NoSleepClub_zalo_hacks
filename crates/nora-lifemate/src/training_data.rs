//! Training-only packing of hash-pinned causal token prefixes.
//! No tokenizer, model, SDK execution or inference policy is involved.
use crate::{
    MAX_CONTEXT_TOKENS, MAX_JSON_BYTES, MAX_MODEL_TURNS, Result, cohort::EvaluationCohort,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{BufRead, Read, Write},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prefix {
    example_id: String,
    sample_id: String,
    partition: String,
    ids: Vec<u32>,
    prompt_tokens: usize,
    full_tokens: usize,
    completion_tokens: usize,
    record_sha256: String,
    token_ids_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Turn {
    pub example_id: String,
    /// Labels are ids[target_start..target_end]; logits start one token earlier.
    pub target_start: usize,
    pub target_end: usize,
    pub source_record_sha256: String,
}

#[derive(Serialize)]
struct Trajectory {
    sample_id: String,
    partition: &'static str,
    ids: Vec<u32>,
    full_tokens: usize,
    target_tokens: usize,
    turns: Vec<Turn>,
    token_ids_sha256: String,
    no_truncation: bool,
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Summary {
    pub samples: usize,
    pub turns: usize,
    pub prefix_tokens: u64,
    pub trajectory_tokens: u64,
    pub target_tokens: u64,
    pub max_sequence_tokens: usize,
    pub max_target_tokens: usize,
}

fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn token_sha(ids: &[u32]) -> String {
    let mut hash = Sha256::new();
    for id in ids {
        hash.update(id.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}

impl Prefix {
    fn validate(&self, allowed: &EvaluationCohort, eos: u32, vocabulary: u32) -> Result<()> {
        if self.partition != "train"
            || !allowed.contains(&self.sample_id)
            || self.full_tokens != self.ids.len()
            || self.full_tokens > MAX_CONTEXT_TOKENS as usize
            || self.prompt_tokens == 0
            || self.prompt_tokens >= self.full_tokens
            || self.completion_tokens != self.full_tokens - self.prompt_tokens
            || self.ids.last() != Some(&eos)
            || self.ids.iter().any(|id| *id >= vocabulary)
            || !valid_sha(&self.record_sha256)
            || self.token_ids_sha256 != token_sha(&self.ids)
        {
            return Err(
                "invalid training prefix, partition, whitelist, loss span or token digest".into(),
            );
        }
        Ok(())
    }
}

/// Group only contiguous prefixes of the same whitelisted training sample.
/// A complete task retains every earlier token/target exactly. A caller must
/// stage output until this function succeeds, as malformed later rows can fail.
/// For a uniform task sampler, (sample_count / turn_count) * sum(turn_mean_CE)
/// has the same expected fixed-weight objective as uniform prefix sampling.
/// Grouping changes the sequence of optimizer updates; it is not an Adam replay.
pub fn pack(
    mut input: impl BufRead,
    mut output: impl Write,
    allowed: &EvaluationCohort,
    eos: u32,
    vocabulary: u32,
) -> Result<Summary> {
    if eos >= vocabulary || vocabulary == 0 || allowed.is_empty() {
        return Err("invalid tokenizer or empty training whitelist".into());
    }
    let mut summary = Summary::default();
    let mut seen = BTreeSet::new();
    let mut pending: Option<Trajectory> = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        if input
            .by_ref()
            .take((MAX_JSON_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            break;
        }
        if line.len() > MAX_JSON_BYTES {
            return Err("training prefix line exceeds size bound".into());
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let row: Prefix = serde_json::from_slice(&line)?;
        row.validate(allowed, eos, vocabulary)?;
        if pending
            .as_ref()
            .is_some_and(|task| task.sample_id != row.sample_id)
        {
            emit(
                pending.take().expect("pending task"),
                &mut output,
                &mut summary,
            )?;
        }
        if pending.is_none() {
            if !seen.insert(row.sample_id.clone()) {
                return Err("noncontiguous or duplicated training sample".into());
            }
            pending = Some(Trajectory {
                sample_id: row.sample_id.clone(),
                partition: "train",
                ids: Vec::new(),
                full_tokens: 0,
                target_tokens: 0,
                turns: Vec::new(),
                token_ids_sha256: String::new(),
                no_truncation: true,
            });
        }
        let task = pending.as_mut().expect("initialized task");
        if task.turns.len() >= MAX_MODEL_TURNS
            || row.example_id != format!("{}:actor:{:03}", row.sample_id, task.turns.len())
            || task.full_tokens >= row.prompt_tokens
            || !row.ids.starts_with(&task.ids)
        {
            return Err(
                "noncausal prefix, overlapping target, reordered or missing actor turn".into(),
            );
        }
        summary.prefix_tokens += row.full_tokens as u64;
        task.target_tokens += row.completion_tokens;
        task.turns.push(Turn {
            example_id: row.example_id,
            target_start: row.prompt_tokens,
            target_end: row.full_tokens,
            source_record_sha256: row.record_sha256,
        });
        task.full_tokens = row.full_tokens;
        task.ids = row.ids;
        task.token_ids_sha256 = row.token_ids_sha256;
    }
    if let Some(task) = pending {
        emit(task, &mut output, &mut summary)?;
    }
    if summary.samples != allowed.len() {
        return Err("training whitelist coverage is incomplete".into());
    }
    output.flush()?;
    Ok(summary)
}

fn emit(task: Trajectory, output: &mut impl Write, summary: &mut Summary) -> Result<()> {
    summary.samples += 1;
    summary.turns += task.turns.len();
    summary.trajectory_tokens += task.full_tokens as u64;
    summary.target_tokens += task.target_tokens as u64;
    summary.max_sequence_tokens = summary.max_sequence_tokens.max(task.full_tokens);
    summary.max_target_tokens = summary.max_target_tokens.max(task.target_tokens);
    serde_json::to_writer(&mut *output, &task)?;
    output.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::Cursor;

    fn cohort(names: &[&str]) -> EvaluationCohort {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let bytes = serde_json::to_vec(names).unwrap();
        file.write_all(&bytes).unwrap();
        EvaluationCohort::from_file(file.path(), &format!("{:x}", Sha256::digest(&bytes))).unwrap()
    }
    fn row(sid: &str, index: usize, ids: Vec<u32>, start: usize) -> Value {
        json!({"example_id":format!("{sid}:actor:{index:03}"),"sample_id":sid,"partition":"train",
            "prompt_tokens":start,"full_tokens":ids.len(),"completion_tokens":ids.len()-start,
            "record_sha256":"a".repeat(64),"token_ids_sha256":token_sha(&ids),"ids":ids})
    }
    fn run(rows: &[Value], names: &[&str]) -> Result<(Summary, Vec<Value>)> {
        let text = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let mut out = Vec::new();
        let report = pack(Cursor::new(text), &mut out, &cohort(names), 99, 200)?;
        Ok((
            report,
            String::from_utf8(out)?
                .lines()
                .map(serde_json::from_str)
                .collect::<std::result::Result<_, _>>()?,
        ))
    }
    #[test]
    fn all_targets_survive_without_future_tokens_in_earlier_prefixes() {
        let rows = [
            row("train_a", 0, vec![10, 11, 99], 2),
            row("train_a", 1, vec![10, 11, 99, 12, 13, 99], 4),
        ];
        let (s, out) = run(&rows, &["train_a"]).unwrap();
        assert_eq!(
            s,
            Summary {
                samples: 1,
                turns: 2,
                prefix_tokens: 9,
                trajectory_tokens: 6,
                target_tokens: 3,
                max_sequence_tokens: 6,
                max_target_tokens: 3
            }
        );
        assert_eq!(out[0]["ids"], rows[1]["ids"]);
        assert_eq!(out[0]["turns"][0]["target_start"], 2);
        assert_eq!(out[0]["turns"][0]["target_end"], 3);
        assert_eq!(out[0]["turns"][1]["target_start"], 4);
    }
    #[test]
    fn changed_past_and_overlapping_loss_ranges_are_rejected() {
        let first = row("train_a", 0, vec![10, 11, 99], 2);
        assert!(
            run(
                &[
                    first.clone(),
                    row("train_a", 1, vec![10, 12, 99, 12, 13, 99], 4)
                ],
                &["train_a"]
            )
            .is_err()
        );
        assert!(
            run(
                &[first, row("train_a", 1, vec![10, 11, 99, 12, 13, 99], 3)],
                &["train_a"]
            )
            .is_err()
        );
    }
    #[test]
    fn partition_whitelist_digest_and_complete_coverage_are_enforced() {
        let correct = row("train_a", 0, vec![10, 11, 99], 2);
        let mut validation = correct.clone();
        validation["partition"] = "validation".into();
        assert!(run(&[validation], &["train_a"]).is_err());
        assert!(run(std::slice::from_ref(&correct), &["train_b"]).is_err());
        assert!(run(std::slice::from_ref(&correct), &["train_a", "train_b"]).is_err());
        let mut corrupt = correct;
        corrupt["ids"][0] = 12.into();
        assert!(run(&[corrupt], &["train_a"]).is_err());
    }
    #[test]
    fn missing_turns_duplicate_tasks_and_bad_terminal_are_rejected() {
        let first = row("train_a", 0, vec![10, 11, 99], 2);
        let other = row("train_b", 0, vec![10, 11, 99], 2);
        assert!(
            run(
                &[first.clone(), other, first.clone()],
                &["train_a", "train_b"]
            )
            .is_err()
        );
        assert!(
            run(
                &[first, row("train_a", 2, vec![10, 11, 99, 12, 13, 99], 4)],
                &["train_a"]
            )
            .is_err()
        );
        assert!(run(&[row("train_a", 0, vec![10, 11, 98], 2)], &["train_a"]).is_err());
    }
}
