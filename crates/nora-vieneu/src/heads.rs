// Modified Rust translation of VieNeu's tied embeddings, speaker projection
// and output heads. Original Apache-2.0 attribution is retained in NOTICE.
use std::{fs::File, io::Read, path::Path, sync::OnceLock};

use ndarray::{ArrayView1, ArrayView2, ArrayViewMut1, linalg::general_mat_vec_mul};
use regex::Regex;
use zip::ZipArchive;

use crate::{Error, Limits, ModelConfig, Prompt, Result, checked_elements, invalid, io_error};

pub struct Heads {
    text: Vec<f32>,
    audio: Vec<f32>,
    hidden: usize,
    text_vocab: usize,
    audio_vocab: usize,
    channels: usize,
    speaker: Option<SpeakerProjection>,
}

pub struct SpeakerProjection {
    pub weight: Vec<f32>,
    pub bias: Vec<f32>,
    pub norm_weight: Vec<f32>,
    pub norm_bias: Vec<f32>,
    pub epsilon: f32,
    pub input_dim: usize,
}

impl Heads {
    pub fn from_arrays(
        config: &ModelConfig,
        limits: &Limits,
        text: Vec<f32>,
        audio: Vec<f32>,
        speaker: Option<SpeakerProjection>,
    ) -> Result<Self> {
        config.validate(limits)?;
        if text.len() != checked_elements(&[config.text_vocab_size, config.hidden_size])?
            || audio.len()
                != checked_elements(&[config.n_vq, config.audio_vocab_size, config.hidden_size])?
            || text.iter().chain(&audio).any(|v| !v.is_finite())
        {
            return Err(invalid("embedding shape or nonfinite values"));
        }
        if config.use_speaker_embedding {
            let s = speaker
                .as_ref()
                .ok_or_else(|| invalid("speaker projection missing"))?;
            if s.input_dim != config.speaker_embedding_dim
                || s.weight.len() != checked_elements(&[config.hidden_size, s.input_dim])?
                || s.bias.len() != config.hidden_size
                || s.norm_weight.len() != config.hidden_size
                || s.norm_bias.len() != config.hidden_size
                || !s.epsilon.is_finite()
                || s.epsilon <= 0.0
                || s.weight
                    .iter()
                    .chain(&s.bias)
                    .chain(&s.norm_weight)
                    .chain(&s.norm_bias)
                    .any(|v| !v.is_finite())
            {
                return Err(invalid("speaker projection dimensions/values"));
            }
        } else if speaker.is_some() {
            return Err(invalid("unexpected speaker projection"));
        }
        Ok(Self {
            text,
            audio,
            hidden: config.hidden_size,
            text_vocab: config.text_vocab_size,
            audio_vocab: config.audio_vocab_size,
            channels: config.n_vq,
            speaker,
        })
    }

    pub fn load_npz(path: &Path, config: &ModelConfig, limits: &Limits) -> Result<Self> {
        config.validate(limits)?;
        let file = File::open(path).map_err(|e| io_error(path, e))?;
        if file.metadata().map_err(|e| io_error(path, e))?.len() > limits.max_heads_bytes {
            return Err(Error::Limit("heads archive file size".into()));
        }
        let mut archive = ZipArchive::new(file).map_err(|e| invalid(format!("heads ZIP: {e}")))?;
        if archive.len() > 16 {
            return Err(Error::Limit("heads archive entry count".into()));
        }
        let mut total = 0u64;
        let mut names = std::collections::HashSet::new();
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| invalid(format!("heads ZIP entry: {e}")))?;
            let name = file
                .name()
                .map_err(|e| invalid(format!("heads ZIP name: {e}")))?;
            if !names.insert(name.into_owned()) {
                return Err(invalid("duplicate heads archive entry"));
            }
            total = total
                .checked_add(file.size())
                .ok_or_else(|| Error::Limit("uncompressed archive overflow".into()))?;
            if total > limits.max_heads_bytes {
                return Err(Error::Limit("heads decompression budget".into()));
            }
        }
        let text = read_f32_npy(
            &mut archive,
            "text_emb.npy",
            &[config.text_vocab_size, config.hidden_size],
            limits,
        )?;
        let audio = read_f32_npy(
            &mut archive,
            "audio_emb.npy",
            &[config.n_vq, config.audio_vocab_size, config.hidden_size],
            limits,
        )?;
        let speaker = if config.use_speaker_embedding {
            Some(SpeakerProjection {
                weight: read_f32_npy(
                    &mut archive,
                    "xvec_w.npy",
                    &[config.hidden_size, config.speaker_embedding_dim],
                    limits,
                )?,
                bias: read_f32_npy(&mut archive, "xvec_b.npy", &[config.hidden_size], limits)?,
                norm_weight: read_f32_npy(
                    &mut archive,
                    "xvec_ln_w.npy",
                    &[config.hidden_size],
                    limits,
                )?,
                norm_bias: read_f32_npy(
                    &mut archive,
                    "xvec_ln_b.npy",
                    &[config.hidden_size],
                    limits,
                )?,
                epsilon: read_f32_npy(&mut archive, "xvec_ln_eps.npy", &[], limits)?[0],
                input_dim: config.speaker_embedding_dim,
            })
        } else {
            None
        };
        Self::from_arrays(config, limits, text, audio, speaker)
    }

    pub fn speaker_anchor(&self, input: Option<&[f32]>) -> Result<Option<Vec<f32>>> {
        let Some(p) = &self.speaker else {
            return Ok(None);
        };
        let input = input.ok_or_else(|| invalid("speaker embedding required"))?;
        if input.len() != p.input_dim
            || input.iter().any(|v| !v.is_finite())
            || !input.iter().any(|v| *v != 0.0)
        {
            return Err(invalid("invalid/all-zero speaker embedding"));
        }
        let mut output = vec![0.0; self.hidden];
        matvec(&p.weight, self.hidden, p.input_dim, input, &mut output)?;
        for (v, b) in output.iter_mut().zip(&p.bias) {
            *v += b;
        }
        let mean = output.iter().sum::<f32>() / self.hidden as f32;
        let variance =
            output.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / self.hidden as f32;
        let denom = (variance + p.epsilon).sqrt();
        for (i, v) in output.iter_mut().enumerate() {
            *v = (*v - mean) / denom * p.norm_weight[i] + p.norm_bias[i];
        }
        finite(&output)?;
        Ok(Some(output))
    }

    pub fn embed_prompt(
        &self,
        prompt: &Prompt,
        audio_pad: i64,
        anchor: Option<&[f32]>,
    ) -> Result<Vec<f32>> {
        let mut output = vec![0.0; checked_elements(&[prompt.row_count(), self.hidden])?];
        self.embed_rows(prompt.rows(), audio_pad, anchor, &mut output)?;
        Ok(output)
    }

    pub fn embed_rows(
        &self,
        rows: &[i64],
        audio_pad: i64,
        anchor: Option<&[f32]>,
        output: &mut [f32],
    ) -> Result<()> {
        let width = self.channels + 1;
        if !rows.len().is_multiple_of(width) || output.len() != rows.len() / width * self.hidden {
            return Err(invalid("prompt embedding dimensions"));
        }
        if anchor.is_some_and(|v| v.len() != self.hidden || v.iter().any(|x| !x.is_finite())) {
            return Err(invalid("speaker anchor dimensions/values"));
        }
        for (row, dst) in rows
            .chunks_exact(width)
            .zip(output.chunks_exact_mut(self.hidden))
        {
            dst.copy_from_slice(self.text_embedding(row[0])?);
            for channel in 0..self.channels {
                if row[channel + 1] == audio_pad {
                    continue;
                }
                let embedding = self.audio_embedding(channel, row[channel + 1])?;
                for (v, e) in dst.iter_mut().zip(embedding) {
                    *v += e;
                }
            }
            if let Some(anchor) = anchor {
                for (v, a) in dst.iter_mut().zip(anchor) {
                    *v += a;
                }
            }
        }
        finite(output)
    }

    pub fn text_embedding(&self, id: i64) -> Result<&[f32]> {
        if id < 0 || id as usize >= self.text_vocab {
            return Err(invalid("text token outside embedding"));
        }
        let start = id as usize * self.hidden;
        Ok(&self.text[start..start + self.hidden])
    }

    pub fn audio_embedding(&self, ch: usize, id: i64) -> Result<&[f32]> {
        if ch >= self.channels || id < 0 || id as usize >= self.audio_vocab {
            return Err(invalid("audio token outside embedding"));
        }
        let start = (ch * self.audio_vocab + id as usize) * self.hidden;
        Ok(&self.audio[start..start + self.hidden])
    }

    pub fn audio_logits(&self, ch: usize, hidden: &[f32], output: &mut [f32]) -> Result<()> {
        if ch >= self.channels {
            return Err(invalid("audio channel outside embeddings"));
        }
        let start = ch * self.audio_vocab * self.hidden;
        matvec(
            &self.audio[start..start + self.audio_vocab * self.hidden],
            self.audio_vocab,
            self.hidden,
            hidden,
            output,
        )
    }

    pub fn text_logits(&self, hidden: &[f32], output: &mut [f32]) -> Result<()> {
        matvec(&self.text, self.text_vocab, self.hidden, hidden, output)
    }
}

fn finite(values: &[f32]) -> Result<()> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err(invalid("nonfinite model/head result"));
    }
    Ok(())
}

fn matvec(
    matrix: &[f32],
    rows: usize,
    cols: usize,
    input: &[f32],
    output: &mut [f32],
) -> Result<()> {
    if input.len() != cols
        || output.len() != rows
        || matrix.len() != checked_elements(&[rows, cols])?
    {
        return Err(invalid("matvec dimensions"));
    }
    finite(input)?;
    let a = ArrayView2::from_shape((rows, cols), matrix).map_err(|e| invalid(e.to_string()))?;
    let x = ArrayView1::from(input);
    let mut y = ArrayViewMut1::from(&mut *output);
    general_mat_vec_mul(1.0, &a, &x, 0.0, &mut y);
    finite(output)
}

/// Streaming safe-subset NPY reader: validates the shape and byte count BEFORE
/// allocating. No Python literals are evaluated and no ZIP paths are extracted.
fn read_f32_npy(
    archive: &mut ZipArchive<File>,
    name: &str,
    expected: &[usize],
    limits: &Limits,
) -> Result<Vec<f32>> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| invalid(format!("required heads array {name}: {e}")))?;
    let mut first = [0u8; 8];
    file.read_exact(&mut first)
        .map_err(|e| invalid(e.to_string()))?;
    if &first[..6] != b"\x93NUMPY" {
        return Err(invalid("invalid NPY magic"));
    }
    let (header_len, prefix) = match first[6] {
        1 => {
            let mut b = [0; 2];
            file.read_exact(&mut b)
                .map_err(|e| invalid(e.to_string()))?;
            (u16::from_le_bytes(b) as usize, 10usize)
        }
        2 | 3 => {
            let mut b = [0; 4];
            file.read_exact(&mut b)
                .map_err(|e| invalid(e.to_string()))?;
            (u32::from_le_bytes(b) as usize, 12usize)
        }
        _ => return Err(invalid("unsupported NPY format")),
    };
    if first[7] != 0 || header_len == 0 || header_len > 16_384 {
        return Err(invalid("invalid/oversized NPY header"));
    }
    let mut header = vec![0; header_len];
    file.read_exact(&mut header)
        .map_err(|e| invalid(e.to_string()))?;
    let header = std::str::from_utf8(&header).map_err(|e| invalid(e.to_string()))?;
    static HEADER: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    let regexes = HEADER.get_or_init(|| {
        (
            Regex::new(r#"['\"]descr['\"]\s*:\s*['\"]([<>=|]f4)['\"]"#).expect("constant regex"),
            Regex::new(r#"['\"]fortran_order['\"]\s*:\s*(False|True)"#).expect("constant regex"),
            Regex::new(r#"['\"]shape['\"]\s*:\s*\(([^)]*)\)"#).expect("constant regex"),
        )
    });
    let capture = |r: &Regex| -> Result<String> {
        let mut caps = r.captures_iter(header);
        let found = caps
            .next()
            .and_then(|c| c.get(1))
            .map(|s| s.as_str().to_owned())
            .ok_or_else(|| invalid("unsupported NPY header"))?;
        if caps.next().is_some() {
            return Err(invalid("duplicate NPY header key"));
        }
        Ok(found)
    };
    let dtype = capture(&regexes.0)?;
    if (dtype != "<f4" && dtype != "|f4" && !(dtype == "=f4" && cfg!(target_endian = "little")))
        || capture(&regexes.1)? != "False"
    {
        return Err(invalid(
            "only little-endian C-order float32 heads are supported",
        ));
    }
    let shape = capture(&regexes.2)?
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<usize>().map_err(|_| invalid("invalid NPY shape")))
        .collect::<Result<Vec<_>>>()?;
    if shape != expected {
        return Err(invalid(format!("heads shape mismatch for {name}")));
    }
    let count = checked_elements(expected)?;
    let bytes = count
        .checked_mul(4)
        .ok_or_else(|| Error::Limit("heads array byte overflow".into()))?;
    if bytes as u64 > limits.max_heads_bytes || file.size() != (prefix + header_len + bytes) as u64
    {
        return Err(invalid("heads payload length/budget mismatch"));
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::Limit("heads allocation".into()))?;
    let mut buffer = [0u8; 65_536];
    let mut remaining = bytes;
    while remaining > 0 {
        let take = remaining.min(buffer.len());
        file.read_exact(&mut buffer[..take])
            .map_err(|e| invalid(e.to_string()))?;
        for b in buffer[..take].as_chunks::<4>().0 {
            result.push(f32::from_le_bytes(*b));
        }
        remaining -= take;
    }
    let mut end = [0];
    if file.read(&mut end).map_err(|e| invalid(e.to_string()))? != 0 {
        return Err(invalid("trailing NPY bytes"));
    }
    Ok(result)
}
