//! Trusted supervisor integration. Model proposals never become authority.
pub mod assets;
pub mod audio_bridge;
pub mod audio_worker;
pub mod backend;
pub mod budget;
pub mod config;
pub mod diagnostics;
mod event_outbox;
mod leases;
pub mod live_check;
pub mod runner;
pub mod tools;
pub mod voice;
pub mod wav;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub fn now_ms() -> Result<i64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

pub fn new_id(prefix: &str) -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "system randomness unavailable")?;
    use std::fmt::Write;
    let mut id = String::with_capacity(prefix.len() + 33);
    id.push_str(prefix);
    id.push('-');
    for byte in bytes {
        write!(id, "{byte:02x}")?;
    }
    Ok(id)
}

pub fn truncate_utf8(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
