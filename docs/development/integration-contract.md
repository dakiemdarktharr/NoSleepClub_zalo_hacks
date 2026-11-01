# Rust desktop integration contracts

Implementation started 2026-11-01 from parent `3107385f080713775b228bb941f0508ec9cddee4`. The integrated implementation and subsequent updates are maintained directly on `main`.

This is an implementation contract, not a readiness certificate. Lead owns root manifests/lock, runtime orchestration, packaging, hardware harness and release evidence. Implementers each own one crate directory and coordinate public API changes before changing them. Never read, print, copy or commit `.env.local` from an implementer; only the lead performs opt-in live API tests.

## LLM transport (`nora-llm`)

OpenAI Responses API, exact user model `gpt-6-luna`, reasoning effort `xhigh`. `store=false`, explicit local user file; no hidden provider/model fallback. Never expose the key through Debug/errors/telemetry. Responses function schema is flat (`type:function`, `name`, `description`, `parameters`, `strict`). Tool results use `function_call_output`. Preserve opaque output items needed by reasoning/tool continuation without displaying reasoning to users.

Public surface to agree with lead: `ApiConfig::load(&Path)`; redacted configuration summary; `ResponsesClient::new(config)`; synchronous facade `respond(&TurnRequest, &AtomicBool) -> Result<TurnResponse>` that cancels pending HTTP work promptly using an async implementation internally. Request carries `instructions`, `input:Vec<Value>`, `tools:Vec<Value>` and optional output ceiling. Response carries bounded final text, typed function calls (`call_id`, `name`, parsed object `arguments`), usage, raw bounded opaque output items and completion status. No tools execute inside the transport.

Enforce request/body/item/token bounds, HTTPS and same-origin credentials, no redirects, no implicit system proxies, bounded deadlines, explicit 429/timeout/incomplete handling and no logging provider bodies. Optional pure/local HTTP mock testing is isolated from production HTTPS rules. Lead tests paid API only after a local key exists; user authorized those tests.

## Scoped broker (`nora-broker`)

Public surface to agree with lead: file-backed grants owned by a single trusted process; `Broker::open(state_dir)`, add/revoke/list roots with read/create/LLM disclosure flags; `execute` for a typed `ToolAction` plus goal/session identity and explicit approval proof; preview bindings cannot be minted by model content. Scope local file reads/list/search/create-new notes, app allowlist/open, calculator/time and durable reminders as justified. No generic shell, package installation, arbitrary process arguments, unrestricted web fetch or automatic message sending.

All paths rooted by capability handles, no traversal/ADS/devices, file-size/entry limits, symlink/junction/race tests, no overwrite/delete by default. Local file contents are untrusted data. Local-only grant does not permit contents/paths to be sent to the API: broker must gate disclosure. Write/launch previews require exact argument bindings, expiry, revocation/cancel recheck and verified readback/receipt. Use no external application during tests. Persist grants atomically, avoid corrupt/foreign state adoption, and expose structured receipts/errors without secret content. Tests use temp roots and synthetic data.

## Desktop shell (`nora-desktop`)

Windows-first native Rust UI with eframe/egui, glow only, accesskit, bounded chat/events and on-demand repainting. Do not create a WebView/browser runtime. Native folder picker, explicit mic/PTT controls, draft/approval cards, goals/history, grants/revoke, quiet mode, pause/cancel/stop-speech distinct, clear model/config status, background tray show/quit if buildable. Never open mic or network before caller/user intent. No secrets in UI.

Publish a `contracts` module using std bounded channels: UI commands and runtime events, `AppSnapshot`/grant/goal/preview DTOs with only serializable/public content. Keep runtime/transport out of the UI crate; the lead will wire a launcher binary to the shell library. Lead and implementer agree command/event variants before integration. Native-only dependencies gated to Windows for portable core CI; no pretend Linux desktop support. UI tests should exercise state transitions/disabled controls/bounds; real screenshots and click journeys are lead release checks.

## Hardware/release

Target full NORA-owned process tree: 2,000,000,000 bytes RAM ceiling, device GPU usage <=4,000,000,000 bytes, installed runtime/assets/state/update staging <=15,000,000,000 bytes. Include startup/model loading and failure/recovery peaks. No local LLM. Serialize ASR/TTS model residency; enforce OS commit limits where possible and meter working set separately. External user apps launched by explicit approval are reported separately and are not a pretext to hide NORA-owned browsers/children.

No claim of signing, native sandbox, human voice quality, API success or hardware compliance without the corresponding actual evidence. No microphone/background monitoring/autostart is enabled merely by tests/building the source. Final gates must name unmet conditions.
