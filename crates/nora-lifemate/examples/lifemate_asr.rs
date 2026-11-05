//! Compare real NORA ASR on development audio without loading an LLM.
//! Gold transcripts/labels are never deserialized. This is not task accuracy.
use nora_lifemate::{
    MAX_JSON_BYTES, Result,
    protocol::{InputItem, audio_path},
    runner::{Speech, TrainPartition},
    speech_gpu::GpuSpeech,
};
use serde_json::json;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(6..=7).contains(&args.len()) {
        return Err(
            "expected DATA_ROOT WHISPER_SERVER MODEL OUTPUT_JSONL NEW_LOG LIMIT [BEAM]".into(),
        );
    }
    let beam_size: u32 = args
        .get(6)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1);
    if !(1..=10).contains(&beam_size) {
        return Err("invalid GPU ASR beam size; expected 1..10".into());
    }
    let root = PathBuf::from(&args[0]).canonicalize()?;
    let split = root.join("train").canonicalize()?;
    let limit: usize = args[5].parse()?;
    if !(1..=2000).contains(&limit) {
        return Err("invalid development ASR bound".into());
    }
    let mut output = File::create_new(&args[3])?;
    let mut dataset = BufReader::new(File::open(split.join("train.jsonl"))?);
    let startup = Instant::now();
    let mut speech = GpuSpeech::start_with_beam(
        &PathBuf::from(&args[1]).canonicalize()?,
        &PathBuf::from(&args[2]).canonicalize()?,
        &PathBuf::from(&args[4]),
        beam_size,
    )?;
    eprintln!(
        "NORA ASR ready after {} ms; beam {beam_size}; development-only",
        startup.elapsed().as_millis(),
    );
    let mut line = Vec::new();
    let mut count = 0;
    loop {
        line.clear();
        if dataset
            .by_ref()
            .take((MAX_JSON_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            break;
        }
        if line.len() > MAX_JSON_BYTES {
            return Err("training input exceeds bound".into());
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let item: InputItem = serde_json::from_slice(&line)?;
        item.validate()?;
        if !TrainPartition::Development.accepts(&item.id) {
            continue;
        }
        let started = Instant::now();
        let deadline = started + Duration::from_secs(30);
        let mut history = Vec::new();
        for turn in &item.history {
            if turn.role == "user" {
                let relative = turn.audio.as_deref().ok_or("missing history audio")?;
                history.push(speech.transcribe(&audio_path(&split, relative)?, deadline)?);
            }
        }
        let latest = speech.transcribe(&audio_path(&split, &item.audio)?, deadline)?;
        serde_json::to_writer(
            &mut output,
            &json!({"scope":"development ASR only; not task accuracy",
            "id":item.id,"latest_request":latest,"history":history,
            "asr_ms":started.elapsed().as_secs_f64()*1000.0}),
        )?;
        output.write_all(b"\n")?;
        output.flush()?;
        count += 1;
        if count >= limit {
            break;
        }
    }
    output.sync_all()?;
    eprintln!("NORA ASR completed {count} development samples; no LLM or gold input");
    Ok(())
}
