# LifeMate V2 — architecture and empirical gates

2026-11-03; current status updated 2026-11-04. The V2 milestone is **strictly greater than90%**; the resumed formal goal and main publication gate remain **real100%**. Neither is achieved. An official score requires the organizer's server; local mirrors retain the disclosed fuzzy-tokenization uncertainty.

Current evidence: best reference-text engineering score remains R14 **51.0149%**, with only2/12 nonempty tasks perfect and strict30s score0%. R17Qwen3.5-4B9.6825% and R18FunReason-MT16.8651% do not beat it under the tested constrained interface; these are not author benchmark reproductions. All12 reused development samples and failures are retained. No broad actual-audio/A30/official confirmation.

Training capacity V8 now passes two complete7881/5581-token updates with native RMSNorm chunks256; all504 gradient tensors match the unchanged257-token numerical control exactly. Minimum sampledGPUfree924MiB, no ownedCPUOOM/swap and verified cleanup. This removes the observed long-backward capacity blocker for that recipe. Actual fresh-process resumeV2 now passes exact comparison of all504 adapter tensors, complete optimizer state and CPU/CUDA RNG; its source/resource audit also passes. Real F32 adapter export preserves all504 tensor bytes and alpha/rank. R19 Linux builds with149 passing tests, and its Rust library completes native base/adapter smoke inference. These are transport/capacity gates, not policy quality or HF/GGUF numerical equivalence. Ordered SFT has started on the first64 of1082 internal-train tasks; a complete pass and learned policy evaluation remain pending. Chronological sections below preserve older results and superseded statuses; `.artifacts/nora-current-state.json` records the latest job.


## Assessment of the supplied V1 review

The review correctly prioritizes a direct next-action actor over a mandatory full-plan DSL, preserves installed tools and causal history, separates 2B from 4B evidence, and treats calls plus state as the task objective. It does not yet solve all implementation questions:

1. An evidence pointer copies data but cannot prove the selected person, condition or intent is correct. Namespace coverage must be explicit; missing `returns` metadata cannot be filled by guessing an entity type.
2. `done=false` can mean failure, unavailable information, or an already-satisfied state. Automatic retries and automatic task failure both need outcome-specific semantics. A generic receipt ledger must preserve uncertainty.
3. A completion flag attached to a block cannot establish success before results arrive. An unexpected result must pause the remaining block. Binding must be resolved against pre-block evidence to prevent guessing future IDs.
4. A repeat blacklist can suppress legitimate current requests and annotation cases. Report repeats with their source/state context; do not silently drop actual SDK calls or ban the same tool globally.
5. Full descriptive schemas can exceed the context/deadline on long histories. Measure actual tokens and latency; never truncate history, infer test answers, or move test transcription outside the timer.
6. Numeric/time computation requires a selected operation and trustworthy operands. A deterministic calculator cannot infer intent. Its interface and training examples need a separate ablation before promotion.
7. Training interface changes require new causal targets and native export validation. Existing 2B capability/gradient tests do not certify a 4B checkpoint or this new interface.

## V2 architecture

Audio including supplied user history → ASR inside the sample clock → original request/context/history plus all allowed schemas → one 4B next-action actor → causal binding and schema/provenance checks → sequential calls to one original ToolEnv → typed outcome/evidence feedback → next action/ask/stop → original get_tool_calls/get_state_hash receipt.

The actor retains responsibility for intent, conditions, corrections, ambiguity, optional fields and goal coverage. The runtime does not use gold transcripts, labels, hidden state or final hashes to guide inference. It only uses validate/call/get_tool_calls/get_state_hash through the existing bridge. No hosted LLM or additional TTS in this path.

### R1 implementation boundary

Opt-in `--llm-mode actor-v2-experimental` in the Rust contest adapter:

- Full installed tool metadata retains descriptions, required/optional fields, x-ref, x-one-of, formats and returns hints. Common example stripping remains in place.
- A parameter may copy an exact JSON value via `{"$source":"/observations/0/result/..."}`. Allowed roots are context, history and past observations. No type coercion, string repair, hidden-state access or future-result binding. SDK validates the resolved arguments. Literal arguments remain supported.
- Explicit collection fields matching public x-ref namespaces establish typed ID observations. Conflicting known namespaces are rejected; coverage is intentionally partial. Unknown types still use the original generic provenance boundary. This does not certify freshness, subtype or user authorization.
- Unknown argument keys are returned for correction rather than silently removed in this profile. Clarification target/field/uniqueness checks remain active at runtime.
- Every actual or refused proposal has outcome feedback: completed, reported_success, observed, incomplete, failed, awaiting_user or unknown. `done=true` establishes reported tool completion, not completion of all user goals. The later success-envelope fix classifies `ok=true` writes without `done` as reported_success; neither status establishes semantic goal coverage.
- Unexpected/incomplete outcomes pause the rest of the block. All actual calls stay in the SDK receipt. A later empty finish may represent an already-satisfied or inapplicable goal; runtime does not invent semantic proof either way.
- Repeat counts are feedback, not a global blacklist. No hard six-call limit inferred from training data.

This is an unadapted 4B runtime candidate. SFT, comprehensive tool-specific freshness/outcome contracts, deterministic date/arithmetic operations, semantic goal ledger and ASR alternatives remain experiments with individual tests and task-score gates. The architecture assigns these responsibilities but does not claim they are already implemented or solve every failure.

## Frozen experiment sequence

1. R1 smoke/diagnostic pair: first 12 IDs in the existing ordered 123 engineering-validation cohort, chosen by position before V2 inference. Same executable, Qwen3-4B-Instruct-2507 Q3_K_M, Whisper turbo Q5, actual latest/history audio, context 12,288, CPU output, deterministic decoding, 30 seconds/sample. Control compact-typed vs actor-v2-experimental. No score-based sample replacement. Small diagnostic results never establish >90%.
2. If runtime/admission completes, inspect paired failures and actual calls/state. Freeze corrections as a new revision, preserve failed runs. A candidate must improve mean score without increasing incorrect effects or failed samples before replacing the control. Comprehensive confirmation uses all 123 with the same frozen policy, then the untouched-for-tuning portion of the development protocol and official submission as allowed.
3. Establish a 4B capability baseline before fine-tuning. Train only internal-train causal trajectories; validation remains excluded. Add rare-tool/contrastive/recovery data one component at a time; teacher labels require validation. Do not append a gold suffix to a state changed by erroneous rollouts.
4. Compare ASR with the chosen actor held fixed; score critical slots and end-to-end task outcomes. Do not promote from CER alone.
5. Freeze the candidate before larger evaluation, offline container/A30 validation and all 634 public outputs. Report mean state-gated call score, nonempty-perfect, zero-call accuracy, asks, namespace mistakes, timeouts and latency tails. Official and local results remain separately named.

Hardware on lattice is a development constraint, not the contest's A30 requirement. Research inference retains MemoryMax 8 GiB, swap 0, host free ≥4 GiB and GPU free ≥256 MiB sampled guards. Desktop's 2 GB + API contract is separate. Source/model/data/SDK pins and process ownership/cleanup remain required.

## Source and evidence

- [User-supplied review, repository copy](lifemate-4b-critical-review-2026-11-03.md)
- [Prior handoff](../development/nora-handoff-2026-11-03.md)
- [V2 Rust actor boundary](../../crates/nora-lifemate/src/actor_v2.rs)
- [Runner integration](../../crates/nora-lifemate/src/runner.rs)
- [Controller integration tests](../../crates/nora-lifemate/tests/workflow.rs)
- Private run protocol, source pins, input hashes and reports: `.artifacts/lifemate-v2-r1/` and lattice `.tools/lifemate-v2-r1/`.

Do not publish dataset, traces, labels, weights or keys. No commit/push in this iteration unless the user's publication gate is met or explicitly changed.

## R1 measured result and R2 revision

The all-GPU/context12,288 admission stopped before samples at 94,371,840 bytes GPU free. The preserved fit2 pair used28 GPU layers and the same context: both arms scored **0%/12** under the unchanged local metric. Control had10 failed/timeout samples and4 actual SDK observations; V2 had12 failed samples and0 actual SDK observations. Original SDK replay matched calls/results/state. V2's supervisor additionally failed in a smaps/exiting-child telemetry race after all12 outputs were written; its original failed receipt remains intact and it is not eligible for promotion.

Successful control responses measured prompt processing around60–76 tokens/s with CPU layer offload. Full descriptive schemas in V2 did not produce a decision before the30s deadline. This is a measured failure of the tested profile, not evidence that all4B architectures fail.

R2 adds opt-in `actor-v2-progressive-experimental`: the complete tool index remains present and any tool can be inspected later; schemas are disclosed on demand using the existing internal catalog interface. Synthetic controller tests verify later-tool recovery, evidence binding and exclusion of inspections from SDK receipts. It does not add a neural router or permanent top-k filtering.

The frozen R2 pair uses context8192/allGPU layers/CPUoutput, with full V2 vs progressive V2 on the same12 inputs. This within-R2 pair isolates disclosure. Cross-R1/R2 differences also involve placement/context and cannot isolate architecture. Native tests/build completed; task results pending at this entry.

The original pinned HF4B shards have now been downloaded and SHA-verified; unlike the previous handoff, weights are no longer header-only. A separate raw-logit200-MCQ baseline is running on4B. No SFT adapter or4B capability score has been accepted yet. The optional cohort extension to the text diagnostic permits a same-ID reference-transcript comparison; those outputs are explicitly marked diagnostic-only and must never be submitted as voice results.


## R4/R5 outcome and training-contract corrections

The official SDK uses `App.noop` with `ok=true,done=false`, while successful
`create`, `patch`, `delete` and app writes normally omit `done`. R1/R2 therefore
incorrectly classified successful writes as unknown. R4 adds `reported_success`
for an executed catalog-kind write with `ok=true` and no `done`. This permits
continuing an independent block; it does not prove that the user's intended goal
was achieved. Explicit failures/no-ops still pause the block. The original
completed/observed/incomplete distinctions remain.

R5 fixes three missing sources of phone provenance: literal 1800/1900 hotlines,
phone digits in returned message text, and an explicit old/new suffix correction
of exactly one observed phone. Ambiguous candidates are not selected. Text
still cannot create a trusted entity ID or authorize actions. These changes are
based on internal-train errors, with independent synthetic regression cases.

The initial supervised runtime check matched3370/3377 prefixes and failed5/1082
tasks: four provenance refusals and one Python/Rust exponent-rendering mismatch.
After fixes and Rust user-message canonicalization, all3377 causal prefixes match
byte-for-byte; all1082 supervised tasks have exact gold calls/state,2455 calls,
zero runtime failures/refusals. This is GOLD REPLAY WITH NO MODEL, not100% accuracy.
The original failed data/replay artifacts remain preserved. Only internal-train
IDs are exported; validation and public labels are excluded from training.

The pinned4B tokenizer accepts all3377 complete training sequences without
truncation: min1556, median4334, max7855 tokens. Assistant-only loss includes the
actual EOS and excludes the chat-template newline after EOS. GPU training and
native checkpoint export have not been certified by these CPU checks.

## Current empirical limits

- HF4B capability baseline:129/200=64.5%, with evidence50/50, safety/completion48/50,
  quantity16/50 and time15/50. This is synthetic raw-logit MCQ, not LifeMate score.
- R2 reference text at30s: native-compact and fullV2 both0%/12, all12 failed in each.
- R2 real audio at30s: fullV2 and progressiveV2 both0%/12; failures12/11,
  actual SDK calls1/3, progressive inspections4. No candidate promoted.
- Lattice prefill near100–140 tokens/s and histories with ASR near20s leave too
  little time for these profiles. This does not measure A30 performance.
- R3/R4 are preserved build/preparation snapshots, not additional quality results.
- R5 reference text,180s: native-compact0%, fullV2 2.7778% on the same12 tasks;
  failures0/1, actual SDK calls35/44, nonempty-perfect0/0. Both score0% when
  the original30s deadline is applied. All79 actual call results/state replay
  through the unchanged SDK. The relaxed diagnostic is not voice accuracy,
  an official submission, or evidence of >90% performance.

Private evidence: `.artifacts/lifemate-v2-r2/`,
`.artifacts/lifemate-v2-4b-actor-data-r4/runtime-contract-r5.json`,
`.artifacts/lifemate-v2-r5/tokenizer-report.json` and
`.artifacts/lifemate-eval-harness-v1/runs/hf-4b-v2/summary.json`.
The >90% target remains unmet; SFT, arithmetic/time operations, ASR and A30/official
confirmation remain open. Do not replace the metric or infer success from replay.

## R5 error analysis and training feasibility, 04/11/2026

Reviewing all12 paired diagnostic traces finds semantic failures even with exact
reference transcripts and sufficient time to terminate. The actor selects SMS
for an OTT request, invents monetary amounts/addresses, ignores conditional
branches, chooses a person from an ambiguous lookup, adds optional arguments,
uses translated filters that return no records, and stops without completing
the request. Some messages claim a payment succeeded although no payment ran.
These are observed failures of the tested actors, not failures corrected merely
by a valid JSON schema, observed ID, `ok=true`, or `finished=true`.

The V2 runtime fixes false refusals and supplies causal evidence, but it does
not yet prove correct entity selection, conditions, arithmetic or goal coverage.
The next learned-policy step remains train-only workflow SFT, with stop/ask and
lookup dependencies included. Recovery targets must be generated from the actual
resulting state; never attach a gold suffix after an erroneous write. Native
function calling with complete field/return semantics is an additional untrained
control worth measuring; the compact native baseline discarded those semantics.
Any comparison must state the prompt, schema, quantization and timing differences.

The first4B rank8/all252-linear QLoRA capacity pilot failed before any optimizer
update: a recorded forward peak violated512MiB GPU headroom at the shortest1556
token training record. Seven full train sequences1556..7855 tokens had been
selected by length, not quality. Its failed receipt and cleanup evidence are
preserved. This is a hardware/backend failure, not an SFT-quality result.

Capacity V2 replaces the frozen GPU-staged vocabulary head with an ordinary CPU
FP16 `F.linear` and native autograd/device copies. It keeps the complete sequence,
decoder, trainable targets, loss mask and original resource guards. Before any
update, the actual4B synthetic check measured loss difference0.00012112,
global gradient relativeL2 0.0013493 and maximum per-tensor relativeL2 0.0022385
across504 tensors, within the predeclared mixed-precision tolerances. This is
numerical agreement on a34-token probe, not bitwise equivalence or task quality.
The first1556-token update completed in176.49s. The next3565-token record failed
during backward with CUDA OOM (134MiB requested); the supervisor also rejected
sampled GPU headroom. Exactly one optimizer update occurred, and no adapter was
accepted or exported. All owned processes were cleaned up. Full training,
saved-state resume, export and inference quality remain unverified.

Capacity V3 was prepared with256-token native MLP checkpoints, CPU input storage,
expandable allocator segments and a tighter80%-of-visible-memory allocator cap.
This is a backend experiment, not a quality intervention. CPU tests cover nested
checkpoints, a non-divisible token length and nonzero LoRA B weights; GPU257-token
numerical parity must pass before four train stress records (shortest, previous
failure, longest sequence and longest target). The original time/resource guards
remain. See [PyTorch checkpoint semantics](https://docs.pytorch.org/docs/2.8/checkpoint.html),
[allocator options](https://docs.pytorch.org/docs/2.8/notes/cuda.html) and
[allocator fraction](https://docs.pytorch.org/docs/2.8/generated/torch.cuda.memory.set_per_process_memory_fraction.html).

Private evidence: `.artifacts/lifemate-v2-r5/text-paired-report-audit-v2.json`,
`.artifacts/lifemate-4b-capacity-v1/terminal-evidence.zip` and the separately
versioned capacity V2 control files. No validation labels enter training.

## What the current architecture still does not solve

| Requirement | Evidence and remaining boundary |
|---|---|
| Original input/history retained; one authoritative ToolEnv | Implemented and SDK replay checked. Diagnostic reference text is strictly separated from voice runs. |
| Exact causal value copy and partial ID namespaces | Implemented and tested; observed membership does not establish correct entity selection or freshness. |
| Write success/no-op semantics | Successful writes without `done` now classified correctly; user-goal completion is still the actor's responsibility. |
| Conditions, amounts, times and exclusions | Actual semantic errors remain. Deterministic operations and a trained selection policy are not yet validated integrations. |
| Ambiguity and independent work before ask | Runtime validates clarification structure; actors still choose ambiguous entities or omit independent work. |
| Learned next-action workflow and recovery | Train-only causal records are parity-checked. No full4B SFT adapter or recovery-trained policy has passed evaluation. |
| Full tool availability under context/latency budget | Full and progressive profiles exist; current30s audio results remain0%. Complete native interface is being measured separately. |
| Safety of inferred intent | Schema/provenance are necessary boundaries, not proof of authorization or semantic correctness. Incorrect effects remain in receipts and scoring. |
| Competition result and production readiness | No A30 execution, official public score, >90% confirmation or production certification. |

Training feasibility, training quality, model export, ASR quality, latency and
original scorer accuracy are separate gates. Passing one cannot substitute for
the others. More descriptive prompts alone have not established a successful
candidate; architecture changes remain opt-in until measured on complete cohorts.

## Updated evidence, 04/11/2026: native control and training capacity

R6 completed the full native-function interface control on the same 12 reference-
text diagnostics: **6.25% at 180s; 0% under the original 30s deadline**. There were
40 actual calls, one failed sample and zero nonempty-perfect tasks. The failed
request contained 8,681 tokens against an 8,192-token server context. The other
traces still show incorrect channel selection, conditions and ambiguous entities.
R6 changes both prompt wording and schema detail relative to native-compact; it
does not isolate either effect. It uses the same R5 source and Q3_K_M model.
All 40 SDK observations and final states were replay-audited. This is neither
official accuracy nor a voice result, and no candidate is promoted.

Capacity experiments are a separate series from those quality runs:

| Pilot | Observed result; no full SFT or model-quality claim |
|---|---|
| V3 | Failed the 257-token numerical gate before optimizer; the original assertion did not record drift magnitude. |
| V4 | Balanced chunks passed CPU checks, but implicit causal masking selected direct GQA unavailable in the chosen SM75 efficient-SDPA kernel. Zero updates. |
| V5 | Selected the installed Hugging Face `repeat_kv` path. Its repeated-text probe had nearly zero loss/gradients and failed relative-gradient comparison; zero updates. It is not accepted as a useful parity control. |
| V6 | Replaced only the degenerate probe with 257 fixed synthetic token IDs. Meaningful loss 12.3087 and all 252 nonzero LoRA-B gradient tensors. Explicit/implicit attention agreed exactly; balanced MLP global gradient drift 0.002868. Two real updates completed at 1,556 and 3,565 tokens; 7,855-token forward hit the CUDA allocator limit. Cleanup verified. |
| V7 | Native reentrant checkpoints/pageable CPU storage passed exact loss/all 504-gradient comparison on the meaningful 257-token 4B probe. Two updates completed (207.65s/406.64s), then 7,855-token attention-forward allocation failed. Cleanup verified; no full SFT. |
| V8 | Running an explicit RMSNorm output-dtype experiment: native FP32 statistics/scales and FP32 LoRA parameters remain; outputs return to the incoming activation dtype. Tiny CPU AMP loss/all nonzero adapter-gradient checks passed. Actual 4B checks precede optimizer. |

V6 sampled cgroup memory reached about 8.05 GB (kernel peak 8.09 GB), with no CPU
OOM or swap. It failed a CUDA allocation despite 583 MiB physically free because
the allocator limit was reached. The measured 3,565-token update took 337.18s.
One successful short optimizer update is insufficient evidence for training on
all complete trajectories. Preserve all failures; do not truncate the longest
sample, weaken resource guards or report numerical parity as task accuracy.

V7 uses PyTorch's existing checkpoint and saved-tensor APIs, with identical MLP
chunk shapes in forward/recomputation and an explicit gradient-bearing embedding
output. A tiny CPU test includes nonzero LoRA A and B, followed by the same 4B
numerical tolerances before any optimizer step. See [checkpoint semantics](https://docs.pytorch.org/docs/2.8/checkpoint.html)
and [CPU saved tensors](https://docs.pytorch.org/docs/2.8/autograd.html#torch.autograd.graph.save_on_cpu).

Private R6 audit: `.artifacts/lifemate-v2-r6/text-single-report-audit-v2.json`.
V6 terminal archive SHA256:
`94fae70a9c52bcd3d6be94ea42b4ff2755157f13813846f4fc5ba7ede68517c1`.

## Rust preparation of complete supervised trajectories

`training_data.rs` and the `lifemate-pack-trajectories` example pack contiguous,
hash-pinned train-only prefixes into complete causal task trajectories. They
validate whitelist coverage, partition, order, token digests, exact previous
prefixes, non-overlapping assistant loss intervals, EOS and context bounds. The
CLI stages output until validation succeeds and refuses to overwrite artifacts.
No validation/public/outer labels or inference policy are involved.

The full corpus produces **1,082 trajectories, 3,377 assistant turns and 89,563
target tokens**. Total input processing decreases from 14,882,937 to 4,846,990
tokens (67.43%) by avoiding repeated earlier prefixes. Every earlier token and
target is retained; longest sequence is 7,855 tokens and the largest combined
target is 234 tokens. Rust output is byte-identical to the independent Python
packer (SHA256 `05cc3bc635ed6184cdbbdbc7004e6af05694ef654fa4d2412557cce6f2831159`).

For uniform task sampling, loss `(1082/3377) * sum(mean_token_CE_per_turn)` has
the same fixed-weight expected objective as uniform prefix sampling. It does
**not** reproduce the sequence of Adam updates. CPU tiny-model tests comparing
all independent prefixes with the complete trajectory passed (global gradient
relative difference 1.40e-7). The first test's overstrict tolerance failure is
retained separately; FP32 RMSNorm explains why a nominal `.double()` model did
not support a 1e-10 absolute tolerance.

Windows package verification after adding the Rust packer: 121 passed, 0 failed,
3 ignored; formatting and all-target Clippy with warnings denied passed. This
new source has not yet been rebuilt on Linux. The capacity pilots above still
use single-turn loss; **234-target complete-trajectory training, fresh checkpoint
resume, export, trained-policy evaluation and the >90% target remain open**.

## xLAM reference-text challenger R8 and a wire-layout hypothesis

The already-staged xLAM-2-3B Q4_K_M challenger completed the same 12 reference-text
tasks with a 16,384-token context and the original R5 executable: **6.25% at 180s,
0% under 30s**, one failure, 15 actual SDK calls and zero nonempty-perfect tasks.
All 15 observations and final states replay exactly. No quality promotion.

Source audit clarification: `xlam-native` exposes the **full set of tools with
compact parameter schemas**, the compact English policy and xLAM array-format
instructions. It is not the R6 full-description/Vietnamese-policy profile.
R8 changes backbone, quantization, context, policy, schema detail and output
format relative to R6. Its original experiment note used “full native” imprecisely;
the pinned source and runtime mode are authoritative. Archive SHA256:
`3d59b7baeda4eaa66c6d849c16c61a379c1fc1123d9b13b8cf5acf51f27168b4`.

Raw responses reveal a concrete failure mode: several tasks produce English
summaries of past actions, claim to be waiting for an old tool result, or stop
with an empty array instead of acting on the latest request. One response reaches
the output-token limit. This motivates a separate opt-in wire-layout trial:
preserve the entire context/history JSON as user data, then provide the exact
latest utterance as a separate user message. Keep model, schemas, policy, current
tool-call/result pairing and SDK authority unchanged. The Rust mode
`xlam-separate-request-experimental` is implemented but not yet measured.
It does not prove the narrative failures were caused by input layout.

The model card marks xLAM3B as a research release under CC-BY-NC-4.0; this
experiment does not clear commercial deployment or contest eligibility.
[Official xLAM model card](https://huggingface.co/Salesforce/xLAM-2-3b-fc-r).

## Binding supervision now has causal targets

Training-target audit found 2,455 one-call turns, 922 empty terminal turns,
145 represented tools (32 with fewer than five examples), **zero multi-call
blocks and zero `$source` bindings** in the original 3,377 targets. The existing
data therefore does not train every capability supported by the V2 runtime.

New Rust `training_bindings.rs` and `lifemate-bind-training-ids` create an
equivalent training representation for IDs with exactly one matching earlier
successful observation in the application/collection explicitly named by the
parameter's `x-ref`. Ambiguous, failed, incomplete, wrong-namespace and missing
sources stay literal. Non-reference fields are unchanged. Prior assistant and
proposal messages are updated causally; executed arguments/results are retained.

The resulting corpus changes **652 arguments in 602 target records**. All 3,377
prefixes match the real Rust runtime; all 1,082 task receipts (2,455 actual calls
and final state hashes) match the original supervised replay exactly, with zero
refusals. This is a **no-model training-data verification**, not task accuracy.
Output SHA256: `8540cba80689bde263528cc4d4c9b63775b8b206f3f3e6eebf485d54aa74e82c`.

Tokenizer/full-trajectory checks and a trained-policy comparison against
direct-call SFT remain pending. This does not add arithmetic, freshness
reasoning, recovery training or independent-action blocks. A single source does
not itself prove correct user intent. Windows package checks after these changes
and the wire-layout experiment: **127 passed, 0 failed, 3 ignored**, fmt and
all-target Clippy with warnings denied passed. Linux build of this source pending.

## Latest measured results and source provenance, 04/11/2026

R9 Linux build completed: 132 tests passed, 3 ignored; formatting, Clippy with
warnings denied and release build passed. Its xLAM separate-request candidate
scored **0% on all 12 reference-text tasks**, at both 180s and 30s. No task
failed at the transport/deadline level. All 14 actual SDK calls and final states
replayed exactly. Eight tasks immediately returned an empty array or refusal;
another proposed invalid smart-home arguments without executing a call. The
remaining tasks chose wrong effects or arguments. Reject promotion of this
layout; separating the latest request alone is not a semantic solution.
Archive SHA256: `9527fa8337b1c97669491bf84ecddd5d072c7510c98849d96a1e60eb5caa334d`.
Some early R9 tasks overlapped a bounded CPU tokenizer job and later CPU checks;
timings are not an isolated speed comparison. No timeout caused the zero score.

Binding targets passed exact tokenizer and Rust trajectory packing checks:
1,082 tasks, 3,377 turns, 94,768 target tokens, max sequence 7,881 and max combined
target 249. Processing falls from 14,904,814 prefix tokens to 4,857,400 complete
trajectory tokens without truncation. Corpus SHA256:
`dae2e6c554931ca020d5110c093a8782873a7d88f6c24fb146c428584a328d00`.
This remains verified supervision, not a trained policy.

V8 capacity failed after two updates. The 7,855-token forward now passes, but
backward requests 124 MiB beyond the .80 allocator admission. The error reports
673 MiB physically free; allocator limits and the physical 512 MiB floor are
distinct. The failed run and verified cleanup are retained, archive SHA256:
`b43f0b778fd67faa79fa3c62202ec3d3166563ec6acf9abdf0b8cbe0dc19887d`.

The new complete-trajectory candidate keeps CPU head logits/loss on CPU, tests
loss and all adapter gradients against the GPU-loss control, and explicitly
sets allocator admission to .83. Physical GPU 512 MiB, host 4 GiB, owned 8 GiB
and zero-swap guards remain unchanged. The diagnostic allows 3,600s parent /
3,700s kernel time; contest evaluation still uses 30s. It checks complete train
trajectories of 7,881 tokens/238 targets and 5,581 tokens/249 targets. V2 of that
pilot failed before model load due to Python import binding; V3 fixes that bug
and adds a direct worker CPU preflight. V3 is running, with no capacity, SFT,
fresh-resume, export or quality acceptance yet.

A separate runtime review found that globally observed ID strings lost their
specific source namespace when the same string occurred in two apps. Evidence
now also records namespaces at exact history/observation pointers and rejects
bindings with a known contradictory namespace. Failed/incomplete results do not
grant a namespace. Unknown source types retain previous handling; this is not
complete provenance coverage, freshness verification or proof of user intent.
Windows checks: 129 passed, 3 ignored; fmt/Clippy passed. All 3,377 supervised
prefixes and all 1,082 receipts still match the prior gold replay, with zero
refusals. This newest source fix is not yet in the frozen R9 Linux executable.

## R10 worked-example control

An additional bounded alternative is one train-only worked example retrieved
with BM25 (k1=1.2, b=0.75, lowercase alphanumeric words). The same 4B full V2 actor
runs with and without retrieval on the same 12 reference-text tasks; both arms
include the source-namespace fix. Model, schemas, context 8192, sampling, SDK and
180s diagnostic / 30s comparison are held fixed. No quality result yet.

The corpus derives only from the pinned 1,082 internal-train tasks. It contains
latest text, history, context excluding installed-app metadata, labelled decisions
and available causal observations. Audio paths are omitted; terminal observations
not present in the supervision are explicitly null, never fabricated. No final
state/hash or validation/public labels enter the corpus. Its 3,510,469 bytes are
pinned at `2d3792a7570981c03add3a4af0b9a7c5b645fc6ae264e58d30c501df9545fa10`.

The Rust index enforces exact train coverage and a disjoint evaluation cohort.
Only examples at most 4,000 UTF-8 bytes whose action tools are currently available
are eligible: 853 examples meet the size bound before per-task tool filtering.
This selection never removes an installed tool or truncates current task data.
One explicitly marked example is added as user data in the initial JSON; its
values are not current evidence. Retrieval metadata is logged separately from
SDK observations. Similarity does not establish intent, and example copying or
context overflow can still reduce score; the paired test retains such failures.

Windows tests after adding retrieval: 133 passed, 3 ignored; formatting and Clippy
passed. R10 Linux build passed 138 tests, 3 ignored, formatting, Clippy and release
build in a separate 2 GiB CPU scope. Source manifest
SHA256: `54dff77315a03be4e80ed52bcf7e7110e1c5cb979b5e237b676c4d55cd1348a4`.
The inference pair completed after verified capacity-worker cleanup. Two short
bounded CPU attention checks overlapped its control arm, and the R12 bounded CPU
build overlapped the treatment tail; timing is not a fully isolated speed
comparison. Model outputs, actual elapsed times and failures are retained.

Independent unchanged-metric scoring: **2.7778% control versus 39.7186% with one
train example at 180s**, on these 12 reference-text development tasks. Both are
**0% with the 30s deadline**. Nonempty perfect tasks improve from 0 to 3; failures
are 1 versus 2. All 44/49 actual SDK calls, observations and final states replay
exactly. A separate Python BM25 audit reproduces all 12 selected training IDs and
scores, confirms the disjoint whitelist, corpus pin and example eligibility.
This supports continuing the retrieval direction, not deployment or a >90 claim.

The remaining errors are substantive: skipping calendar lookup and guessing
destinations/times, choosing a phone bill instead of electricity, choosing an
insufficient account despite an explicit fallback, executing a false numeric
condition, and choosing one of ambiguous contacts. Extra successful reads also
reduce call F1. One task exceeds the model-turn budget; another reaches 8,233
tokens against the unchanged 8,192 context. The latter failure is retained;
context was not silently cropped. Retrieval alone does not solve these issues.

Private paired evidence: `.artifacts/lifemate-v2-r10/text-paired-report-audit-v2.json`
and `retrieval-selection-audit-v1.json`. Archive SHA256 control:
`d4db1fc0d0dac319e8622bca043e1d6e91c42a57b9ab5ed68f1124ba5fd8ae85`,
treatment: `288279f847e85e415ab280a5b84c5e198a97f2959bcb103ed0645906b6809336`.

## Complete-trajectory V3 stopped; V4 and a backbone challenger prepared

Complete binding-trajectory V3 stopped at 1,953.56s with **zero optimizer steps**.
The supervisor observed 510 MiB free GPU memory, below the unchanged 512 MiB
training floor. Host available memory was 10.72 GB, owned swap was zero and no
cgroup OOM occurred. The longest 7,881-token forward had passed; backward had
not completed. The worker's last report still says `running` because it was
terminated externally: the terminal supervisor receipt is authoritative.
Owned process-group cleanup was verified. Archive SHA256:
`1d1fc5c01dbbee7779155242359292995bad8e58aab8b087ecf4054ea103ef27`.

V4 is prepared with one attention change: execute each of eight KV-head groups
with its four query heads through native efficient SDPA. Every query/key token
is retained. No custom gradient or local/sliding-attention approximation is
introduced. CPU synthetic Q/K/V gradients agreed exactly; a random tiny Qwen3
with 28 nonzero LoRA gradient tensors agreed within 6.54e-7 relative error.
Actual 4B/SM75 loss and all 504 adapter gradients must pass the existing numerical
gates before any optimizer step. Long-sequence capacity is not yet established;
the physical guards, full sequences, MLP chunking and optimizer are unchanged.

R11 is prepared using the existing R10 executable and staged Qwen3.5-4B Q4_K_M.
It compares full V2 structured output with the existing native-thinking profile
on the same 12 reference-text tasks. This changes output interface, policy,
reasoning and sampling together; it is not an isolated thinking ablation.
Comparing its control with R10 also changes backbone and quantization together.
Both profiles retain all failures, the original scorer and exact SDK replay.
R11 is now running; there is no accepted quality result yet. GPU experiments run
serially. Its native-thinking mode retains the existing 128-token server
reasoning budget. The R12 bounded CPU build may overlap this diagnostic, so its
timings are not an isolated GPU speed comparison either.

A private checkpoint helper passed fresh-object CPU AdamW/RNG continuation,
including parameter/optimizer-state equality and rejection of mismatched or
corrupt artifacts. It has **not** passed actual 4B/bitsandbytes restart/resume,
native export or trained-policy evaluation and is not used by these capacity
pilots. No full SFT, >90% result, A30 certification or official score exists.

## R12 prefill batch experiment prepared

Completed R10 control logs attribute 550.208s to 53,217 prompt tokens versus
71.248s to decoding, with at least 958 MiB free GPU memory at sampled instants.
Those timings include the disclosed CPU-test overlap. This motivates a new
within-run comparison, not an inferred speedup.

Rust `ModelRuntime::with_prefill_batch` and an optional trailing diagnostic flag
make batch/microbatch explicit. Defaults remain 256/128; values must be powers
of two in 32..512 with microbatch no larger than batch. Windows checks pass:
135 tests, 3 ignored, formatting and Clippy with warnings denied. The new Linux
build is running; source manifest SHA256:
`d0ff38960970c082b094d86ddac0dcdc43de10461bf15eed2c1111049498e853`.

Prepared R12 compares 256/128 with 512/512 on the same full V2 Qwen3-4B Q3 profile,
without retrieval, using the same new executable and all 12 tasks. Both arms
must run without concurrent build/training workloads. Changing GEMM shapes can
change numerical logits and greedy decisions, so quality and exact SDK replay
are still required alongside time/resource measurements. Full context and all
failure/deadline rules remain; no hardware or quality acceptance yet.

## R11 audited result and current retrieval work (04/11/2026)

R11 has finished with both archives frozen before scoring. On the same 12
engineering-development reference transcripts, Qwen3.5-4B Q4_K_M full V2 scored
6.3973064%, while native-thinking scored 29.7222222% at the diagnostic 180s limit.
Both scored 0% at 30s. Each had one HTTP400 failure; control had no nonempty
perfect task, treatment had one. All 114 executed SDK observations and terminal
states reproduced exactly. Native-thinking is not an isolated reasoning test:
policy, interface and sampling differ too. R10's BM25 example result39.7186%
remains the strongest of these small reference diagnostics; no default promotion.

The new backbone still invented a ride destination instead of resolving the
calendar event and still called one ambiguous contact instead of asking. The
structured arm also performed unrelated SMS/calendar effects in a phone-call
task. These are semantic/workflow errors; JSON validity and typed provenance
alone do not fix them. A correct final state can still lose call-F1 from extra
reads or mismatched exact arguments. Detailed receipts and errors are in
`.artifacts/lifemate-v2-r11/text-paired-report-audit-v2.json` and the two audited
trace directories. Original scorer SHA remains
`efa87d2b3187abc1c1421356b7e230a41ea153b9ef52d4bc74465768453241e0`.

R12's Linux release build, formatting, Clippy and 140 tests passed (3 ignored).
The paired GPU run is active with no concurrent lattice build/training/indexing
workload. This result is pending; a larger prefill batch is not yet accepted.

The optional Rust `semantic-retrieval` feature now contains an E5 CPU host,
train-index builder and equal-weight BM25/cosine RRF(k=60), retaining the same
one-example size and installed-action eligibility. Its index must cover exactly
the pinned internal train IDs and original request hashes, with matching corpus,
model and tokenizer hashes. Evaluation must be disjoint. Every query is encoded
inside the sample clock; only the last exact live query is cached. The LLM request
timeout subtracts preprocessing time. An embedding input over512 tokens falls
back explicitly to BM25 without truncating the original task, history or schema.
The feature is opt-in and restricted to the experimental full V2 actor.

Pinned multilingual-e5-small assets and ONNX Runtime1.31.1 are staged privately
on lattice. A CPU Hugging Face reference encoded six original synthetic sentences
without CUDA; it has117,653,760 parameters. This is not Rust/ONNX parity or task
quality. Windows hybrid tests currently pass143/0/3 and Clippy; Linux host parity,
train-index build and a paired task evaluation remain mandatory before any claim.
The extra neural parameters count toward the contest budget. Full end-to-end
audio, A30 timing, official submissions and >90% acceptance remain outstanding.

## R12 terminal result; R13 frozen before inference

R12 has completed and all86 actual SDK calls replay exactly. Both batch arms
score2.7777778% at180s,0% at30s, with one failure each. The same12 initial prompts
have matching prompt/cache token counts; aggregate initial-prompt time fell by
2.997% (median per-prompt reduction3.221%). Total task wall times were626.874s
and609.373s, but later trajectories differ (44 versus42 actual calls), so this
total is not a fixed-work throughput comparison. Minimum sampled freeVRAM was
958MiB versus928MiB. Source and protocol were identical except declared batch
settings, and no build/train/reference workload overlapped either arm. Keep
the original256/128 default; larger batches did not fix the deadline or quality.
Evidence: `.artifacts/lifemate-v2-r12/timing-audit.json` and paired audited report.

R13 source is frozen at manifestSHA256
`73fe59f52d30bcc7b18edf1faf34d340ae4898cfe7b604f452a03d810bba204b`.
Windows feature checks143pass/3ignored and default checks136pass/3ignored;
formatting/featureClippy pass. Linux feature build is running in a2GiB/no-swap,
50%-of-one-CPU scope. R13 uses16384 context for both arms, addressing the previous
8192-token overflows without dropping task input. It compares one BM25 example
against one BM25+E5 RRF(k60) example; both retain batch256/128 and original guards.
The optional extra encoder is CPU-only. The synthetic Rust/HF parity gate was
fixed before execution (six inputs, identical token counts, maxabs<=1e-5,
relativeL2<=1e-4,cosine>=1-1e-6); a train-only index is generated only after it
passes. Independent query/ranking reconstruction and unchanged SDK scoring are
prepared. There is no R13 quality result yet and no >90%/production claim.

R13 build CPU quota was increased from50% of one logical CPU to200% after
R12 had fully terminated, to reduce build wait. Its2GiB/no-swap cap and nice15
remain. This changes build throughput only; no model/evaluation ran concurrently.
The separate receipt is `.artifacts/lifemate-v2-r13/build-cpu-quota-change.json`.

## R13 rejected; R14 presentation experiment and prefix research

The R13 Linux feature build passed148 tests (3ignored), formatting, Clippy and
release compilation. Actual Rust/ONNX E5 output matched the fixed HF reference
on six synthetic inputs: maxabs9.686e-8, worst relativeL2 6.091e-7. The Rust
index builder encoded all1082 train requests without truncation; its CPU scope
peaked at1,215,381,504bytes with zero swap, taking22.947s for parity plus indexing.
IndexSHA256:`50e83829f1e0c721b4f1b6e9895e1cdf18efb9dbb06c1baf71747048a216557b`.

That implementation correctness did not produce a better agent. Same12 tasks,
context16384 and diagnostic180s: BM25 scored39.7186147%, hybrid E5 scored31.2169312%.
Both scored0% at30s and each had one model-turn-budget failure on train_0163.
All98 actual SDK calls/state hashes replayed exactly. Independent HF query
encoding and Python ranking reproduced every selection and112 response receipts.
Four examples changed; two tasks regressed: train_0249 paid the wrong bill
(score0.9091->0), train_0257 added an unnecessary chat read (1->0.8889).
The other eight selections were identical. Keep BM25; E5 remains an optional
experimental implementation, not the recommended configuration. Increasing
context removed the prior context-overflow failure but did not improve mean score.

R14 changes only example presentation: the same BM25 worked example moves from
the end of the current initial JSON object to a separate data-only user turn
immediately before it. The original current payload and all subsequent journal
messages stay byte-identical. Corpus, retrieval/eligibility, model, fullV2 policy,
context16384, batch256/128, sampling, SDK and scoring remain the same. This tests
whether presentation confuses the current goal; that explanation is a hypothesis.
Windows145 tests/3ignored, formatting and Clippy pass. New Linux build is active
in2GiB/no-swap/200%-CPU/nice15; no R14 model or score yet. ManifestSHA256:
`c00ed43186176447a61e3a6a2fdd014b34314478644de96dcc4c8b4a37c3f708`.

Separate train-only metadata research found936 distinct installed-tool sets in
1082 tasks. A cache per exact installed set would therefore have limited reuse.
A static reference catalog is a possible alternative: all147 normalized tools
and the policy cost12,139 raw Qwen3 tokens; a signature notation costs9,608 and
an independent decoder restores every normalized ToolSpec exactly. These are
static token counts, excluding chat-template overhead. There is no shared-prefix
runtime, warmup/cache experiment or model-quality result. Any implementation must
keep actual installed permissions and original SDK schemas authoritative, retain
the whole task/history, verify full context budget, and count runtime preparation
inside the relevant clock. Earlier columnar prototype counts failed to mirror
the existing description-example stripping; use the corrected v3 artifacts.
Evidence: `.artifacts/tool-prefix-v1/` and the train-toolsets survey.

## R14 measured presentation change; remaining policy errors (04 November)

Linux R14 passed143 tests/3ignored, fmt, Clippy and release build. The same12
reference tasks, corpus/selected examples, model, context16384 and180s diagnostic
produced39.7186147% for the example field in the current request and51.0149110%
for the same example in a data turn before the current request. Both strict30s
scores remain0%. Nonempty-perfect tasks decreased3->2; failures decreased1->0.
All95 actual SDK calls/results and terminal hashes replayed exactly. Independent
BM25 reconstruction verified identical selections and110 metadata receipts;
original current payloads were byte-identical. Wire bytes were not captured;
the transformation itself is covered by source inspection and a Rust test.

Three tasks improved: train_0105 started looking up the appointment and computed
correct dependent times, but still fabricated a destination; train_0110 selected
the correct bill amount but used `mẹ` where the exact gold query is `Mẹ`;
train_0163 recovered to the savings account but first made an unnecessary failed
transfer. train_0257 regressed from1 to0 by choosing the wrong food item. A state
hash match does not certify free-text addresses: the SDK canonicalizer omits
some free-text values, while call matching still checks them. Presentation is a
promising development candidate, not a complete semantic fix or production promotion.

The R13 review also corrects an easy misdiagnosis: train_0249's extra call was a
failed wallet payment, not a balance read; the gold workflow already includes
`ewallet.get_balance`. Audit actual calls before designing a correction.

Evidence: `.artifacts/lifemate-v2-r14/text-paired-report-audit-v2.json`,
`presentation-audit.json`, and R13 `error-review-v1.json`. ScorerSHA256:
`e893da84116920e81a25dd16c2bd8e22c254a32c1c5076c66ddc915ddf5d949f`.
No changes to the original local metric or sample/failure coverage.

R15 is frozen but unrun: the pinned Qwen3-4B Q3_K_M/Q4_K_M artifacts from the same
upstream revision, both28GPU layers, same fullV2/BM25/current-field presentation,
context16384/batch256/128/180s. Its control is fixed independently of R14 results.
The diagnostic CLI now admits explicit validated GPU layer placement; Windows
146 tests/3ignored, fmt and Clippy passed. This does not make Q4 a BF16 reference.

A proposed training saved-tensor optimization was deprioritized before a long
GPU run: installed bitsandbytes0.50.1 `MatMul4Bit` already keeps quantized weights
in `ctx.tensors`, outside saved-tensor hooks. Retaining frozen parameter storage
through those hooks therefore cannot remove copies of these main weights.
Source inspection is recorded in `.artifacts/bnb-frozen-storage-inspection-v1.json`.
This is not a measured full-model memory saving. CPU head precision/timing and
head-group attention capacity still require their own numerical/resource gates.


## Training capacity V6 terminal failure and V7 preparation (04 November)

V6 passed actual4B numerical checks for native KV-head grouping (all504 gradients
exact), FP32 CPU head with FP16 boundaries (global gradient relative error
0.00132850), and the full-trajectory objective (0.00355914). The unchanged
7881-token trajectory completed forward, then the supervisor stopped backward
at494MiB GPU free, below512MiB. Host available was8,638,001,152bytes; no owned
swap or CPU OOM. Zero optimizer steps and no accepted adapter. The child report
remained running after SIGTERM; the terminal supervisor receipt records failure
and proves owned group cleanup. Peak cgroup memory reached8GiB with file-cache
reclamation; this is not evidence of CPU OOM.

Independent failure audit and immutable terminal archive are under
`.artifacts/lifemate-4b-trajectory-capacity-v6/`; archiveSHA256
`1c335fa7c39cbc29771da2071b0d26c543cc5d23fd343c5d461957b99d1d6853`.
The failure itself lacks CUDA allocated/reserved counters; the previous phase
has them, but cannot establish the live/cached split at termination.

V7 is prepared, not run. It retains exact complete trajectories/math/helper pins
and inherited V6 numerical gates, tightens native process allocator fraction
.83->.80, and records bounded allocator counters during backward. Physical
512MiB GPU/4GiB host/8GiB process/no-swap guards are unchanged. It does not add
a garbage-collection threshold: inspected PyTorch2.8 source excludes expandable
segments from that collector. Source/evidence:
[PyTorch allocator](https://github.com/pytorch/pytorch/blob/v2.8.0/c10/cuda/CUDACachingAllocator.cpp#L2786),
`.artifacts/pytorch-allocator-source-audit-v1.json`. Counter-helper lifecycle,
exclusive output, cap and error handling pass CPU fixture tests; no GPU result.

R15 Linux build and model-metadata audit passed; paired inference is currently
running. Tokenizer/template, architecture metadata and all tensor names/shapes
match between Q3/Q4; only metadata general.file_type differs, with180 tensor
storage-type differences. This does not predict score. Wait for both arms and
independent scorer/SDK/selection audits before reporting results.


A separate actual4B/bitsandbytes fresh-process resume probe is prepared at
`.artifacts/lifemate-4b-resume-v1/`, not executed. It uses the complete shortest
internal-train trajectory (1716tokens/32targets), two reference updates and one
resumed update after restoring step1. The CPU auditor requires exact equality
of all504 trainable tensors, every optimizer tensor/scalar/group, RNG bytes and
the second-step loss/gradient. Syntax validation and native optimizer source
inspection are complete; these are not actual4B continuation results. The older
CPU tiny-model AdamW test remains separately scoped. Long-context fit and full
SFT are separate gates, even if this short resume probe later passes.


## Public portal re-verification, 04 November

The public frontend's observed config API was read without account cookies or
credentials. The Rule document reports an update at2026-11-03T02:09:50Z. Limits
remain whole system<6B, LLM<5B, context<=16384, offline,30s/sample on A30/16-core
Xeon Gold6442Y/64GB. Exactly one ToolEnv/sample; only validate,call,get_tool_calls
and get_state_hash are permitted at inference. No agent reads/copies/parses any
file under the environment directory. The current thin bridge uses this limited
interface; independent train/replay audits are separate from inference.

The formula image was inspected: state-gated callF1 agrees with the existing
local formula. The prose still does not specify the fuzzy tokenizer implementation,
so this re-verification does not certify mirror equality on every fuzzy edge case.
The public model-type leaderboard currently shows a top SLM score96.51; it is
a provisional submitted score, not evidence that NORA or a verified Docker passes.
It supports treating >90 as a meaningful competition target.

[LifeMate portal](https://challenge.zalo.ai/portal/lifemate). Private snapshot,
source URLs, timestamps and hashes: `.artifacts/lifemate-public-rules-20261104/`.
No test labels or environments were accessed by this web read.


R15 control has terminated with12 outputs and8 failed/deadline samples; Q4 arm
is running. Do not compute a paired quality result until both are terminal.
V7 is queued to launch once after both R15 owners/supervisors have exited cleanly.
Defer collector full-model rehashes and any lattice build/CPU benchmarks until
V7 terminates; its current trajectory timing should not overlap other work.

R16 is prepared only: the diagnostic CLI adds explicit `--cpu-output`, six
focused CLI tests/fmt/Clippy pass on Windows. Candidate placement is allGPU
transformer layers/CPU output,context12288 for bothQ3/Q4; overflow fails instead
of truncating current data. No Linux build/admission/quality result yet.

R17 is prepared only: Qwen3.5-4B Q4 with the existing best BM25/before-request
layout, exact R14 code and deterministic fullV2,context16384. Its reference is
the immutable R14 treatment archive, not a concurrent rerun. R11 had no retrieval
and context8192; R17 is a new combined candidate, not a repeat of that profile.
No voice/A30/official/production claim follows from either preparation.

## Native adapter handoff and Unicode retrieval checks (04 November)

The current Rust runtime now accepts an explicit local LoRA alongside the base
GGUF. Existing no-adapter startup is unchanged. The diagnostic CLI requires
`--lora ADAPTER SHA256`, checks the local file, bounded size, GGUFv3 header and
streaming hash before model launch. Missing/directory/base-as-adapter inputs fail
before creating the model log. Native architecture/weight compatibility remains
an integration gate. Windows library102 and diagnostic7 tests passed, plus fmt
and warning-denied Clippy. This is not a Linux build, converted adapter or score.

Pinned llama.cpp source8642541b255bfb5dc39cd351a178be09013a55d6 provides native
LoRA conversion/loading. Explicit local config-only `--base` avoids requiring a
full merged FP16 checkpoint; this path still needs a real conversion test. An
independent stdlib auditor checks all504 Qwen3 rank8 all-linear F32 tensors,
16,515,072 parameters, exact shapes/bytes, finiteness, metadata and alpha/rank.
Its 28-tensor binary fixture and13 corruption/unsupported-recipe controls pass.
Neither byte parity nor a successful load proves NF4/GGUF behavioral equivalence
or task quality. Evidence: `.artifacts/lifemate-lora-native-source-audit-v1.json`
and `.artifacts/audit_lora_gguf_v1.py`. No actual exported adapter is certified.

BM25 tokenization previously split decomposed Vietnamese combining marks into
different words. It now applies NFC to retrieval terms, preserving accents and
the original request/history bytes. Six retrieval tests and Clippy pass; all1082
train requests already use NFC. This fixes canonical-equivalence handling, not a
measured accuracy regression. Frozen R15 and R17 sources remain unchanged.

R17 is staged on lattice, not launched. It reuses the exact R14 source/build and
compares the Qwen3.5-4B/BM25 candidate to immutable historical R14 treatment.
R15 followed by queued capacity V7 retains exclusive access to model compute.


## 04 November: R15 audited, trajectory capacity V7 terminal, R17 active
R15 matched28GPU/8CPU-layer Qwen3-4B pair, all12 development reference tasks:
Q3_K_M16.6666667%, Q4_K_M13.8888889%, each8failed/deadline; strict30s0%.
All52actualSDK calls/results/finalstates replayed, all47retrieval receipts checked,
same examples and byte-identical original payloads. This placement is rejected;
timeout censoring does not establish a causal quality ranking of Q3 versus Q4.
Evidence `.artifacts/lifemate-v2-r15/text-paired-report-audit-v2.json`.

Full-trajectory capacity V7 failed during complete7881-token backward with a
124MiB CUDAallocator request rejected under the .80process fraction. Zero
optimizer updates; free GPU minimum sampled668MiB, no ownedCPUOOM or swap,
cleanup verified. Counter snapshots preceded the final failed allocation;
no proof of fragmentation or of the exact failing operation. Independent audit
`.artifacts/lifemate-4b-trajectory-capacity-v7/terminal-audit.json`, archive
`add1b36ada5f767e9911846f623640b71f5ed9a65244def19e9fb37ab84eb10f`.

R17 is now running on lattice with exactR14 source/build, Qwen3.5-4B Q4 and
BM25/before-request. It has not been scored. No second concurrent model run.
V8 is prepared only: native RMSNorm sequence chunks256 and failure-only native
OOM-operator metadata. CPU/actual4B numerical gates must pass before unchanged
complete-trajectory capacity tests. This is a memory hypothesis, not a proven
fix; the target and fullSFT/export/resume/A30 evidence remain incomplete.


## 04 November: V8 capacity accepted; R17 rejected; trace repair
The complete4B V8 capacity pilot now passes both full train trajectories without
truncation:7881tokens/238targets and5581tokens/249targets, two optimizer updates,
finite adapter save, owned cleanup, no CPUOOM or swap. New native RMSNorm chunk256
execution gave exactly equal loss and all504 parameter gradients on the actual
257-token4B numerical control. Physical GPUfree minimum968884224bytes(924MiB),
no guard relaxation. The two updates took1104.98s and709.64s; wholepilot1906.38s.
This proves bounded capacity on these stress trajectories, not full-corpus SFT,
checkpoint continuation, export behavior or improved taskscore. Independent
archive50c7180230e975842c56eed17bebdb66a8d05ea8c11dbdecaa281aee94800393;
`.artifacts/lifemate-4b-trajectory-capacity-v8/terminal-audit.json`.

R17 Qwen3.5-4B+BM25/before-request completed12 reference tasks at9.68253968%,
strict30s0%,1failure,0nonemptyperfect,103actualcalls. Reject this profile against
historicalR14 51.014911%. One case repeated effects to32callcap. Initial audit
correctly failed: the old runner lost two outcomes from the last partially
executed block before returning the cap error. Original SDK receipt retained
both calls. V3 auditor preserves immutable outputs/failedscore0, proves the two
calls equal the final model-proposal prefix, replays all103calls/finalhashes;
101original observation results can be compared, two original results unavailable.
No assertion that all103original results were logged. All106retrieval receipts
and byte-identical current payloads checked. Evidence
`.artifacts/lifemate-v2-r17/text-paired-report-audit-v3.json`.

Runtime now flushes known executed-prefix outcomes before propagating a block
error. New Python-bridge integration test exercises28calls then a partial4/7block;
all32calls/results retained, remaining3unexecuted, samefailure/state. All28workflow
tests and warning-denyClippy/fmt pass on Windows. Linux build pendingR19.

R18 FunReason-MTQ3 is running on lattice; model fullSHA verified,4411424256stored
parameters. Uses existing R14 runtime/interface, not the author's long-thinking
BFCL profile. No score yet. Actual4B fresh-process resumeV2 is staged using the
acceptedV8 operations; no resume success yet. Formal100% goal is active again,
interim>90 remains unmet; no commit/push or official-submission claim.


## 04 November: R18 tool-specialist constrained profile rejected
FunReason-MT4.4B Q3 completed the same12 reference tasks at16.8650794%, strict30s0%,
1failure/0nonemptyperfect. All54SDK calls/results/finalstates replayed; all95paired
retrieval receipts and current-payload equality audited. Archive
3d09328b3ef28ab967e8dfa9d4b35d1b8eb36ac4fa7dd81d445feed9f1740b56.
This does not beat historicalR14 51.014911%. It tests our constrained JSON/no-thinking
interface, not the author's prompt-mode/long-thinking BFCL recipe; no broad claim
that the checkpoint or its original method is intrinsically inferior.

Actual4B fresh-process resumeV2 is running. R19 runtime source is staged but not
built (manifest4a910d77a96a14de4db6437e12b7c6be3e1410a59bebd17e8c99bbe30ba7fcf5).
Offline actualV8 adapter export is staged, not run. Conversion auditV2 accounts
for PEFT's documented-in-actual-config reduction from252 fullpaths to the exact
seven projection suffixes while still requiring all504tensor names/shapes/bytes;
two positive binary fixtures and15rejection cases pass. No native load yet.

Full1082train order is frozen seed42, SHA0397fa09e35eb2a0a17347e91fdfe66a3d308274a3e9d30de6b64ab5f1edfc0e:
3377causalturns/94768targets/4857400tokens. First64 has291318tokens; no training has
started on this order. Short/long capacity success is not fullpass acceptance.


## 2026-11-04: resume/export/native gates passed; first64 SFT running

The actual4B resumeV2 reference and fresh process produce identical final504
adapter tensors, all optimizer state and CPU/CUDA RNG, and second-update loss /
gradient norm. Independent resource/source audit also passes: minimumGPUfree
1004535808B, no owned swap/CPUOOM, both owned groups empty. Archive:
03b2d6ae6d873399cf46082d31e4632b48f46537b6e8257b1d57068e2e9b0b56.

ExportV1 failed before conversion because config.json was sought in a weights-
only receipt. The failure is preserved. ExportV2 reads the pinned small-config
manifest and produces a66095328B F32 adapter GGUF, SHA
9ba1e549fdfb8190bf3936929489dc588fd0060a85a43bb1c67004b82cdb2d37.
Independent audit requires all504 tensor names/shapes/bytes, rank8, alpha16 and
valid ranges. No adapter values were re-quantized or silently omitted.

R19 Linux source4a910d77...7fcf5 passes149tests/0failures/3ignored plusfmt,
warning-denyClippy and release build. Synthetic Rust harness links that exact
library and completes2requests on base plus2with the actual adapter. Both owned
servers exit cleanly; no swap/OOM. This establishes native transport/load, not
HF-NF4 versus GGUF numerical equivalence or LifeMate accuracy.

First ordered SFT segment0000-0064 is now running on lattice, session50241,
supervisor344285. It starts from the base, not from the two-update capacity
adapter. Same tested recipe, seed42/orderSHA0397fa09...fc0e, full trajectories,
no evaluation rows. Each accepted update persists optimizer/adapter/RNG and
next cursor atomically. The64tasks contain291318tokens. Future fullpass1082,
closed-loop learned score, broader actual audio/A30 and official results remain
pending. Keep the shared GPU lock and physical guards; do not run competing jobs.

Evidence: `.artifacts/lifemate-4b-resume-v2/resource-audit.json`,
`.artifacts/lora-export-v2/independent-gguf-audit.json`,
`.artifacts/native-lora-v1/independent-audit.json`,
`.artifacts/lifemate-v2-r19/build-receipt.json`, and
`.artifacts/lifemate-full-sft-v1/protocol.md`.

Best measured engineering result remains51.0149% on reused12reference tasks,
strict30s0%. No >90/100 or production claim, no commit/push.


## 2026-11-04: audit before scoring the learned adapter

First64 SFT is confirmed live (session50241 / supervisor344285); two full tasks
4044 and5781tokens have committed checkpoints. This is not a quality result.
The saved-adapter audit is prepared to compare every exported PEFT parameter
against committed step64 before GGUF conversion. Its namespace/shape mapping
was tested on all504 tensors in actual previous4B artifacts;5unsupported name
cases are rejected. Actual step64 tensor comparison/export waits for completion.

A separate deployment risk was identified from existing evidence: both R14
16K-context arms had minimum312MiB GPUfree, while the successful native adapter
smoke used2K context. Full16K/cacheon/long synthetic-prefill admission is prepared
and required before R20 scoring; it has not run. No guard weakening or history
truncation. The maximum logged R14 prompts were8233/8517tokens; those observations
are not a bound on future prompts or proof a smaller window is generally enough.
Evidence: `.artifacts/lifemate-v2-r20/resource-admission-risk.json`.

R20 retains a fresh same-profile/no-BM25 base-versus-trained-adapter comparison.
No evaluation executes concurrently with training. Best task score remains the
historical51.0149% engineering12 diagnostic; no official/A30 or >90 result.


## 2026-11-04: telemetry-cap failure, short-segment recovery and model alignment

The long 0000-0064 run failed after six committed updates because the allocator
sampler reached its inherited 8000-record cap (0.5-second interval). Extending
the parent timeout had not extended that instrumentation bound. The independent
audit retains the failure: six complete ordered tasks, 29515 sequence tokens,
621 targets, min GPU free 1040187392 B, min host available 7682084864 B, no owned
CPU OOM/swap, and clean process ownership. This is not a 64-step completion.

Recovery resumes the exact saved adapter/optimizer/RNG at checkpoint-0006,
manifest a76634c3540818149555461da7081fd884ddbf9cb900f21cf999770270a75524.
Phase 0006-0008 is running; it repeats the uncommitted seventh task. Use at most
two full ordered tasks per process and audit each terminal segment before the
next resume. Frozen worker, math, order, guards and telemetry rate/cap are
unchanged. Shorter scheduling avoids the observed long-segment exposure but is
not a guaranteed latency bound. The new chain auditor accepts the actual six-
step prefix, and 16 synthetic invalid-link fixtures are rejected. It must later
prove exactly steps 0..63 with correct parent checkpoints; no skipped/repeated
updates, relabeled failures or synthetic evidence in the quality score.

Public model-fragment comparison independently matches all 398 base tensor
layouts, all 145 normalization tensors (196096 values, exact after F32 widening),
151669 defined token IDs and all BPE merges. It does not compare quantized
projection/embedding values. HF and GGUF template strings differ, but Python
Jinja renders all 3377 existing TRAIN inference prefixes identically. The GGUF
template adds an empty think block when serializing a terminal assistant; the
actual training uses HF serialization, and inference prompts end in user, so
that difference does not establish an error in the tested actor histories.

A synthetic literal </think> in assistant JSON can be removed by the GGUF
template when reasoning_content is absent. Native rendering must be checked;
adding an empty field alone is not a certified fix (the inspected native
serializer omits empty reasoning_content). Do not infer dataset error causality.
The absent GGUF rope.dimension_count defaults to attention.key_length in the
inspected pinned llama.cpp source, yielding 128, so absence is not an observed
RoPE mismatch. EOS/padding metadata and native tokenizer/rendering behavior
remain separate checks; no HF-NF4/GGUF logit equivalence claim.

Evidence: `.artifacts/lifemate-full-sft-v1/audit-0000-0064.json`,
`recovery-after-telemetry-cap.md`, `chain-audit-through-0006/chain-audit.json`,
and `.artifacts/base-alignment-v1/independent-alignment-audit.json` plus
`chat-template-string-audit-v1.json`. The earlier single-report step64 export
helpers require chain-aware admission before use. Best engineering task score
remains 51.0149% on reused12/reference/180s; strict30s remains0%. No >90/100,
full1082 pass, official submission or commit/push.


## 2026-11-04: recovered chain through8 and A30 timing interpretation

Resumed0006-0008 completed both full tasks (3883/6335tokens), exact parent
checkpoint0006 admitted. Independent segment+chain audit accepts exactly8
ordered updates,39733sequence tokens/838targets. MinimumGPUfree1042284544B,
cgroup peak8GiB with file-cache reclaim, no ownedOOM/swap, clean shutdown.
The frozen trainer/math/guard policy did not change. First64 remains pending.

[Measured lattice latency and A30 assessment](lifemate-lattice-vs-a30-2026-11-04.md)
separates hardware timing from task quality. Current official public rules still
specify A30/64GB/30s. R14's12reference tasks take39.49–92.86s and99.98% of
walltime is in the model step. Hypothetical3x/4x speedups are sensitivity checks,
not A30 measurements or scores. Public JSONL upload does not execute a model on
A30. Keep functional-quality and target-hardware timing gates separate; do not
reject an architecture solely from1650 timeout or promote it from theoretical
throughput. R14 quality remains51.0149% even under180s.


## 2026-11-04: lattice memory probe and resumed training

Native16K capacity probes with the same-shaped F32 adapter failed the unchanged
256MiB GPU-free guard at 244/250/254MiB for batch/microbatch256/128,256/64,
and128/32 respectively. V3 completed only the two base requests; its supervisor
stopped before an adapter response. Evidence/owned cleanup audits passed; the
resource admission did not. No observed CPU OOM or owned swap; no A30 or task
quality conclusion follows. R20 has not been promoted or run.

Training segment0008-0010 restored the audited checkpoint0008 and is running on
lattice. This records launch/restore only, not completion or improved accuracy.
Evidence: `.artifacts/native-lora-context-v3/independent-failure-audit.json` and
`.artifacts/nora-current-state.json`.


## 2026-11-04: native capacity profile admitted, ordered chain through10

SFT0008-0010 completed both full tasks (3019/3310 tokens), preserving exact
checkpoint0008 restore. Independent chain audit accepts10 ordered updates,
46062 sequence tokens/943 targets. Segment minimum GPUfree1040MiB, no owned
swap/CPU OOM, cleanup verified. The sequential controller now advances from10
toward64 in at-most-two-task segments, re-auditing each linked chain before the
next resume. It stops on any training/transport/collection/audit failure, with
no automatic retry. Fault-injected admission and transport failures confirmed
that stop behavior; no remote work was executed by those tests.

The native context16384 capacity probe v4 passed all four synthetic requests
(30/6327 tokens per base/adapter arm), using the same R19 library, native server,
Q3 base, F32 capacity LoRA, allGPU, q8KV and batch128/micro32, with CUDA Graphs
explicitly disabled through a temporary Rust exec launcher. The supervisor
verified the actual native environment for both server PIDs. Minimum GPUfree
274MiB, peak cgroup688791552B, zero owned swap/CPU OOM and cleanup verified.
The 256MiB guard remains unchanged. V1/v2/v3 failures are retained.

Both R20 arms are now prepared with this measured profile; R20 has not run or
been frozen. The trained step64 adapter is still required. Its interface audit
also checks common parameters/assets and normalized commands, allowing only
the adapter and per-arm output directories to differ. The temporary launcher
is not a production default change. Synthetic capacity does not establish task
quality, complete audio-pipeline fit, HF/GGUF numerical parity or A30 timing.

Evidence: `.artifacts/lifemate-full-sft-v1/chain-audit-through-0010/chain-audit.json`,
`.artifacts/native-lora-context-v4/independent-audit.json`,
`.artifacts/lifemate-full-sft-v1/queue-0010-0064-v1/queue-state.json`, and
`.artifacts/queue-stop-fixtures-v1/result.json`. No new task score or commit/push.


## 2026-11-04: chain through12 and controller continuity

The original Windows queue controller PID4072 disappeared while the remote
0010-0012 segment completed successfully. Its stale receipt/logs are retained;
the cause of controller exit is not established. Collected the completed remote
segment and independently audited the linked chain through12:58031 sequence
tokens/1064targets, no missing/duplicated optimizer steps. Started the unchanged
queue script from checkpoint12 using an explicit hidden Windows process;
PID40524 and remote supervisor376113 were both verified live for0012-0014.
Do not rely on old exec handle16598 or re-run the completed segment.

Post64 admission/saved-adapter/export/byte-audit helpers are now staged on lattice
with exact hashes, but have not run. Actual64 completion remains required.
[Five R14 policy errors](../development/lifemate-r14-policy-errors-2026-11-04.md)
were checked against the pinned archive. One wrong destination overlaps the
retrieved example but is absent from current input; the observed live calendar
location was then ignored. This supports a contamination hypothesis, not causal
proof. Other errors include an unsatisfied condition, wrong bill selection,
quantity and ambiguous contacts. Keep the no-example R20 comparison; no labels,
training trajectory, scorer or inference defaults changed from this analysis.


## 2026-11-04: audited14 and sequential checkpoint-to-quality controller

The unchanged training queue completed0012-0014, independently verified the
whole chain through14 (66633sequence tokens/1175targets), and resumed0014-0016.
Checkpoint14 manifest59ed9765...61069 and state153d7fd0...3854f remain on lattice.
Chain audit: `.artifacts/lifemate-full-sft-v1/queue-0012-0064-v1/chain-through-0014/chain-audit.json`
SHA256bd2bfdd4d9d85c5a424d96a57b4820a91f745fb08759f0aea8271f65a3d03a10.

A separate hidden controller PID13912 now waits for training queue PID40524 to
finish its audited64 and exit. It then executes the existing CPU tensor binding,
F32 export and independent504-tensor byte audit before freezing/staging R20.
R20 remains sequential base-versus-adapter, same profile, original SDK replay,
unchanged scorer and matched-interface audit. It stops on any failure and never
retries, promotes quality, commits or starts new training. CPU postprocessing
uses2GiB/swap0/600s scopes; native inference keeps its existing resource guards.
No post64 compute has started; prepared future stages are not results.

Controller `.artifacts/run-sft64-r20-pipeline-v1.py`, plan
`.artifacts/sft64-r20-pipeline-plan-v1.json`, state
`.artifacts/sft64-r20-pipeline-run-v1/state.json`. Four prerequisite fault cases
were rejected with zero external commands; three bounded remote command forms
passed syntax checks. Evidence `.artifacts/pipeline-stop-fixtures-v1/result.json`.
Do not manually duplicate the post64 stages while this controller is waiting.
The eventual12-task transcript result cannot establish broad audio/A30/>90/100.


## 2026-11-04: audio CLI adapter path implemented

The audio CLI lacked adapter/prefill options although the text diagnostic had
them. Added paired hash-pinned LoRA and prefill controls, sharing asset admission
with the diagnostic. Windows149tests/0fail/3preexistingignored, clippy/fmt passed;
no real model/audio inference. Failed missing-libclang and incompatible bundled
Linux ABI builds remain recorded; the successful build generated Windows bindings
with pinned portable libclang18.1.1. New source still needs a separate Linux
snapshot/build after the active training/R20 sequence. FrozenR19/R20 unchanged.
See [audio CLI report](../development/lifemate-audio-adapter-cli-2026-11-04.md).
Training chain through16 is audited; checkpoint-to-R20 controller remains waiting.


## 2026-11-04: broad cohort admission without model execution

Verified original1545 ID coverage and exact split:1082actualSFT tasks,153internal
validation and310outer IDs, no overlap. Real Windows Rust CLI accepted the two
correct selections before a deliberate ASR-option failure; wrong partition/hash
controls stopped earlier. No weights, audio inference, SDK environments or gold
decoding in preparation. Validation153 includes all reserved30/reused123 and is
not blind; ID checks alone do not certify historical outer blindness.
[Broader evaluation scope](../development/lifemate-broad-evaluation-2026-11-04.md)
records unchanged30s/originalscorer/allfailures for actual audio, separate from
relaxed diagnostics. Evidence `.artifacts/lifemate-v2-broad-eval-v1/`. No broad
quality result or automatic outer run; source build and candidate freeze followR20.
