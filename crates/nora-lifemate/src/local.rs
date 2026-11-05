//! The only model transport is a numeric loopback address owned by this run.
//! No cloud configuration, proxy, redirects, tools service or remote URL exists.
use crate::{MAX_CONTEXT_TOKENS, MAX_JSON_BYTES, Result, bridge::gpu_command};
use serde::{Serialize, Serializer, ser::SerializeMap, ser::SerializeSeq};
use serde_json::{Value, json};
use std::{
    fs::File,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelMode {
    Legacy,
    Compact,
    CompactTyped,
    ActorV2,
    ActorV2Progressive,
    /// Opt-in decoder/runtime checks for clarification field membership.
    CompactTypedClarification,
    /// Opt-in schema-name-alignment experiment; original SDK authority is unchanged.
    CompactTypedAliases,
    /// Same typed request path as CompactTyped, with fixed synthetic examples.
    CompactTypedFewShot,
    /// Opt-in research boundary, never the default or desktop inference path.
    PlanTyped,
    Typed,
    Native,
    NativeGreedy,
    NativeCompact,
    XlamNative,
    XlamSeparateRequest,
    XlamProgressive,
    NativeThinking,
}
impl ModelMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "legacy" => Ok(Self::Legacy),
            "compact" => Ok(Self::Compact),
            "compact-typed" => Ok(Self::CompactTyped),
            "actor-v2-experimental" => Ok(Self::ActorV2),
            "actor-v2-progressive-experimental" => Ok(Self::ActorV2Progressive),
            "compact-typed-clarify-experimental" => Ok(Self::CompactTypedClarification),
            "compact-typed-alias-experimental" => Ok(Self::CompactTypedAliases),
            "compact-typed-fewshot-experimental" => Ok(Self::CompactTypedFewShot),
            "plan-typed-experimental" => Ok(Self::PlanTyped),
            "typed" => Ok(Self::Typed),
            "native" => Ok(Self::Native),
            "native-greedy" => Ok(Self::NativeGreedy),
            "native-compact" => Ok(Self::NativeCompact),
            "xlam-native" => Ok(Self::XlamNative),
            "xlam-separate-request-experimental" => Ok(Self::XlamSeparateRequest),
            "xlam-progressive" => Ok(Self::XlamProgressive),
            "native-thinking" => Ok(Self::NativeThinking),
            _ => Err("invalid local model mode".into()),
        }
    }
    fn is_native(self) -> bool {
        matches!(
            self,
            Self::Native
                | Self::NativeGreedy
                | Self::NativeCompact
                | Self::XlamNative
                | Self::XlamSeparateRequest
                | Self::XlamProgressive
                | Self::NativeThinking
        )
    }
}

pub struct LocalModel {
    child: Child,
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    origin: String,
    _authentication: tempfile::TempDir,
    pub context_tokens: u32,
    pub model_path: PathBuf,
    pub adapter_path: Option<PathBuf>,
    mode: ModelMode,
    tools: Vec<crate::protocol::ToolSpec>,
    proposal_tools: Vec<crate::protocol::ToolSpec>,
    last_response: Option<Value>,
    prompt_cache: bool,
    catalog_disclosure: bool,
    examples: Option<crate::retrieval::ExampleIndex>,
}

/// Explicit local assets; adapters are never discovered or downloaded implicitly.
/// The experiment supervisor pins both files and verifies their compatibility.
#[derive(Clone, Copy, Debug)]
pub struct ModelAssets<'a> {
    pub base: &'a Path,
    pub lora: Option<&'a Path>,
}

/// Placement is explicit so a small GPU can reserve space for the ASR worker.
/// Defaults retain full GPU offload; no automatic accuracy or timing claim.
#[derive(Clone, Copy, Debug)]
pub struct ModelRuntime {
    pub prompt_cache: bool,
    pub gpu_layers: Option<u32>,
    pub cpu_output: bool,
    pub catalog_disclosure: bool,
    batch_tokens: u32,
    microbatch_tokens: u32,
}
impl ModelRuntime {
    pub fn parse(prompt_cache: bool, gpu_layers: &str) -> Result<Self> {
        let gpu_layers = if gpu_layers == "all" {
            None
        } else {
            let layers = gpu_layers
                .parse::<u32>()
                .map_err(|_| "invalid GPU layer count; expected all or 0..256")?;
            if layers > 256 {
                return Err("invalid GPU layer count; expected all or 0..256".into());
            }
            Some(layers)
        };
        Ok(Self {
            prompt_cache,
            gpu_layers,
            cpu_output: false,
            catalog_disclosure: false,
            batch_tokens: 256,
            microbatch_tokens: 128,
        })
    }

    pub fn with_cpu_output(mut self, enabled: bool) -> Self {
        self.cpu_output = enabled;
        self
    }

    pub fn with_catalog_disclosure(mut self, enabled: bool) -> Self {
        self.catalog_disclosure = enabled;
        self
    }

    /// Opt-in prefill experiment. These bounds are not a memory-fit guarantee;
    /// the process supervisor must still enforce the measured resource limits.
    pub fn with_prefill_batch(mut self, batch: u32, microbatch: u32) -> Result<Self> {
        if !(32..=512).contains(&batch)
            || !(32..=512).contains(&microbatch)
            || !batch.is_power_of_two()
            || !microbatch.is_power_of_two()
            || microbatch > batch
        {
            return Err(
                "prefill batch sizes must be powers of two in 32..512, microbatch <= batch".into(),
            );
        }
        self.batch_tokens = batch;
        self.microbatch_tokens = microbatch;
        Ok(self)
    }
}

impl LocalModel {
    pub fn start(
        executable: &Path,
        model: &Path,
        context_tokens: u32,
        device: &str,
        log: &Path,
        mode: ModelMode,
        options: ModelRuntime,
    ) -> Result<Self> {
        Self::start_with_assets(
            executable,
            ModelAssets {
                base: model,
                lora: None,
            },
            context_tokens,
            device,
            log,
            mode,
            options,
        )
    }

    pub fn start_with_assets(
        executable: &Path,
        assets: ModelAssets<'_>,
        context_tokens: u32,
        device: &str,
        log: &Path,
        mode: ModelMode,
        options: ModelRuntime,
    ) -> Result<Self> {
        let model = assets.base;
        if !(1024..=MAX_CONTEXT_TOKENS).contains(&context_tokens)
            || options.catalog_disclosure && !mode.is_native()
            || !model.is_file()
            || !executable.is_file()
            || device.is_empty()
            || !device.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err("invalid offline model or context limit".into());
        }
        let adapter_path = assets.lora.map(Path::canonicalize).transpose()?;
        if let Some(adapter) = &adapter_path
            && (!adapter.is_file() || *adapter == model.canonicalize()?)
        {
            return Err("LoRA must be a separate local adapter file".into());
        }
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let file = File::create_new(log)?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| "local service authentication entropy unavailable")?;
        let secret = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let authentication = tempfile::Builder::new()
            .prefix("nora-llama-auth-")
            .tempdir_in(log.parent().ok_or("model log has no parent")?)?;
        let secret_file = authentication.path().join("key");
        std::fs::write(&secret_file, &secret)?;
        let runtime = crate::async_transport::runtime()?;
        let mut headers = reqwest::header::HeaderMap::new();
        let mut authorization =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}"))?;
        authorization.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, authorization);
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            // Do not keep idle sockets between synchronous model/ASR phases.
            .pool_max_idle_per_host(0)
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(30))
            .build()?;
        let mut command = gpu_command(executable);
        command
            .arg("--model")
            .arg(model)
            .args(["--device", device, "--split-mode", "none"])
            .arg("--api-key-file")
            .arg(&secret_file)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--ctx-size",
                &context_tokens.to_string(),
                "--parallel",
                "1",
                "--cache-ram",
                "0",
                "--ctx-checkpoints",
                "4",
                "--n-gpu-layers",
                &options
                    .gpu_layers
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "all".into()),
                "--fit",
                "off",
                "--load-mode",
                "none",
                "--no-host",
                "--flash-attn",
                "on",
                "--batch-size",
                &options.batch_tokens.to_string(),
                "--ubatch-size",
                &options.microbatch_tokens.to_string(),
                "--threads",
                "8",
                "--threads-batch",
                "8",
                "--cache-type-k",
                "q8_0",
                "--cache-type-v",
                "q8_0",
                "--seed",
                "42",
                "--temp",
                "0",
                "--offline",
                "--no-webui",
                "--no-ui-mcp-proxy",
                "--no-slots",
                "--no-context-shift",
                "--reasoning",
                if mode == ModelMode::NativeThinking {
                    "on"
                } else {
                    "off"
                },
                "--reasoning-format",
                "deepseek",
                "--reasoning-budget",
                if mode == ModelMode::NativeThinking {
                    "128"
                } else {
                    "0"
                },
                if options.prompt_cache {
                    "--cache-prompt"
                } else {
                    "--no-cache-prompt"
                },
            ]);
        if options.cpu_output {
            // Qwen3.5 ties output to token_embd.weight. Match both possible
            // names so the vocabulary projection stays on CPU in either model.
            command.args(["--override-tensor", r"^(output|token_embd)\.weight$=CPU"]);
        }
        if let Some(adapter) = &adapter_path {
            // Native LoRA applies alpha/rank at scale 1. Do not merge the base,
            // alter adapter weights, or fall back silently if loading fails.
            command.arg("--lora").arg(adapter);
        }
        if matches!(
            mode,
            ModelMode::XlamNative | ModelMode::XlamSeparateRequest | ModelMode::XlamProgressive
        ) {
            // xLAM emits a bare array. Preserve the model's prompt, but let
            // our strict array adapter handle output instead of native PEG.
            command.arg("--skip-chat-parsing");
        }
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::from(file.try_clone()?))
            .stderr(Stdio::from(file))
            .spawn()?;
        let mut instance = Self {
            child,
            client,
            runtime,
            origin: format!("http://127.0.0.1:{port}"),
            _authentication: authentication,
            context_tokens,
            model_path: model.to_owned(),
            adapter_path,
            mode,
            tools: Vec::new(),
            proposal_tools: Vec::new(),
            last_response: None,
            examples: None,
            prompt_cache: options.prompt_cache,
            catalog_disclosure: options.catalog_disclosure
                || matches!(
                    mode,
                    ModelMode::XlamProgressive | ModelMode::ActorV2Progressive
                ),
        };
        let started = Instant::now();
        loop {
            if instance.child.try_wait()?.is_some() {
                return Err(
                    "offline model process exited during startup; inspect its local log".into(),
                );
            }
            if started.elapsed() > Duration::from_secs(180) {
                return Err("offline model startup deadline exceeded".into());
            }
            if let Ok(value) = instance.get("/health", Duration::from_secs(1))
                && value.get("status").and_then(Value::as_str) == Some("ok")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(instance)
    }
    /// Experimental worked examples are admitted only in the direct V2 actor.
    pub fn with_training_examples(
        mut self,
        examples: crate::retrieval::ExampleIndex,
    ) -> Result<Self> {
        if self.mode != ModelMode::ActorV2 || self.catalog_disclosure {
            return Err("worked examples require the full V2 experimental actor".into());
        }
        self.examples = Some(examples);
        Ok(self)
    }
    fn get(&self, route: &str, timeout: Duration) -> Result<Value> {
        self.runtime.block_on(async {
            let mut response = self
                .client
                .get(format!("{}{route}", self.origin))
                .timeout(timeout)
                .send()
                .await?;
            if !response.status().is_success() {
                return Err("offline model is not ready".into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if bytes.len().saturating_add(chunk.len()) > MAX_JSON_BYTES {
                    return Err("model response exceeds bound".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(serde_json::from_slice(&bytes)?)
        })
    }
    fn post(&self, route: &str, body: &Value, timeout: Duration) -> Result<Value> {
        let encoded = if self.mode == ModelMode::PlanTyped {
            crate::planning::request_json(body)?
        } else if matches!(
            self.mode,
            ModelMode::Typed
                | ModelMode::Compact
                | ModelMode::CompactTyped
                | ModelMode::ActorV2
                | ModelMode::ActorV2Progressive
                | ModelMode::CompactTypedClarification
                | ModelMode::CompactTypedAliases
                | ModelMode::CompactTypedFewShot
        ) {
            serde_json::to_vec(&ModelRequestJson(body))?
        } else {
            serde_json::to_vec(body)?
        };
        if encoded.len() > MAX_JSON_BYTES {
            return Err("offline model request exceeds bound".into());
        }
        self.runtime.block_on(async {
            let mut response = self
                .client
                .post(format!("{}{route}", self.origin))
                .timeout(timeout)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(encoded)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        "local model generation deadline exceeded"
                    } else {
                        "local model transport failed"
                    }
                })?;
            if !response.status().is_success() {
                return Err(format!("offline model HTTP {}", response.status().as_u16()).into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if bytes.len().saturating_add(chunk.len()) > MAX_JSON_BYTES {
                    return Err("model response exceeds bound".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(serde_json::from_slice(&bytes)?)
        })
    }
    pub fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        let started = Instant::now();
        self.last_response = None;
        if self.child.try_wait()?.is_some() {
            return Err("offline model process is no longer alive".into());
        }
        let augmented = self
            .examples
            .as_mut()
            .map(|index| index.augment(messages, &self.tools))
            .transpose()?;
        let messages = augmented
            .as_ref()
            .map_or(messages, |(wire, _)| wire.as_slice());
        let mut request = json!({"messages":messages,"temperature":0,"seed":42,"max_tokens":768,
            "cache_prompt":self.prompt_cache,"stream":false,"chat_template_kwargs":{"enable_thinking":false},
            "response_format":{"type":"json_schema","json_schema":{"name":"nora_action","schema":schema}}});
        if matches!(self.mode, ModelMode::Legacy | ModelMode::Compact) {
            request["response_format"]["json_schema"]["schema"] =
                crate::protocol::legacy_decision_schema(&self.tools);
        } else if self.mode.is_native() {
            request
                .as_object_mut()
                .ok_or("invalid model request")?
                .remove("response_format");
            request["messages"] = json!(if self.mode == ModelMode::XlamSeparateRequest {
                crate::native_tools::messages_with_separate_request(messages)?
            } else {
                crate::native_tools::messages(messages)?
            });
            request["tools"] = json!(crate::native_tools::functions(
                &self.tools,
                self.catalog_disclosure
                    || matches!(
                        self.mode,
                        ModelMode::NativeCompact
                            | ModelMode::XlamNative
                            | ModelMode::XlamSeparateRequest
                            | ModelMode::XlamProgressive
                    )
            ));
            request["tool_choice"] = json!("auto");
            if self.mode == ModelMode::Native {
                request["temperature"] = json!(0.7);
                request["top_p"] = json!(0.8);
                request["top_k"] = json!(20);
                request["min_p"] = json!(0.0);
                request["presence_penalty"] = json!(1.5);
            } else if self.mode == ModelMode::NativeThinking {
                request["temperature"] = json!(0.6);
                request["top_p"] = json!(0.95);
                request["top_k"] = json!(20);
                request["min_p"] = json!(0.0);
                request["presence_penalty"] = json!(0.0);
                request["max_tokens"] = json!(1024);
                request["chat_template_kwargs"]["enable_thinking"] = json!(true);
            }
        }
        let remaining = remaining
            .checked_sub(started.elapsed())
            .filter(|time| !time.is_zero())
            .ok_or("local preprocessing exhausted generation deadline")?;
        let mut response = self.post("/v1/chat/completions", &request, remaining)?;
        if let Some((_, receipt)) = augmented {
            response["nora_training_example"] = receipt;
        }
        self.last_response = Some(response.clone());
        let raw_choice = response.pointer("/choices/0").cloned();
        if matches!(
            self.mode,
            ModelMode::XlamNative | ModelMode::XlamSeparateRequest | ModelMode::XlamProgressive
        ) {
            let mut normalized = crate::xlam::normalize(response, &self.proposal_tools)?;
            normalized["nora_native_raw_choice"] = raw_choice.unwrap_or(Value::Null);
            Ok(normalized)
        } else if self.mode.is_native() {
            let mut normalized = crate::native_tools::normalize(response)?;
            normalized["nora_native_raw_choice"] = raw_choice.unwrap_or(Value::Null);
            Ok(normalized)
        } else {
            Ok(response)
        }
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

/// Preserve decision order for llama.cpp's grammar compiler without changing
/// serde_json map semantics in any desktop consent, audit or SDK transport.
/// A tool name is selected before its arguments, and finished follows actions.
struct ModelRequestJson<'a>(&'a Value);

impl Serialize for ModelRequestJson<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        const FIRST: [&str; 4] = ["tool_calls", "finished", "name", "arguments"];
        match self.0 {
            Value::Object(object) => {
                let mut output = serializer.serialize_map(Some(object.len()))?;
                for key in FIRST {
                    if let Some(value) = object.get(key) {
                        output.serialize_entry(key, &Self(value))?;
                    }
                }
                for (key, value) in object {
                    if !FIRST.contains(&key.as_str()) {
                        output.serialize_entry(key, &Self(value))?;
                    }
                }
                output.end()
            }
            Value::Array(array) => {
                let mut output = serializer.serialize_seq(Some(array.len()))?;
                for value in array {
                    output.serialize_element(&Self(value))?;
                }
                output.end()
            }
            value => value.serialize(serializer),
        }
    }
}

impl crate::runner::DecisionModel for LocalModel {
    fn take_diagnostic_response(&mut self) -> Option<Value> {
        self.last_response.take()
    }
    fn progressive_catalog(&self) -> bool {
        self.catalog_disclosure
    }
    fn typed_plan(&self) -> bool {
        self.mode == ModelMode::PlanTyped
    }
    fn tool_format_instructions(&self) -> Option<&'static str> {
        matches!(
            self.mode,
            ModelMode::XlamNative | ModelMode::XlamSeparateRequest | ModelMode::XlamProgressive
        )
        .then_some(crate::xlam::FORMAT_INSTRUCTIONS)
    }
    fn actor_v2(&self) -> bool {
        matches!(
            self.mode,
            ModelMode::ActorV2 | ModelMode::ActorV2Progressive
        )
    }
    fn compact_prompt(&self) -> bool {
        self.catalog_disclosure
            || matches!(
                self.mode,
                ModelMode::Compact
                    | ModelMode::CompactTyped
                    | ModelMode::ActorV2
                    | ModelMode::ActorV2Progressive
                    | ModelMode::CompactTypedClarification
                    | ModelMode::CompactTypedAliases
                    | ModelMode::CompactTypedFewShot
                    | ModelMode::PlanTyped
                    | ModelMode::NativeCompact
                    | ModelMode::XlamNative
                    | ModelMode::XlamSeparateRequest
                    | ModelMode::XlamProgressive
            )
    }
    fn schema_aliases(&self) -> bool {
        self.mode == ModelMode::CompactTypedAliases
    }
    fn schema_bound_clarification(&self) -> bool {
        self.mode == ModelMode::CompactTypedClarification
    }
    fn few_shot_prompt(&self) -> bool {
        self.mode == ModelMode::CompactTypedFewShot
    }
    fn native_tools(&self) -> bool {
        self.mode.is_native()
    }
    fn begin_sample(&mut self, tools: &[crate::protocol::ToolSpec]) -> Result<()> {
        self.tools = tools.to_vec();
        self.proposal_tools = tools.to_vec();
        Ok(())
    }
    fn begin_progressive_sample(
        &mut self,
        exposed: &[crate::protocol::ToolSpec],
        all_proposals: &[crate::protocol::ToolSpec],
    ) -> Result<()> {
        self.tools = exposed.to_vec();
        self.proposal_tools = all_proposals.to_vec();
        Ok(())
    }
    fn complete(
        &mut self,
        messages: &[Value],
        schema: &Value,
        remaining: Duration,
    ) -> Result<Value> {
        LocalModel::complete(self, messages, schema, remaining)
    }
}
impl Drop for LocalModel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_adapter_assets_fail_before_creating_log_or_spawning() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.gguf");
        let log = dir.path().join("must-not-exist.log");
        std::fs::write(&base, b"fixture, not an executable").unwrap();
        for adapter in [
            base.clone(),
            dir.path().join("absent.gguf"),
            dir.path().to_owned(),
        ] {
            assert!(
                LocalModel::start_with_assets(
                    &base,
                    ModelAssets {
                        base: &base,
                        lora: Some(&adapter)
                    },
                    4096,
                    "CUDA0",
                    &log,
                    ModelMode::ActorV2,
                    ModelRuntime::parse(true, "all").unwrap(),
                )
                .is_err()
            );
            assert!(!log.exists());
        }
    }

    #[test]
    fn few_shot_mode_is_distinct_from_the_default_typed_mode() {
        assert_eq!(
            ModelMode::parse("compact-typed").unwrap(),
            ModelMode::CompactTyped
        );
        assert_eq!(
            ModelMode::parse("compact-typed-fewshot-experimental").unwrap(),
            ModelMode::CompactTypedFewShot
        );
        assert!(!ModelMode::CompactTypedFewShot.is_native());
    }

    #[test]
    fn generation_request_selects_actions_and_tool_name_before_completion_and_arguments() {
        let body = json!({"response_format":{"schema":{"properties":{
            "finished":{"type":"boolean"},
            "tool_calls":{"items":{"properties":{
                "arguments":{"type":"object"},"name":{"const":"toy.read"}
            }}}
        }}},"messages":[{"content":"literal name and arguments stay unchanged"}]});
        let encoded = serde_json::to_string(&ModelRequestJson(&body)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&encoded).unwrap(), body);
        assert!(encoded.contains(r#""properties":{"tool_calls":"#));
        assert!(encoded.contains(r#""properties":{"name":{"const":"toy.read"},"arguments":"#));
    }
}
