# nora-core

Pure Rust routing, notification, resource admission and typed broker policy.
It does not execute tools, classify speech, authenticate callers or sandbox OS
processes. No unsafe code is permitted in this crate.

- `routing`: structured task facts choose answer/tool/durable-goal independently
  from input/permission/approval gates. Every effect still needs broker checks.
- `notifications`: decisions match the checked-in Python reference for valid
  events. Callers supply persisted `DeliveryState`; this function does not mark
  speech delivered. `verified` must come from an authenticated verifier, never a
  worker's self-declared JSON.
- `resources`: bounded reservation IDs, checked arithmetic and explicit release.
  These are admission estimates, not RSS/VRAM measurements or OS enforcement.
- `policy`: exact goal/account/tool/target/effect scope, source destination/class
  checks, critical-field/source constraints, narrowed delegation and expiring
  approval bindings. Model requests are egress too. Source labels, constraints
  and grants are trusted broker inputs, separate from an action proposal.

Use `from_json_bounded` on untrusted message bytes before deserialization and
then the entry point's validation. Public Serde types do not establish authority.
Size/depth limits intentionally reject oversized events/JSON and whitespace-only
IDs that a looser reference caller might have accepted.

`canonical_sha256` sorts object keys and streams bytes into SHA256, with limits.
It binds exact content, not RFC8785-normalized numbers, a signature or an identity.
Approvals bind action ID/version, complete grant, data labels and field evidence;
the durable broker must additionally consume/track actions to prevent replay.

Canonical targets must already be adapter-resolved identities. This module does
not provide filesystem containment, URL/DNS checks, secret classification,
prompt-injection detection, native confirmation UI or authenticated IPC.
Adapters must execute the checked canonical target, not a conflicting unchecked
destination inside model arguments. Network/model calls require an explicit
trusted source record even for public or broker-generated request content.

Run `cargo test -p nora-core` and `cargo clippy -p nora-core --all-targets -- -D warnings`.
