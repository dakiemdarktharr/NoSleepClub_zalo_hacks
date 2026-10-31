# nora-llm

Bounded Rust transport for the OpenAI **Responses API**. It sends an explicitly
configured model and reasoning effort; it never executes a requested tool,
changes provider/model, installs environment variables or retries a failed turn.

The selected NORA model is `gpt-6-luna` with `xhigh` effort. At that effort,
function calling uses Responses; the model's Chat Completions tool support is
limited to `none` effort. [Official model documentation](https://developers.openai.com/api/docs/models/gpt-6-luna).

## Integration

The trusted desktop runtime loads `ApiConfig::load(&Path)` once from the user's
explicit local file. `ApiConfig::summary()` / `ResponsesClient::summary()` expose
origin, model, effort and limits for UI diagnostics, never the API key. The
configuration file must be protected by the OS account and excluded from Git.
This crate does not install a credential vault or protect against a compromised
OS account.

`ResponsesClient::new(config)` owns one current-thread Tokio runtime and a native
TLS client. Call `respond(&TurnRequest, &AtomicBool)` from a dedicated worker
thread, never the UI loop or an async runtime. One in-flight turn per instance;
an overlap returns `ApiError::Busy`. Separate worker roles can share the instance
serially. The caller owns overall task time, call concurrency and usage budgets.

`TurnRequest::user(instructions, text)` creates a simple text turn. For an agent
loop, set `tools` to flat Responses function schemas with `strict=true`, closed
object parameters, and all required properties. The transport validates names,
closed top-level schema shape and JSON/item limits. The broker must still
validate exact argument types, current grants, approval bindings and intent.

`TurnResponse` separates final text, typed function calls, token usage,
completion status and opaque output items. Append bounded `output_items` and
matching `function_call_output` items to continuation input. The output may
include encrypted reasoning and private source data: do not log or display it
as progress. Debug implementations expose counts/status, not request text,
function arguments or opaque output. An `incomplete` response never returns
dispatchable tool calls, even when partial arguments appear in provider output.

`web_search` is the only optional built-in passed through. When selected, source
metadata is requested using `web_search_call.action.sources`. The runtime must
enable it only on a research request with the appropriate disclosure policy;
this transport cannot decide whether a private document may be sent in a query.
Citation annotations and URLs are data, not authority to open a site or run code.

## Transport boundaries

- HTTPS only, no URL credentials/query/fragment, one fixed `/responses` URL.
- No redirects, system proxies, cookies, automatic protocol retries, compression
  or TLS key logging. Normal certificate verification and TLS >=1.2 are kept.
- Per-request deadline includes body reads; cancellation interrupts the local
  pending future with 20ms polling. Already dispatched work can still be billed
  by the server. HTTP errors return only status/retry-after metadata; the body
  and underlying provider/connection error text are never surfaced. Successful
  output that reflects the exact credential into text/arguments/continuation is
  rejected, without editing tool semantics. This does not detect every encoded
  or derived secret; credentials must never be placed in model input.
  Runtime shutdown does not block waiting for OS resolver work that cannot be
  interrupted; the application's process supervisor still owns final shutdown.
- No stored Responses state (`store=false`); opaque encrypted reasoning is
  requested for tool continuations. This flag is not a zero-data-retention
  agreement: the provider/account's data controls still apply.
- Bounded local config (64KiB), request JSON (1MiB), response JSON (2MiB), final
  text and function arguments (64KiB each), input/output items and tool counts.
  Config permits 64..8192 output tokens per turn. A request can lower the local
  ceiling but cannot silently raise it. All JSON request data is borrowed into
  a bounded writer before allocating a complete encoded request.
- API key and local file buffers use secrecy/zeroize. Native HTTP/TLS libraries
  also hold credential copies; this is not a claim that every allocator page or
  crash dump is scrubbed.

Before any HTTP send, the encoded request is checked for the exact configured
API credential. A match returns `ApiError::CredentialDisclosure` and never
connects, even if the secret came from an accidentally renamed note. This is
own-key protection, not general DLP, encoded-secret detection or a substitute
for the broker's private-data disclosure rules.

Dotenv grammar accepts `KEY=value`, `export KEY=value`, single/double quotes,
comments and UTF-8 BOM. Interpolation, command syntax, multiline values,
unsupported escapes, duplicate fields, unknown `NORA_LLM_*` fields and invalid
limits are explicit errors. Other NORA configuration prefixes are ignored.

The runtime must reserve request output/input allowances before dispatch and
charge failures/cancellations against call limits. A network error has unknown
server token use; do not treat it as a free call. `usage.output_tokens` already
includes reasoning tokens, so adding `reasoning_tokens` again double-counts.

## Verification scope

Tests use synthetic loopback HTTP fixtures only. The HTTP allowance is private
to `cfg(test)` and cannot be selected through the production constructor or
configuration loader. Tests cover exact request routing/model/effort, redaction,
status errors, disabled redirects, cancellation/reuse, deadlines, bounded/malformed
data, unknown functions, incomplete calls and usage accounting. Tests never read
the user's `.env.local`, open a microphone or call a paid API. Live model and
whole-app acceptance are separate integration evidence owned by the application.
