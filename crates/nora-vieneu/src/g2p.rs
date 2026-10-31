//! Native sea-g2p integration. Python bindings are disabled in the dependency.
use std::path::Path;

use sea_g2p_rs::{g2p::G2PEngine, lang::vi::Normalizer};

use crate::{Error, Result, invalid, io_error};

pub struct VietnameseFrontend {
    normalizer: Normalizer,
    engine: G2PEngine,
    max_bytes: usize,
}

impl VietnameseFrontend {
    pub fn load(dictionary: &Path, max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 || max_bytes > 65_536 {
            return Err(invalid("G2P text budget"));
        }
        let dictionary = std::fs::canonicalize(dictionary).map_err(|e| io_error(dictionary, e))?;
        if !dictionary.is_file()
            || dictionary
                .metadata()
                .map_err(|e| io_error(&dictionary, e))?
                .len()
                > 128 * 1024 * 1024
        {
            return Err(invalid("dictionary path/size"));
        }
        let path = dictionary
            .to_str()
            .ok_or_else(|| invalid("dictionary path must be UTF-8"))?;
        // Load the engine first: unlike optional normalizer initialization, a
        // missing/corrupt dictionary must be an error, not silent fallback.
        let engine = G2PEngine::new(path).map_err(|e| io_error(&dictionary, e))?;
        Ok(Self {
            normalizer: Normalizer::new("vi", Some(path)),
            engine,
            max_bytes,
        })
    }

    pub fn phonemize(&self, text: &str) -> Result<String> {
        let normalized = normalize_checked(&self.normalizer, text, self.max_bytes)?;
        let result = self.engine.phonemize(&normalized);
        if result.is_empty() || result.len() > self.max_bytes.saturating_mul(8) {
            return Err(Error::Limit("empty/excessive G2P output".into()));
        }
        Ok(result)
    }
}

pub fn normalize_vietnamese(text: &str, max_bytes: usize) -> Result<String> {
    normalize_checked(&Normalizer::new("vi", None), text, max_bytes)
}

fn normalize_checked(normalizer: &Normalizer, text: &str, max_bytes: usize) -> Result<String> {
    if text.trim().is_empty() || text.len() > max_bytes || max_bytes == 0 || max_bytes > 65_536 {
        return Err(Error::Limit("empty/excessive G2P input".into()));
    }
    if text.chars().any(|c| c.is_control() && !c.is_whitespace()) {
        return Err(invalid("control character in speech text"));
    }
    let unmapped = normalizer.audit(text);
    if !unmapped.is_empty() {
        return Err(invalid(format!(
            "normalization would drop unsupported characters: {}",
            unmapped.join(" ")
        )));
    }
    let normalized = normalizer.normalize(text, true);
    if normalized.is_empty() || normalized.len() > max_bytes.saturating_mul(8) {
        return Err(Error::Limit("normalized text budget".into()));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vietnamese_numbers_and_unsupported_input() {
        let text = normalize_vietnamese("Tôi có 12 quyển sách.", 1024).unwrap();
        assert!(text.contains("mười hai"));
        assert!(normalize_vietnamese("", 1024).is_err());
        assert!(normalize_vietnamese("a\0b", 1024).is_err());
        assert!(normalize_vietnamese("Xin chào", 2).is_err());
    }
}
