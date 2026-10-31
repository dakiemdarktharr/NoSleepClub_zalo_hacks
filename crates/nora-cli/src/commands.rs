use std::{
    error::Error,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use nora_core::{
    MAX_JSON_BYTES,
    notifications::{Context, Event, EventKind, Speech, Surface, decide},
    routing::{TaskFacts, route},
};
use nora_store::{EffectStatus, GoalStatus, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::assets;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HELP: &str =
    "NORA Rust foundation (offline; no microphone or model API is started automatically)
  nora replay EVENTS.jsonl [--db STATE.sqlite]
  nora route TASK.json
  nora verify-pack ROOT MANIFEST.json
  nora make-manifest ROOT PACK_ID RELATIVE_FILE...
  nora audio-probe [ITERATIONS]
  nora demo-note ROOT RELATIVE_NAME TEXT_FILE STATE.sqlite GOAL_ID
  nora goal STATE.sqlite GOAL_ID
  nora vad-smoke ORT_DLL SILERO.onnx                  [feature onnx]
  nora render-tts ORT_DLL MODEL_DIR CODEC_DIR DICTIONARY OUTPUT_WAV TEXT [MAX_FRAMES] [--greedy]
                                                      [features onnx,g2p]
  nora transcribe MODEL.ggml PCM_F32LE LANGUAGE       [feature whisper]
";

pub fn run(args: impl Iterator<Item = String>) -> Result<()> {
    let args: Vec<String> = args.collect();
    let Some(command) = args.first().map(String::as_str) else {
        print!("{HELP}");
        return Ok(());
    };
    match command {
        "help" | "--help" | "-h" => {
            print!("{HELP}");
            Ok(())
        }
        "replay" => replay(&args[1..]),
        "route" if args.len() == 2 => {
            let bytes = assets::read_bounded(Path::new(&args[1]), MAX_JSON_BYTES as u64)?;
            let facts: TaskFacts = nora_core::from_json_bounded(&bytes)?;
            print_json(&route(&facts)?)
        }
        "verify-pack" if args.len() == 3 => {
            print_json(&assets::verify(Path::new(&args[1]), Path::new(&args[2]))?)
        }
        "make-manifest" if args.len() >= 4 => print_json(&assets::make_manifest(
            Path::new(&args[1]),
            &args[2],
            &args[3..],
        )?),
        "audio-probe" => audio_probe(&args[1..]),
        "demo-note" if args.len() == 6 => demo_note(
            Path::new(&args[1]),
            &args[2],
            Path::new(&args[3]),
            Path::new(&args[4]),
            &args[5],
        ),
        "goal" if args.len() == 3 => print_json(&Store::open(&args[1])?.goal(&args[2])?),
        #[cfg(feature = "onnx")]
        "vad-smoke" if args.len() == 3 => vad_smoke(Path::new(&args[1]), Path::new(&args[2])),
        #[cfg(all(feature = "onnx", feature = "g2p"))]
        "render-tts" => render_tts(&args[1..]),
        #[cfg(feature = "whisper")]
        "transcribe" if args.len() == 4 => {
            transcribe(Path::new(&args[1]), Path::new(&args[2]), &args[3])
        }
        _ => Err(format!("unsupported command/arguments; use nora --help ({command})").into()),
    }
}

fn print_json(value: &impl Serialize) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, value)?;
    output.write_all(b"\n")?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayRow {
    event: Event,
    #[serde(default)]
    context: Option<Context>,
}

fn replay(args: &[String]) -> Result<()> {
    if !(args.len() == 1 || (args.len() == 3 && args[1] == "--db")) {
        return Err("replay expects a JSONL path and optional --db path".into());
    }
    let store = if args.len() == 3 {
        Store::open(&args[2])?
    } else {
        Store::memory()?
    };
    let mut reader = BufReader::new(File::open(&args[0])?);
    let mut line = Vec::with_capacity(4096);
    let mut line_number = 0u64;
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take((MAX_JSON_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        line_number += 1;
        if line.len() > MAX_JSON_BYTES {
            return Err(format!("input line {line_number} exceeds bounded JSON limit").into());
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let row: ReplayRow = nora_core::from_json_bounded(&line)
            .map_err(|e| format!("invalid input line {line_number}: {e}"))?;
        row.event.validate()?;
        let context = row
            .context
            .unwrap_or_else(|| Context::new(row.event.occurred_at));
        let key = row.event.notice_key();
        let delivery = store.delivery_state(&row.event.goal_id, &key)?;
        let decision = decide(&row.event, &context, delivery)?;
        let mut value = serde_json::to_value(&decision)?;
        let object = value.as_object_mut().ok_or("decision is not an object")?;
        object.insert("simulation".into(), true.into());
        object.insert("event_id".into(), row.event.event_id.clone().into());
        print_json(&value)?;
        // Replay simulates acknowledgements, never speaker/app delivery.
        if decision.surface != Surface::None {
            store.acknowledge(
                &row.event.goal_id,
                &decision.notice_key,
                "surface",
                context.now,
            )?;
        }
        if decision.speech == Speech::Ready {
            store.acknowledge(
                &row.event.goal_id,
                &decision.notice_key,
                "speech",
                context.now,
            )?;
        }
    }
    Ok(())
}

fn audio_probe(args: &[String]) -> Result<()> {
    if args.len() > 1 {
        return Err("audio-probe takes at most one iteration count".into());
    }
    let iterations = args
        .first()
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(100_000);
    if iterations == 0 || iterations > 1_000_000 {
        return Err("iterations must be in 1..=1000000".into());
    }
    let mut ring = nora_audio::PcmRing::new(4096)?;
    let samples = [0.125f32; 512];
    let mut out = [0.0f32; 512];
    let allocated = ring.memory_bytes();
    let started = Instant::now();
    let mut checksum = 0f64;
    for _ in 0..iterations {
        ring.try_push(&samples)?;
        if ring.read_into(&mut out) != out.len() {
            return Err("PCM probe underflow".into());
        }
        checksum += f64::from(out[0]);
        std::hint::black_box(&out);
    }
    print_json(
        &json!({"scope":"PCM ring only; no models, GUI, browser or LLM loaded","iterations":iterations,"samples_processed":iterations*512,"pcm_capacity_bytes":allocated,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"checksum":checksum,"pid":std::process::id()}),
    )
}

fn now_ms() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

/// Explicit local command, not a network-facing permission broker. User-supplied
/// root is opened once as a capability; names cannot escape it or overwrite files.
fn demo_note(root: &Path, name: &str, input: &Path, database: &Path, goal_id: &str) -> Result<()> {
    let effect_id = format!("{goal_id}:note");
    if effect_id.len() > 256 {
        return Err("goal ID plus note suffix exceeds the 256-byte effect ID limit".into());
    }
    let name = assets::relative_path(name)?;
    let bytes = assets::read_bounded(input, 65_536)?;
    let text = std::str::from_utf8(&bytes)?;
    if text.trim().is_empty() {
        return Err("note must have nonempty UTF-8 content".into());
    }
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    let root_canonical = root.canonicalize()?;
    let root_text = root_canonical
        .to_str()
        .ok_or("root path must be representable in UTF-8")?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let name_text = name.to_str().ok_or("note name is not UTF-8")?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &json!({"operation":"create_note","root":root_text,"name":name_text,"content_sha256":hash})
        )?)
    );
    let mut store = Store::open(database)?;
    let objective = format!("Create and verify scoped note {name_text}");
    let mut goal = match store.goal(goal_id) {
        Ok(goal) => goal,
        Err(StoreError::NotFound(_)) => store.create_goal(goal_id, &objective, now_ms()?)?,
        Err(error) => return Err(error.into()),
    };
    if goal.objective != objective {
        return Err("goal ID belongs to a different objective".into());
    }
    let existing = match store.effect(&effect_id) {
        Ok(e) => Some(e),
        Err(StoreError::NotFound(_)) => None,
        Err(e) => return Err(e.into()),
    };
    if let Some(effect) = existing.as_ref() {
        if effect.fingerprint != fingerprint {
            return Err("action content changed; existing goal/effect ID cannot be reused".into());
        }
        if effect.status == EffectStatus::Verified {
            let file = directory.open(name)?;
            let (actual, size) = assets::digest_reader(file, 65_536)?;
            if actual != hash || size != bytes.len() as u64 {
                return Err("verified note was changed or removed; no automatic overwrite".into());
            }
            if matches!(
                goal.status,
                GoalStatus::Running | GoalStatus::NeedsReconciliation
            ) {
                goal = store.transition_goal(
                    goal_id,
                    goal.revision,
                    GoalStatus::Verifying,
                    now_ms()?,
                )?;
            }
            if goal.status == GoalStatus::Verifying {
                goal = store.transition_goal(
                    goal_id,
                    goal.revision,
                    GoalStatus::Completed,
                    now_ms()?,
                )?;
            }
            return print_json(
                &json!({"goal":goal,"effect":effect,"did_write":false,"state_readback_verified":true}),
            );
        }
        if effect.status != EffectStatus::Prepared {
            return Err("effect outcome is uncertain; reconcile before any retry".into());
        }
    }
    if goal.status == GoalStatus::Queued {
        goal = store.transition_goal(goal_id, goal.revision, GoalStatus::Running, now_ms()?)?;
    }
    if goal.status != GoalStatus::Running {
        return Err("goal is not runnable".into());
    }
    if directory.symlink_metadata(name).is_ok() {
        return Err("target already exists; no overwrite was authorized".into());
    }
    store.prepare_effect(&effect_id, goal_id, goal.revision, &fingerprint, now_ms()?)?;
    store.begin_effect(&effect_id, now_ms()?)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut output = directory.open_with(name, &options)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    drop(output);
    let (actual, size) = assets::digest_reader(directory.open(name)?, 65_536)?;
    if actual != hash || size != bytes.len() as u64 {
        return Err("note readback failed; effect requires reconciliation".into());
    }
    let receipt = serde_json::to_string(
        &json!({"kind":"file_readback","relative_path":name_text,"bytes":size,"sha256":actual}),
    )?;
    let effect = store.verify_effect(&effect_id, &receipt, now_ms()?)?;
    goal = store.transition_goal(goal_id, goal.revision, GoalStatus::Verifying, now_ms()?)?;
    goal = store.transition_goal(goal_id, goal.revision, GoalStatus::Completed, now_ms()?)?;
    let time = now_ms()? as f64 / 1000.0;
    let event = Event {
        goal_id: goal_id.into(),
        event_id: effect_id,
        kind: EventKind::GoalCompleted,
        occurred_at: time,
        summary: "Đã lưu note và đối chiếu lại nội dung file.".into(),
        milestone_key: "verified-note".into(),
        verified: true,
        evidence_refs: vec![format!("file-sha256:{actual}")],
        read_source_ids: vec![],
    };
    let update = decide(
        &event,
        &Context::new(time),
        store.delivery_state(goal_id, &event.notice_key())?,
    )?;
    print_json(
        &json!({"goal":goal,"effect":effect,"did_write":true,"state_readback_verified":true,"notification_proposal":update,"speech_actually_played":false}),
    )
}

#[cfg(feature = "onnx")]
fn model_environment(dll: &Path) -> Result<ort::environment::Environment> {
    // Bind the explicit trusted DLL before constructing *any* ORT object.
    // GlobalThreadPoolOptions calls the native API and otherwise can cause the
    // loader to pick an unrelated onnxruntime.dll from the ambient search path.
    let environment = ort::init_from(dll)?.with_telemetry(false);
    let pool = ort::environment::GlobalThreadPoolOptions::default()
        .with_intra_threads(2)?
        .with_inter_threads(1)?
        .with_spin_control(false)?;
    Ok(environment.with_global_thread_pool(pool).build()?)
}

#[cfg(feature = "onnx")]
fn vad_smoke(dll: &Path, model: &Path) -> Result<()> {
    // Explicit user-provided DLL must be trusted; this command does not verify
    // publisher authenticity. Packaging must pin/check it before launch.
    let started = Instant::now();
    let environment = model_environment(dll)?;
    let mut vad = nora_audio::SileroVad::load(&environment, model, 2)?;
    let loaded_ms = started.elapsed().as_secs_f64() * 1000.0;
    let silence = [0.0f32; 512];
    let infer = Instant::now();
    let mut probabilities = Vec::with_capacity(32);
    for _ in 0..32 {
        let p = vad.probability(&silence)?;
        if !p.is_finite() || !(0.0..=1.0).contains(&p) {
            return Err("Silero returned invalid probability".into());
        }
        probabilities.push(p);
    }
    vad.reset();
    print_json(
        &json!({"backend":"ONNX Runtime CPU","scope":"real Silero inference on silence; not recognition-quality evaluation","frames":probabilities.len(),"loaded_ms":loaded_ms,"inference_ms":infer.elapsed().as_secs_f64()*1000.0,"last_probability":probabilities.last(),"pid":std::process::id()}),
    )
}

#[cfg(all(feature = "onnx", feature = "g2p"))]
fn render_tts(args: &[String]) -> Result<()> {
    use nora_vieneu::{
        CancellationToken, Limits, ModelPack,
        g2p::VietnameseFrontend,
        onnx::{OnnxEngine, SynthesisOptions},
    };
    use std::io::{BufWriter, Seek, SeekFrom};
    if !(6..=8).contains(&args.len()) || (args.len() == 8 && args[7] != "--greedy") {
        return Err("render-tts expects DLL,model,codec,dictionary,output,text and optional frame limit/--greedy".into());
    }
    let greedy = args.last().is_some_and(|v| v == "--greedy");
    let frames = args
        .get(6)
        .filter(|v| v.as_str() != "--greedy")
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(300);
    nora_vieneu::configure_single_user_runtime();
    let begin = Instant::now();
    let environment = model_environment(Path::new(&args[0]))?;
    let pack = ModelPack::open(&args[1], &args[2], Limits::default())?;
    let (voice_name, voice) = pack.load_voice_catalog(Path::new("voices_v3_turbo.json"), None)?;
    let frontend = VietnameseFrontend::load(Path::new(&args[3]), pack.limits().max_text_bytes)?;
    let phonemes = frontend.phonemize(&args[5])?;
    let mut engine = OnnxEngine::load(&pack, &environment, 2)?;
    let loaded_ms = begin.elapsed().as_secs_f64() * 1000.0;
    let mut output = BufWriter::with_capacity(65_536, File::create_new(&args[4])?);
    output.write_all(&[0u8; 44])?;
    let mut options = SynthesisOptions {
        max_new_frames: frames,
        ..Default::default()
    };
    if greedy {
        options.sampling.temperature = 0.0;
    }
    let start = Instant::now();
    let mut first_audio_ms = None;
    let mut samples = 0usize;
    let stats = engine.synthesize_phonemes(
        &phonemes,
        &voice,
        &options,
        &CancellationToken::default(),
        |chunk| {
            first_audio_ms.get_or_insert_with(|| start.elapsed().as_secs_f64() * 1000.0);
            for sample in chunk {
                output
                    .write_all(&sample.to_le_bytes())
                    .map_err(|e| nora_vieneu::Error::Runtime(e.to_string()))?;
            }
            samples = samples
                .checked_add(chunk.len())
                .ok_or_else(|| nora_vieneu::Error::Limit("WAV length overflow".into()))?;
            Ok(())
        },
    )?;
    let synthesis_ms = start.elapsed().as_secs_f64() * 1000.0;
    let data_bytes: u32 = samples
        .checked_mul(4)
        .ok_or("WAV size overflow")?
        .try_into()?;
    let riff_bytes = data_bytes.checked_add(36).ok_or("RIFF size overflow")?;
    output.seek(SeekFrom::Start(0))?;
    output.write_all(b"RIFF")?;
    output.write_all(&riff_bytes.to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16u32.to_le_bytes())?;
    output.write_all(&3u16.to_le_bytes())?;
    output.write_all(&1u16.to_le_bytes())?;
    output.write_all(&48_000u32.to_le_bytes())?;
    output.write_all(&192_000u32.to_le_bytes())?;
    output.write_all(&4u16.to_le_bytes())?;
    output.write_all(&32u16.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&data_bytes.to_le_bytes())?;
    output.flush()?;
    output.get_ref().sync_all()?;
    print_json(
        &json!({"scope":"VieNeu CPU + native G2P only; no ASR,GUI,browser or LLM","voice":voice_name,"phonemes":phonemes,"greedy":greedy,"options":options,"stats":stats,"loaded_ms":loaded_ms,"first_audio_ms":first_audio_ms,"synthesis_ms":synthesis_ms,"samples":samples,"audio_seconds":samples as f64/48000.0,"played_audio":false,"pid":std::process::id()}),
    )
}

#[cfg(feature = "whisper")]
fn transcribe(model: &Path, pcm: &Path, language: &str) -> Result<()> {
    let data = assets::read_bounded(pcm, 16_000 * 60 * 4)?;
    if data.is_empty() || data.len() % 4 != 0 {
        return Err("PCM input must contain little-endian f32 samples at16kHz mono".into());
    }
    let samples: Vec<f32> = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let mut whisper = nora_audio::WhisperEngine::load(model, 16_000 * 60, 2)?;
    print_json(&whisper.transcribe_16khz(&samples, language)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_note_is_verified_and_second_run_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("input.txt");
        let db = dir.path().join("state.sqlite");
        std::fs::write(&text, "Biên bản tiếng Việt có kiểm chứng.").unwrap();
        demo_note(dir.path(), "note.md", &text, &db, "goal").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("note.md")).unwrap(),
            "Biên bản tiếng Việt có kiểm chứng."
        );
        demo_note(dir.path(), "note.md", &text, &db, "goal").unwrap();
        assert_eq!(
            Store::open(&db).unwrap().goal("goal").unwrap().status,
            GoalStatus::Completed
        );
        std::fs::write(&text, "Nội dung khác").unwrap();
        assert!(demo_note(dir.path(), "note.md", &text, &db, "goal").is_err());
    }
    #[test]
    fn note_does_not_overwrite_or_escape() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("input.txt");
        let db = dir.path().join("state.sqlite");
        std::fs::write(&text, "new").unwrap();
        std::fs::write(dir.path().join("note.md"), "existing").unwrap();
        assert!(demo_note(dir.path(), "note.md", &text, &db, "goal").is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("note.md")).unwrap(),
            "existing"
        );
        assert!(demo_note(dir.path(), "../outside.md", &text, &db, "other").is_err());
    }
    #[test]
    fn oversized_composed_effect_id_does_not_create_database_or_note() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("input.txt");
        let db = dir.path().join("state.sqlite");
        std::fs::write(&text, "hello").unwrap();
        assert!(demo_note(dir.path(), "note.md", &text, &db, &"g".repeat(256)).is_err());
        assert!(!db.exists());
        assert!(!dir.path().join("note.md").exists());
    }
}
