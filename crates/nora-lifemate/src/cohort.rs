//! Hash-pinned, ID-only selection for train evaluations. Never model input.
use crate::{MAX_JSON_BYTES, Result, runner::TrainPartition};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

const MAX_COHORT_BYTES: usize = 262_144;

#[derive(Debug, Clone)]
pub struct EvaluationCohort {
    ids: BTreeSet<String>,
    sha256: String,
}

impl EvaluationCohort {
    /// Only a JSON array of unique IDs is accepted; labels and annotations
    /// cannot share this input surface. Check the raw bytes before decoding.
    pub fn from_file(path: &Path, expected_sha256: &str) -> Result<Self> {
        if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("cohort SHA256 must contain 64 hexadecimal characters".into());
        }
        let file = File::open(path)?;
        if !file.metadata()?.is_file() {
            return Err("cohort must be a regular file".into());
        }
        let mut bytes = Vec::new();
        file.take((MAX_COHORT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_COHORT_BYTES {
            return Err("cohort exceeds size bound".into());
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        if sha256 != expected_sha256.to_ascii_lowercase() {
            return Err("cohort bytes differ from pinned SHA256".into());
        }
        let supplied: Vec<String> = serde_json::from_slice(&bytes)?;
        if supplied.is_empty()
            || supplied.len() > 10_000
            || supplied.iter().any(|id| {
                id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
        {
            return Err("invalid cohort IDs or count".into());
        }
        let count = supplied.len();
        let ids: BTreeSet<_> = supplied.into_iter().collect();
        if ids.len() != count {
            return Err("duplicate cohort ID".into());
        }
        Ok(Self { ids, sha256 })
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Reject missing, repeated or partition-excluded selected IDs before
    /// warming models or opening outputs. Other row fields are ignored.
    pub(crate) fn verify_dataset(&self, path: &Path, partition: TrainPartition) -> Result<()> {
        #[derive(Deserialize)]
        struct IdOnly {
            id: String,
        }
        let mut source = BufReader::new(File::open(path)?);
        let mut seen = BTreeSet::new();
        let mut line = Vec::new();
        loop {
            line.clear();
            if source
                .by_ref()
                .take((MAX_JSON_BYTES + 1) as u64)
                .read_until(b'\n', &mut line)?
                == 0
            {
                break;
            }
            if line.len() > MAX_JSON_BYTES {
                return Err("dataset record exceeds bound during cohort preflight".into());
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let item: IdOnly = serde_json::from_slice(&line)?;
            if self.contains(&item.id) && (!partition.accepts(&item.id) || !seen.insert(item.id)) {
                return Err("selected cohort ID repeated or excluded by partition".into());
            }
        }
        if seen != self.ids {
            return Err("selected cohort IDs absent from dataset".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pinned(bytes: &[u8]) -> Result<EvaluationCohort> {
        let mut file = tempfile::NamedTempFile::new()?;
        std::io::Write::write_all(&mut file, bytes)?;
        EvaluationCohort::from_file(file.path(), &format!("{:x}", Sha256::digest(bytes)))
    }

    #[test]
    fn rejects_changed_bytes_duplicate_ids_and_embedded_answers() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), br#"["toy_1"]"#).unwrap();
        assert!(EvaluationCohort::from_file(file.path(), &"0".repeat(64)).is_err());
        for bytes in [
            br#"["toy_1","toy_1"]"#.as_slice(),
            br#"[{"id":"toy_1","label":"answer"}]"#,
            br#"[]"#,
            br#"["../toy_1"]"#,
        ] {
            assert!(pinned(bytes).is_err());
        }
    }

    #[test]
    fn rejects_missing_duplicate_or_outside_partition_ids_before_run() {
        let cohort = pinned(br#"["toy_1","toy_6"]"#).unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        for content in [
            "{\"id\":\"toy_1\"}\n",
            "{\"id\":\"toy_1\"}\n{\"id\":\"toy_6\"}\n{\"id\":\"toy_6\"}\n",
        ] {
            std::fs::write(file.path(), content).unwrap();
            assert!(
                cohort
                    .verify_dataset(file.path(), TrainPartition::All)
                    .is_err()
            );
        }
        std::fs::write(
            file.path(),
            "{\"id\":\"toy_1\",\"label\":\"ignored\"}\n{\"id\":\"toy_6\"}\n",
        )
        .unwrap();
        assert!(
            cohort
                .verify_dataset(file.path(), TrainPartition::All)
                .is_ok()
        );
        assert!(
            cohort
                .verify_dataset(file.path(), TrainPartition::Development)
                .is_err()
        );
    }
}
