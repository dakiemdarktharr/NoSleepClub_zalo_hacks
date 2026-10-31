use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

use tokenizers::Tokenizer;

use crate::{
    Error, Heads, Limits, ModelConfig, Result, StreamLayout, VoicePreset, invalid, io_error,
};

/// An explicitly selected, local, trusted graph pack. This loader validates
/// host inputs and file bounds; it is not an ONNX parser sandbox or a signature
/// verifier. External-data references inside graphs must be manifest-reviewed.
pub struct ModelPack {
    model_root: PathBuf,
    codec_root: PathBuf,
    pub(crate) config: ModelConfig,
    pub(crate) heads: Heads,
    pub(crate) tokenizer: Tokenizer,
    pub(crate) limits: Limits,
    pub(crate) stream: Option<StreamLayout>,
}

impl ModelPack {
    pub fn open(
        model_root: impl AsRef<Path>,
        codec_root: impl AsRef<Path>,
        limits: Limits,
    ) -> Result<Self> {
        limits.validate()?;
        let model_root = canonical_dir(model_root.as_ref())?;
        let codec_root = canonical_dir(codec_root.as_ref())?;
        let config_path = local_file(&model_root, "config.json")?;
        let config: ModelConfig = serde_json::from_slice(&read_bounded(&config_path, 65_536)?)
            .map_err(|e| invalid(format!("config JSON: {e}")))?;
        config.validate(&limits)?;
        let mut total = 0u64;
        for name in [
            "vieneu_prefill.onnx",
            "vieneu_decode_step.onnx",
            "vieneu_acoustic_cached.onnx",
            "vieneu_backbone_shared.data",
            "vieneu_v3_heads.npz",
            "tokenizer.json",
            "config.json",
        ] {
            total = add_file_size(
                total,
                &local_file(&model_root, name)?,
                limits.max_pack_bytes,
            )?;
        }
        for name in [
            "moss_audio_tokenizer_decode_full.onnx",
            "moss_audio_tokenizer_decode_shared.data",
        ] {
            total = add_file_size(
                total,
                &local_file(&codec_root, name)?,
                limits.max_pack_bytes,
            )?;
        }
        let meta_path = codec_root.join("codec_browser_onnx_meta.json");
        let stream = if meta_path.exists() {
            let p = local_file(&codec_root, "codec_browser_onnx_meta.json")?;
            let value: serde_json::Value = serde_json::from_slice(&read_bounded(&p, 262_144)?)
                .map_err(|e| invalid(format!("codec metadata JSON: {e}")))?;
            match value.get("streaming_decode").filter(|v| !v.is_null()) {
                Some(value) => {
                    let layout: StreamLayout = serde_json::from_value(value.clone())
                        .map_err(|e| invalid(format!("streaming metadata: {e}")))?;
                    layout.validate(&limits)?;
                    total = add_file_size(
                        total,
                        &local_file(&codec_root, "moss_audio_tokenizer_decode_step.onnx")?,
                        limits.max_pack_bytes,
                    )?;
                    Some(layout)
                }
                None => None,
            }
        } else {
            None
        };
        let _validated_total = total;
        let heads = Heads::load_npz(
            &local_file(&model_root, "vieneu_v3_heads.npz")?,
            &config,
            &limits,
        )?;
        let tok_path = local_file(&model_root, "tokenizer.json")?;
        let tokenizer = Tokenizer::from_bytes(read_bounded(&tok_path, 2_097_152)?)
            .map_err(|e| invalid(format!("tokenizer JSON: {e}")))?;
        if tokenizer.get_truncation().is_some() || tokenizer.get_padding().is_some() {
            return Err(invalid(
                "tokenizer must not silently truncate or pad phonemes",
            ));
        }
        Ok(Self {
            model_root,
            codec_root,
            config,
            heads,
            tokenizer,
            limits,
            stream,
        })
    }

    pub fn config(&self) -> &ModelConfig {
        &self.config
    }
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
    pub fn heads(&self) -> &Heads {
        &self.heads
    }
    pub fn supports_streaming(&self) -> bool {
        self.stream.is_some()
    }
    pub fn model_file(&self, name: &str) -> Result<PathBuf> {
        local_file(&self.model_root, name)
    }
    pub fn codec_file(&self, name: &str) -> Result<PathBuf> {
        local_file(&self.codec_root, name)
    }

    pub fn load_voice(&self, relative: &Path) -> Result<VoicePreset> {
        let path = local_file(&self.model_root, relative)?;
        let voice = serde_json::from_slice::<VoicePreset>(&read_bounded(&path, 262_144)?)
            .map_err(|e| invalid(format!("voice JSON: {e}")))?;
        voice.validate(&self.config, &self.limits)?;
        Ok(voice)
    }

    /// Reads the SDK's voices_v3_turbo.json without Python conversion. Name
    /// matches win over aliases, as in the upstream wrapper. Returned codes
    /// have the historical final encoder-pad frame removed when applicable.
    pub fn load_voice_catalog(
        &self,
        relative: &Path,
        name: Option<&str>,
    ) -> Result<(String, VoicePreset)> {
        #[derive(serde::Deserialize)]
        struct Catalog {
            default_voice: Option<String>,
            presets: std::collections::BTreeMap<String, Entry>,
        }
        #[derive(serde::Deserialize)]
        struct Entry {
            speaker_emb: Option<Vec<f32>>,
            codes: Option<Vec<Vec<i64>>>,
            #[serde(default)]
            aliases: Option<Vec<String>>,
        }
        let path = local_file(&self.model_root, relative)?;
        let catalog: Catalog = serde_json::from_slice(&read_bounded(&path, 4_194_304)?)
            .map_err(|e| invalid(format!("voice catalog JSON: {e}")))?;
        if catalog.presets.is_empty() || catalog.presets.len() > 128 {
            return Err(invalid("voice catalog count"));
        }
        let requested = name
            .or(catalog.default_voice.as_deref())
            .ok_or_else(|| invalid("no preset voice selected/default"))?;
        if requested.is_empty() || requested.len() > 256 {
            return Err(invalid("invalid preset voice name"));
        }
        let selected = if let Some(selected) = catalog.presets.get_key_value(requested) {
            selected
        } else {
            let mut aliases = catalog.presets.iter().filter(|(_, entry)| {
                entry
                    .aliases
                    .as_ref()
                    .is_some_and(|aliases| aliases.iter().any(|alias| alias == requested))
            });
            let selected = aliases
                .next()
                .ok_or_else(|| invalid(format!("preset voice not found: {requested}")))?;
            if aliases.next().is_some() {
                return Err(invalid("ambiguous preset alias"));
            }
            selected
        };
        let mut voice = VoicePreset {
            speaker_embedding: selected.1.speaker_emb.clone(),
            ref_codes: selected.1.codes.clone(),
        };
        if let Some(codes) = &mut voice.ref_codes
            && codes.len() > 1
            && codes.last().and_then(|row| row.first()) == Some(&455)
        {
            codes.pop();
        }
        voice.validate(&self.config, &self.limits)?;
        Ok((selected.0.clone(), voice))
    }

    pub fn prompt(&self, phonemes: &str, voice: &VoicePreset) -> Result<crate::Prompt> {
        crate::Prompt::build(&self.tokenizer, phonemes, voice, &self.config, &self.limits)
    }
}

fn canonical_dir(path: &Path) -> Result<PathBuf> {
    let root = fs::canonicalize(path).map_err(|e| io_error(path, e))?;
    if !root.is_dir() {
        return Err(invalid("model root is not a directory"));
    }
    Ok(root)
}

pub(crate) fn local_file(root: &Path, relative: impl AsRef<Path>) -> Result<PathBuf> {
    let relative = relative.as_ref();
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("asset path must be a relative normal path"));
    }
    let joined = root.join(relative);
    let path = fs::canonicalize(&joined).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::Missing(relative.display().to_string())
        } else {
            io_error(&joined, e)
        }
    })?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(invalid("asset escaped model root or is not a file"));
    }
    Ok(path)
}

pub(crate) fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|e| io_error(path, e))?;
    if file.metadata().map_err(|e| io_error(path, e))?.len() > max {
        return Err(Error::Limit(format!("file too large: {}", path.display())));
    }
    let mut result = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut result)
        .map_err(|e| io_error(path, e))?;
    if result.len() as u64 > max {
        return Err(Error::Limit("file grew during bounded read".into()));
    }
    Ok(result)
}

fn add_file_size(total: u64, path: &Path, max: u64) -> Result<u64> {
    let size = fs::metadata(path).map_err(|e| io_error(path, e))?.len();
    let total = total
        .checked_add(size)
        .ok_or_else(|| Error::Limit("pack size overflow".into()))?;
    if total > max {
        return Err(Error::Limit("model pack disk budget".into()));
    }
    Ok(total)
}
