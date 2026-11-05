//! CPU-only training data preparation. Never part of the inference path.
use nora_lifemate::{Result, cohort::EvaluationCohort, training_data};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

fn sha(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("usage: lifemate-pack-trajectories INPUT INPUT_SHA TRAIN_IDS TRAIN_IDS_SHA OUTPUT REPORT (Qwen3-4B tokenizer)".into());
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[4]);
    let report = PathBuf::from(&args[5]);
    if output.exists() || report.exists() || output == report || sha(&input)? != args[1] {
        return Err("input hash mismatch or output/report already exists".into());
    }
    let allowed = EvaluationCohort::from_file(Path::new(&args[2]), &args[3])?;
    let parent = output.parent().ok_or("output parent required")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    let result = training_data::pack(
        BufReader::new(File::open(&input)?),
        staged.as_file_mut(),
        &allowed,
        151645,
        151936,
    )?;
    if sha(&input)? != args[1] {
        return Err("input changed during preparation".into());
    }
    let receipt = json!({"scope":"training-only full trajectories; not model accuracy", "status":"complete",
        "input_sha256":args[1],"train_ids_sha256":allowed.sha256(),"output_sha256":sha(staged.path())?,
        "counts":result,"all_prefix_tokens_and_targets_retained":true,"training_or_quality_evaluated":false,
        "uniform_task_loss":"(sample_count/turn_count)*sum(turn_mean_token_CE)","sequential_optimizer_updates_equivalent":false});
    let mut receipt_file =
        tempfile::NamedTempFile::new_in(report.parent().ok_or("report parent required")?)?;
    serde_json::to_writer_pretty(receipt_file.as_file_mut(), &receipt)?;
    receipt_file.write_all(b"\n")?;
    staged.persist_noclobber(output)?;
    receipt_file.persist_noclobber(report)?;
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
