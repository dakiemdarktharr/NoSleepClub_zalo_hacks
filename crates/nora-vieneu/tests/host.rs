use std::{fs::File, io::Write, path::Path};

use nora_vieneu::{Heads, Limits, ModelConfig, ModelPack, Prompt, SpeakerProjection, VoicePreset};
use tokenizers::{Tokenizer, models::wordlevel::WordLevel, pre_tokenizers::whitespace::Whitespace};
use zip::{ZipWriter, write::SimpleFileOptions};

fn config() -> ModelConfig {
    serde_json::from_value(serde_json::json!({
        "text_vocab_size":8,"audio_vocab_size":4,"n_vq":2,"hidden_size":2,
        "num_hidden_layers":1,"num_attention_heads":1,"num_key_value_heads":1,
        "local_num_hidden_layers":1,"local_num_attention_heads":1,
        "max_position_embeddings":32,"audio_pad_token_id":4,
        "text_prompt_start_token_id":1,"text_prompt_end_token_id":2,
        "speech_generation_start_token_id":3,"speech_generation_end_token_id":4,
        "audio_ref_slot_token_id":5,"default_style_token_id":0,"unk_token_id":0
    }))
    .unwrap()
}

fn npy(shape: &[usize], values: &[f32], dtype: &str, fortran: bool) -> Vec<u8> {
    let shape = if shape.is_empty() {
        String::new()
    } else {
        shape
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
            + ","
    };
    let mut header = format!(
        "{{'descr': '{dtype}', 'fortran_order': {}, 'shape': ({shape}), }}",
        if fortran { "True" } else { "False" }
    );
    let pad = (64 - (10 + header.len() + 1) % 64) % 64;
    header.extend(std::iter::repeat_n(' ', pad));
    header.push('\n');
    let mut out = b"\x93NUMPY\x01\x00".to_vec();
    out.extend((header.len() as u16).to_le_bytes());
    out.extend(header.as_bytes());
    for value in values {
        out.extend(value.to_le_bytes());
    }
    out
}

fn archive(path: &Path, text: Vec<u8>, audio: Vec<u8>) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    zip.start_file("text_emb.npy", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&text).unwrap();
    zip.start_file("audio_emb.npy", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&audio).unwrap();
    zip.finish().unwrap();
}

fn minimal_pack(root: &Path) {
    let cfg = config();
    std::fs::write(root.join("config.json"), serde_json::to_vec(&cfg).unwrap()).unwrap();
    for name in [
        "vieneu_prefill.onnx",
        "vieneu_decode_step.onnx",
        "vieneu_acoustic_cached.onnx",
        "vieneu_backbone_shared.data",
        "moss_audio_tokenizer_decode_full.onnx",
        "moss_audio_tokenizer_decode_shared.data",
    ] {
        // Host-loader fixture only. Never presented as a runnable ONNX model.
        std::fs::write(root.join(name), b"host-loader-fixture").unwrap();
    }
    archive(
        &root.join("vieneu_v3_heads.npz"),
        npy(&[8, 2], &[0.0; 16], "<f4", false),
        npy(&[2, 4, 2], &[0.0; 16], "<f4", false),
    );
    let model = WordLevel::builder()
        .vocab(
            [
                ("[UNK]".to_owned(), 0),
                ("hi".to_owned(), 6),
                ("there".to_owned(), 7),
            ]
            .into_iter()
            .collect(),
        )
        .unk_token("[UNK]".to_owned())
        .build()
        .unwrap();
    let mut tokenizer = Tokenizer::new(model);
    tokenizer.with_pre_tokenizer(Some(Whitespace));
    tokenizer.save(root.join("tokenizer.json"), false).unwrap();
}

#[test]
fn prompt_rows_and_embeddings_match_hand_computed_reference() {
    let cfg = config();
    let limits = Limits::default();
    let voice = VoicePreset {
        speaker_embedding: None,
        ref_codes: Some(vec![vec![1, 2]]),
    };
    let prompt = Prompt::from_token_ids(&[6], &voice, &cfg, &limits).unwrap();
    assert_eq!(prompt.rows(), [0, 4, 4, 1, 4, 4, 6, 4, 4, 2, 4, 4, 5, 1, 2]);
    let text = (0..16).map(|n| n as f32).collect();
    let audio = (0..16).map(|n| 100.0 + n as f32).collect();
    let heads = Heads::from_arrays(&cfg, &limits, text, audio, None).unwrap();
    let embedded = heads.embed_prompt(&prompt, 4, None).unwrap();
    assert_eq!(&embedded[..8], [0.0, 1.0, 2.0, 3.0, 12.0, 13.0, 4.0, 5.0]);
    assert_eq!(&embedded[8..], [224.0, 227.0]);
    assert!(Prompt::from_token_ids(&[8], &voice, &cfg, &limits).is_err());
}

#[test]
fn projection_and_tied_head_matvec_are_finite_and_shaped() {
    let mut cfg = config();
    cfg.use_speaker_embedding = true;
    cfg.speaker_embedding_dim = 2;
    let p = SpeakerProjection {
        weight: vec![1.0, 0.0, 0.0, 1.0],
        bias: vec![0.0; 2],
        norm_weight: vec![1.0; 2],
        norm_bias: vec![0.0; 2],
        epsilon: 1e-5,
        input_dim: 2,
    };
    let heads = Heads::from_arrays(
        &cfg,
        &Limits::default(),
        vec![1.0; 16],
        vec![1.0; 16],
        Some(p),
    )
    .unwrap();
    let anchor = heads.speaker_anchor(Some(&[1.0, 3.0])).unwrap().unwrap();
    assert!((anchor[0] + 1.0).abs() < 0.0001 && (anchor[1] - 1.0).abs() < 0.0001);
    assert!(heads.speaker_anchor(Some(&[0.0, 0.0])).is_err());
    let mut logits = [0.0; 4];
    heads.audio_logits(0, &[2.0, 3.0], &mut logits).unwrap();
    assert_eq!(logits, [5.0; 4]);
    assert!(
        heads
            .audio_logits(0, &[f32::NAN, 1.0], &mut logits)
            .is_err()
    );
}

#[test]
fn npz_reader_checks_header_before_allocation() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("heads.npz");
    let cfg = config();
    let limits = Limits::default();
    let audio = npy(&[2, 4, 2], &[0.0; 16], "<f4", false);
    archive(&p, npy(&[8, 2], &[0.0; 16], "<f4", false), audio.clone());
    assert!(Heads::load_npz(&p, &cfg, &limits).is_ok());
    for bad in [
        npy(&[usize::MAX, 2], &[], "<f4", false),
        npy(&[8, 2], &[0.0; 16], ">f4", false),
        npy(&[8, 2], &[0.0; 16], "<f4", true),
        npy(&[8, 2], &[f32::NAN; 16], "<f4", false),
        npy(&[8, 2], &[], "<f4", false),
    ] {
        archive(&p, bad, audio.clone());
        assert!(Heads::load_npz(&p, &cfg, &limits).is_err());
    }
}

#[test]
fn pack_rejects_unknown_symbols_paths_and_budgets() {
    let dir = tempfile::tempdir().unwrap();
    minimal_pack(dir.path());
    let pack = ModelPack::open(dir.path(), dir.path(), Limits::default()).unwrap();
    assert_eq!(
        pack.prompt("hi there", &VoicePreset::default())
            .unwrap()
            .row_count(),
        5
    );
    assert!(pack.prompt("unknown", &VoicePreset::default()).is_err());
    assert!(pack.model_file("../outside").is_err());
    assert!(pack.model_file("/absolute").is_err());
    assert!(pack.load_voice(Path::new("../../private.json")).is_err());
    let limits = Limits {
        max_pack_bytes: 10,
        ..Default::default()
    };
    assert!(ModelPack::open(dir.path(), dir.path(), limits).is_err());
}

#[test]
fn missing_and_malformed_model_assets_are_explicit_errors() {
    let dir = tempfile::tempdir().unwrap();
    assert!(ModelPack::open(dir.path(), dir.path(), Limits::default()).is_err());
    minimal_pack(dir.path());
    std::fs::write(dir.path().join("config.json"), b"{}").unwrap();
    assert!(ModelPack::open(dir.path(), dir.path(), Limits::default()).is_err());
    let mut cfg = config();
    cfg.hidden_size = usize::MAX;
    assert!(cfg.validate(&Limits::default()).is_err());
    let cfg = config();
    let limits = Limits {
        max_state_bytes: 1,
        ..Default::default()
    };
    assert!(cfg.check_backbone_state(10, &limits).is_err());
}

#[test]
fn upstream_voice_catalog_resolves_names_aliases_and_default() {
    let dir = tempfile::tempdir().unwrap();
    minimal_pack(dir.path());
    std::fs::write(dir.path().join("voices.json"),serde_json::to_vec(&serde_json::json!({
        "default_voice":"Nora","presets":{"Nora":{"speaker_emb":null,"codes":[[1,2]],"aliases":["old-name"],"description":"test"}}
    })).unwrap()).unwrap();
    let pack = ModelPack::open(dir.path(), dir.path(), Limits::default()).unwrap();
    assert_eq!(
        pack.load_voice_catalog(Path::new("voices.json"), None)
            .unwrap()
            .0,
        "Nora"
    );
    assert_eq!(
        pack.load_voice_catalog(Path::new("voices.json"), Some("old-name"))
            .unwrap()
            .0,
        "Nora"
    );
    assert!(
        pack.load_voice_catalog(Path::new("voices.json"), Some("unknown"))
            .is_err()
    );
    std::fs::write(dir.path().join("voices.json"),serde_json::to_vec(&serde_json::json!({
        "default_voice":"Nora","presets":{"Nora":{"speaker_emb":null,"codes":null,"aliases":null}}
    })).unwrap()).unwrap();
    assert!(
        pack.load_voice_catalog(Path::new("voices.json"), None)
            .is_ok()
    );
    std::fs::write(
        dir.path().join("voices.json"),
        serde_json::to_vec(&serde_json::json!({
            "default_voice":"Nora","presets":{
                "Nora":{"speaker_emb":null,"codes":null,"aliases":["ambiguous"]},
                "Other":{"speaker_emb":null,"codes":null,"aliases":["ambiguous"]}
            }
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(
        pack.load_voice_catalog(Path::new("voices.json"), Some("ambiguous"))
            .is_err()
    );
}
