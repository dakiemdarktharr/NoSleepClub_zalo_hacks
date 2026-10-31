//! Diagnostic render only: no microphone/playback/network is activated.
use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
    process::ExitCode,
    time::Instant,
};

use nora_vieneu::{
    CancellationToken, Limits, ModelPack, SamplingConfig,
    g2p::VietnameseFrontend,
    onnx::{OnnxEngine, SynthesisOptions},
};

fn parse_options(tail: &[String]) -> Result<SynthesisOptions, String> {
    if tail.len() > 2 {
        return Err("too many diagnostic options".into());
    }
    let (mut frames, mut greedy) = (None, false);
    for argument in tail {
        if argument.eq_ignore_ascii_case("greedy") || argument.eq_ignore_ascii_case("--greedy") {
            if greedy {
                return Err("duplicate GREEDY option".into());
            }
            greedy = true;
        } else {
            if frames.is_some() {
                return Err("duplicate MAX_FRAMES option".into());
            }
            let value = argument
                .parse::<usize>()
                .map_err(|_| "expected MAX_FRAMES or GREEDY")?;
            if value == 0 || value > Limits::default().max_new_frames {
                return Err("MAX_FRAMES outside host limit".into());
            }
            frames = Some(value);
        }
    }
    Ok(SynthesisOptions {
        max_new_frames: frames.unwrap_or(64),
        sampling: SamplingConfig {
            temperature: if greedy { 0.0 } else { 0.8 },
            ..Default::default()
        },
        ..Default::default()
    })
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(6..=8).contains(&args.len()) {
        return Err("usage: vieneu-render ORT_DLL MODEL_DIR CODEC_DIR DICTIONARY OUTPUT_WAV TEXT [MAX_FRAMES] [GREEDY]".into());
    }
    let options = parse_options(&args[6..])?;
    let begin = Instant::now();
    nora_vieneu::configure_single_user_runtime();
    let environment = ort::init_from(&args[0])?.with_telemetry(false).build()?;
    let pack = ModelPack::open(&args[1], &args[2], Limits::default())?;
    let (voice_name, voice) = pack.load_voice_catalog(Path::new("voices_v3_turbo.json"), None)?;
    let frontend = VietnameseFrontend::load(Path::new(&args[3]), pack.limits().max_text_bytes)?;
    let phonemes = frontend.phonemize(&args[5])?;
    let mut engine = OnnxEngine::load(&pack, &environment, 2)?;
    let loaded = begin.elapsed();
    let mut file = BufWriter::with_capacity(65_536, File::create_new(&args[4])?);
    file.write_all(&[0u8; 44])?;
    let mut samples = 0usize;
    let mut first_audio = None;
    let synthesis = Instant::now();
    let result = engine.synthesize_phonemes(
        &phonemes,
        &voice,
        &options,
        &CancellationToken::default(),
        |chunk| {
            if first_audio.is_none() {
                first_audio = Some(synthesis.elapsed());
            }
            for sample in chunk {
                file.write_all(&sample.to_le_bytes())
                    .map_err(|e| nora_vieneu::Error::Runtime(e.to_string()))?;
            }
            samples += chunk.len();
            Ok(())
        },
    )?;
    let bytes = u32::try_from(samples.checked_mul(4).ok_or("WAV length overflow")?)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(b"RIFF")?;
    file.write_all(&(bytes + 36).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&3u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&48_000u32.to_le_bytes())?;
    file.write_all(&192_000u32.to_le_bytes())?;
    file.write_all(&4u16.to_le_bytes())?;
    file.write_all(&32u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    file.flush()?;
    file.get_ref().sync_all()?;
    println!(
        "{}",
        serde_json::json!({"voice":voice_name,"load_ms":loaded.as_millis(),
        "first_audio_ms":first_audio.map(|t|t.as_millis()),"synthesis_ms":synthesis.elapsed().as_millis(),
        "stats":result,"output":args[4],"diagnostic_only":true,"phonemes":phonemes,"options":options})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }
    #[test]
    fn defaults_and_greedy_change_only_requested_parameters() {
        let standard = parse_options(&[]).unwrap();
        assert_eq!(standard.max_new_frames, 64);
        assert_eq!(standard.sampling.temperature, 0.8);
        let greedy = parse_options(&args(&["48", "GREEDY"])).unwrap();
        assert_eq!(greedy.max_new_frames, 48);
        assert_eq!(greedy.sampling.temperature, 0.0);
        assert_eq!(greedy.chunk_frames, 25);
        assert_eq!(greedy.repetition_window, 64);
        assert_eq!(greedy.sampling.top_k, 25);
        assert_eq!(greedy.sampling.top_p, 0.95);
        assert_eq!(greedy.sampling.repetition_penalty, 1.2);
        assert!(greedy.frame_cap);
        assert_eq!(
            parse_options(&args(&["--greedy"]))
                .unwrap()
                .sampling
                .temperature,
            0.0
        );
    }
    #[test]
    fn malformed_or_duplicate_options_fail_before_loading_models() {
        for invalid in [
            vec!["0"],
            vec!["769"],
            vec!["-1"],
            vec!["4", "5"],
            vec!["greedy", "greedy"],
            vec!["unknown"],
        ] {
            assert!(parse_options(&args(&invalid)).is_err());
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
