use std::{collections::BTreeMap, fmt, fs::File, io::Read, path::Path, time::Duration};

use reqwest::Url;
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use zeroize::Zeroizing;

use crate::{ApiError, MAX_CONFIG_BYTES, MAX_OUTPUT_TOKENS, Result};

/// Non-secret, serializable limits. The orchestration layer must enforce these
/// across *all* calls/worker roles belonging to a goal, including failed calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct GoalLimits {
    pub max_calls: u32,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConfigSummary {
    pub endpoint_origin: String,
    pub model: String,
    pub reasoning_effort: String,
    pub api_style: &'static str,
    pub timeout_seconds: u64,
    pub max_output_tokens: u32,
    pub goal_limits: GoalLimits,
}

/// Only [`ApiConfig::summary`] is intended for UI/status serialization.
pub struct ApiConfig {
    pub(crate) endpoint: Url,
    pub(crate) api_key: SecretString,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) timeout: Duration,
    pub(crate) max_output_tokens: u32,
    pub(crate) goal_limits: GoalLimits,
}

impl fmt::Debug for ApiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiConfig")
            .field("summary", &self.summary())
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl ApiConfig {
    /// Read an explicit UTF-8 dotenv file with a bounded safe grammar. No
    /// interpolation, shell execution, implicit discovery or env mutation.
    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|_| ApiError::ConfigRead)?;
        let metadata = file.metadata().map_err(|_| ApiError::ConfigRead)?;
        if !metadata.is_file() || metadata.len() > MAX_CONFIG_BYTES as u64 {
            return Err(ApiError::Limit("configuration file"));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        file.take(MAX_CONFIG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ApiError::ConfigRead)?;
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(ApiError::Limit("configuration file"));
        }
        Self::parse(&bytes, false)
    }

    pub fn summary(&self) -> ConfigSummary {
        ConfigSummary {
            endpoint_origin: self.endpoint.origin().ascii_serialization(),
            model: self.model.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
            api_style: "openai-responses",
            timeout_seconds: self.timeout.as_secs(),
            max_output_tokens: self.max_output_tokens,
            goal_limits: self.goal_limits,
        }
    }

    pub(crate) fn parse(bytes: &[u8], test_loopback: bool) -> Result<Self> {
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(ApiError::Limit("configuration file"));
        }
        let mut fields = parse_fields(bytes)?;
        let style = take_or(&mut fields, "NORA_LLM_API_STYLE", "openai-responses");
        if style.as_str() != "openai-responses" {
            return Err(ApiError::Config("only openai-responses is supported"));
        }
        let base = take_or(
            &mut fields,
            "NORA_LLM_BASE_URL",
            "https://api.openai.com/v1",
        );
        let mut endpoint = Url::parse(&base).map_err(|_| ApiError::Config("invalid endpoint"))?;
        let loopback = test_loopback
            && endpoint.scheme() == "http"
            && endpoint.host_str().is_some_and(|h| h == "127.0.0.1");
        if !(endpoint.scheme() == "https" || loopback)
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(ApiError::Config(
                "endpoint must be credential-free HTTPS with no query or fragment",
            ));
        }
        let base_path = endpoint.path().trim_end_matches('/');
        if base_path.contains('%') || base_path.split('/').any(|s| matches!(s, "." | "..")) {
            return Err(ApiError::Config("ambiguous endpoint path"));
        }
        let path = format!("{base_path}/responses");
        endpoint.set_path(&path);
        let mut key = fields
            .remove("NORA_LLM_API_KEY")
            .ok_or(ApiError::MissingApiKey)?;
        if key.is_empty() {
            return Err(ApiError::MissingApiKey);
        }
        if key.len() > 4096 || !key.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
            return Err(ApiError::Config(
                "API key contains whitespace, controls or excessive bytes",
            ));
        }
        let model = take_or(&mut fields, "NORA_LLM_MODEL", "");
        if model.is_empty() {
            return Err(ApiError::MissingModel);
        }
        if model.len() > 256
            || !model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
        {
            return Err(ApiError::Config("invalid model identifier"));
        }
        if model.contains(key.as_str()) || endpoint.as_str().contains(key.as_str()) {
            return Err(ApiError::Config(
                "credential must not appear in the model or endpoint",
            ));
        }
        let reasoning = take_or(&mut fields, "NORA_LLM_REASONING_EFFORT", "xhigh");
        if !matches!(
            reasoning.as_str(),
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
        ) {
            return Err(ApiError::Config("invalid reasoning effort"));
        }
        let timeout_seconds = number(&mut fields, "NORA_LLM_TIMEOUT_SECONDS", 60, 1, 300)?;
        let max_output_tokens = number(
            &mut fields,
            "NORA_LLM_MAX_OUTPUT_TOKENS",
            2048,
            64,
            u64::from(MAX_OUTPUT_TOKENS),
        )? as u32;
        let goal_limits = GoalLimits {
            max_calls: number(&mut fields, "NORA_LLM_MAX_CALLS_PER_GOAL", 24, 1, 64)? as u32,
            max_input_tokens: number(
                &mut fields,
                "NORA_LLM_MAX_INPUT_TOKENS_PER_GOAL",
                48_000,
                1,
                1_000_000,
            )?,
            max_output_tokens: number(
                &mut fields,
                "NORA_LLM_MAX_OUTPUT_TOKENS_PER_GOAL",
                12_000,
                64,
                128_000,
            )?,
        };
        if u64::from(max_output_tokens) > goal_limits.max_output_tokens {
            return Err(ApiError::Config(
                "per-turn output exceeds per-goal output budget",
            ));
        }
        if fields.keys().any(|k| k.starts_with("NORA_LLM_")) {
            return Err(ApiError::Config("unknown NORA_LLM configuration field"));
        }
        Ok(Self {
            endpoint,
            api_key: SecretString::from(std::mem::take(&mut *key)),
            model: model.to_string(),
            reasoning_effort: reasoning.to_string(),
            timeout: Duration::from_secs(timeout_seconds),
            max_output_tokens,
            goal_limits,
        })
    }

    pub(crate) fn key(&self) -> &str {
        self.api_key.expose_secret()
    }
}

type Fields = BTreeMap<String, Zeroizing<String>>;

fn take_or(fields: &mut Fields, key: &str, default: &str) -> Zeroizing<String> {
    fields
        .remove(key)
        .unwrap_or_else(|| Zeroizing::new(default.to_owned()))
}

fn number(fields: &mut Fields, key: &str, default: u64, min: u64, max: u64) -> Result<u64> {
    let value = match fields.remove(key) {
        Some(v) => v
            .parse::<u64>()
            .map_err(|_| ApiError::Config("invalid numeric limit"))?,
        None => default,
    };
    if !(min..=max).contains(&value) {
        return Err(ApiError::Config("numeric limit out of range"));
    }
    Ok(value)
}

fn parse_fields(bytes: &[u8]) -> Result<Fields> {
    let text = std::str::from_utf8(bytes).map_err(|_| ApiError::Config("file must be UTF-8"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut fields = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        if i >= 512 || line.len() > 16_384 {
            return Err(ApiError::Limit("dotenv lines"));
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, raw) = line
            .split_once('=')
            .ok_or(ApiError::Config("expected KEY=value"))?;
        let key = key.trim();
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(ApiError::Config("invalid dotenv field name"));
        }
        let value = parse_value(raw.trim())?;
        if fields.insert(key.to_owned(), value).is_some() {
            return Err(ApiError::Config("duplicate dotenv field"));
        }
    }
    Ok(fields)
}

fn parse_value(raw: &str) -> Result<Zeroizing<String>> {
    let mut value = Zeroizing::new(String::new());
    if let Some(quote) = raw.chars().next().filter(|c| matches!(c, '\'' | '"')) {
        let rest = &raw[1..];
        let mut escaped = false;
        let mut end = None;
        for (index, c) in rest.char_indices() {
            if escaped {
                if !matches!(c, '\\' | '"') {
                    return Err(ApiError::Config("unsupported dotenv escape"));
                }
                value.push(c);
                escaped = false;
            } else if c == '\\' && quote == '"' {
                escaped = true;
            } else if c == quote {
                end = Some(index + 1);
                break;
            } else {
                value.push(c);
            }
        }
        let suffix = &rest[end.ok_or(ApiError::Config("unterminated dotenv quote"))?..];
        if !suffix.trim().is_empty() && !suffix.trim_start().starts_with('#') {
            return Err(ApiError::Config("unexpected text after dotenv quote"));
        }
    } else {
        value.push_str(raw.split(" #").next().unwrap_or("").trim());
    }
    if value.contains(['$', '`', '\0', '\r', '\n']) {
        return Err(ApiError::Config(
            "dotenv interpolation, commands and multiline values are not supported",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn config(extra: &str) -> String {
        format!("NORA_LLM_API_KEY=synthetic-test-key\nNORA_LLM_MODEL=gpt-6-luna\n{extra}")
    }

    #[test]
    fn config_is_explicit_redacted_and_does_not_install_environment() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("local.env");
        std::fs::write(&path, config("NORA_LLM_REASONING_EFFORT=xhigh\nNORA_CONFIG_PARSE_PROBE_NO_MUTATION=synthetic-sentinel\n")).unwrap();
        // Inspect only a synthetic sentinel, never a credential environment
        // variable belonging to the developer or the test runner.
        let before = std::env::var_os("NORA_CONFIG_PARSE_PROBE_NO_MUTATION");
        let config = ApiConfig::load(&path).unwrap();
        assert_eq!(
            std::env::var_os("NORA_CONFIG_PARSE_PROBE_NO_MUTATION"),
            before
        );
        assert_eq!(config.summary().model, "gpt-6-luna");
        assert_eq!(config.summary().reasoning_effort, "xhigh");
        assert_eq!(config.summary().goal_limits.max_calls, 24);
        assert_eq!(config.summary().goal_limits.max_input_tokens, 48_000);
        assert_eq!(config.summary().goal_limits.max_output_tokens, 12_000);
        assert_eq!(
            config.endpoint.as_str(),
            "https://api.openai.com/v1/responses"
        );
        assert!(!format!("{config:?}").contains("synthetic-test-key"));
        assert!(
            !serde_json::to_string(&config.summary())
                .unwrap()
                .contains("synthetic-test-key")
        );
    }

    #[test]
    fn configuration_rejects_insecure_or_ambiguous_targets_and_missing_key() {
        for endpoint in [
            "http://127.0.0.1:3000/v1",
            "https://name:password@example.com/v1",
            "https://example.com/v1?secret=x",
            "https://example.com/v1#x",
            "file:///tmp/x",
        ] {
            assert!(
                ApiConfig::parse(
                    config(&format!("NORA_LLM_BASE_URL={endpoint}")).as_bytes(),
                    false
                )
                .is_err()
            );
        }
        assert_eq!(
            ApiConfig::parse(b"NORA_LLM_MODEL=gpt-6-luna", false).unwrap_err(),
            ApiError::MissingApiKey
        );
        for extra in [
            "NORA_LLM_API_KEY=other",
            "NORA_LLM_REASONING_EFFORT=typo",
            "NORA_LLM_TIMEOUT_SECONDS=0",
            "NORA_LLM_MAX_OUTPUT_TOKENS=9000",
            "NORA_LLM_EXTRA=ignored",
            "NORA_LLM_API_STYLE=openai-chat-completions",
        ] {
            assert!(ApiConfig::parse(config(extra).as_bytes(), false).is_err());
        }
    }

    #[test]
    fn dotenv_quotes_comments_and_no_interpolation() {
        let config = ApiConfig::parse(b"export NORA_LLM_API_KEY = 'synthetic-test-key' # local only\nNORA_LLM_MODEL=\"gpt-6-luna\"\nNORA_OTHER_SETTING=true", false).unwrap();
        assert_eq!(config.key(), "synthetic-test-key");
        assert!(
            ApiConfig::parse(
                b"NORA_LLM_API_KEY=${OPENAI_API_KEY}\nNORA_LLM_MODEL=gpt-6-luna",
                false
            )
            .is_err()
        );
        assert!(parse_value("\"unterminated").is_err());
        assert!(parse_value("'value' unexpected").is_err());
        assert!(ApiConfig::parse(&vec![b'x'; MAX_CONFIG_BYTES + 1], false).is_err());
    }
}
