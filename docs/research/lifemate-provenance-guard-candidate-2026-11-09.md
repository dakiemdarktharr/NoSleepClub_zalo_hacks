# LifeMate: NORA provenance guard candidate

- **Research snapshot:** 09/11/2026 (GMT+7)
- **Status:** leading recorded full-validation proxy result; **not promoted**.
- **Scope:** documentation of an experiment in the separate, local LifeMate research harness. This note does not claim integration into the Rust desktop runtime, a submission, or an official score.

## Summary

The provenance guard checks whether identifier-like values in a proposed state-changing tool call can be traced to information supplied for that sample. If a required identifier has no acceptable source, the guard withholds the WRITE before it reaches the simulator. It does not ask the model to justify its own guess, invent a replacement identifier, or change model weights or prompt text.

Among the paired full-validation experiments recorded on 09/11, the **provenance-only** arm had the highest reconstructed proxy score: **11.30/100**, compared with **10.39/100** for its paired control (**+0.91 points**). It also produced more matching final-state hashes and fewer unmatched writes. However, it matched fewer gold tool calls and gold WRITE calls, and both arms had one inference failure. The preregistered gate therefore failed. This is the highest measured aggregate proxy in the current record, not a clean overall win or evidence of promotion.

## Problem and hypothesis

A tool call can be valid JSON and match the tool schema while still naming the wrong account, contact, event, device, or other record. Schema validation checks shape and type; it does not establish where an identifier came from. An invented identifier can lead to an unintended state change or a wasted action.

The hypothesis is that checking identifier provenance immediately before a WRITE will suppress some unsupported state changes while allowing identifiers grounded in the current sample to pass. The guard should be deterministic, local, and independent of model self-critique.

## Design

The experimental harness builds a per-sample evidence ledger from:

- The current user request and its local ASR transcript.
- Context and conversation history belonging to that sample.
- Actual results returned by the sample's allowed tools, including prior READ results in history.

For each proposed WRITE, the harness uses the supplied tool schema to locate identifier fields, including fields named as IDs/codes and fields marked with a reference annotation. It checks the proposed values against the ledger. Where a schema declares a referenced app, an identifier from a READ result must come from that app; contextual evidence from the same sample can also support a value.

If every checked identifier has a source, the action continues through the normal tool allowlist, schema validation, and `ToolEnv` execution. If an identifier is unsupported, the candidate withholds the WRITE and stops that branch safely. The selected provenance-only experiment has no automatic recovery READ. Recovery is a separate feature and must be evaluated independently.

The guard is **not** an authorization system. A value appearing in context does not prove the user intended a particular action, that the value is current, or that the action is permitted. Existing permission and tool policies remain necessary. The guard also does not alter or validate the ASR transcript; an incorrectly recognized identifier can still pass if it appears in that transcript.

## Runtime workflow

1. Start a fresh environment for the sample and load only the sample-provided request, context, history, tool catalog, and current tool results.
2. Seed the local provenance ledger from those sources. Do not carry identifiers across samples or obtain them from outside services.
3. Let the planner propose the next tool action using the existing offline LifeMate workflow.
4. Apply existing parsing, tool allowlist, and schema checks.
5. For a WRITE, inspect schema-declared identifier fields against the ledger before execution.
6. Execute supported actions through the official sample environment and record the environment's returned result/state hash. Withhold unsupported writes without fabricating a value.
7. Report blocked actions and provenance counts as diagnostics; do not reinterpret a blocked call as a successful task.

READ actions and the existing missing-information path are not blocked by this WRITE-only check. The candidate does not change the sample data, gold labels, evaluator, model, ASR, or tool schemas.

## Evaluation workflow and evidence

The research loop used paired arms and fixed experiment gates rather than choosing thresholds after seeing results:

1. **M0 static/replay checks:** exercise known grounded and ungrounded identifier cases with synthetic fixtures, then replay gold tool traces through the sample environment to check for false blocks and state mismatches. A prior train replay audit recorded zero false blocks across 1,077 gold identifier-WRITEs; this is a static property check, not evidence of improved model quality.
2. **M1 train screen:** compare control and candidate on the same 30 audio-mode train samples. The first standalone M1 had zero proxy delta and reduced matched gold WRITEs from 4 to 3, so it did not advance. A later four-arm screen on a different panel separated control, review-only, provenance-only, and combined behavior. Its 15 no-call stress samples reused previously seen families; the other 15 WRITE samples were family-disjoint. Provenance-only scored **12.22 → 18.89/100** and reduced no-call rows with any WRITE from **10 to 7**, while preserving **3/3** matched gold WRITEs and **3/15** hashes on the positive group. The reused stress cases limit how strongly this result generalizes.
3. **M2 paired full validation:** compare the single-feature provenance arm with a same-configuration control on all 290 train-derived validation samples. No holdout, public-test, or private-test rows were used.
4. **Apply the registered gate:** require reliable completion, no loss of matched gold calls or state hashes, safe no-call behavior, and non-decreasing proxy. A high proxy alone does not pass the gate.

### Full-validation aggregate comparison

| Arm | Proxy /100 | State hashes matched | Gold calls matched | False ASK rows | No-call WRITE errors |
|---|---:|---:|---:|---:|---:|
| Paired control | 10.39 | 65 | 137 | 61 | 12 |
| Provenance-only | **11.30** | **72** | 107 | 48 | 9 |
| One-turn ASK reflection | 10.61 | 63 | 137 | 24 | 12 |
| ASK schema guard | 10.44 | 64 | 138 | 29 | 13 |

These are locally reconstructed, train-derived validation diagnostics from separate paired experiments on the same validation partition. They are not a four-arm single-run ranking, an untouched generalization set, or the official challenge evaluator. The provenance experiment's paired result was +0.91 points (7 wins, 280 ties, 3 losses). The ASK reflection result had a smaller proxy gain but sharply fewer false ASK rows; the schema guard slightly increased matched gold calls but also increased no-call WRITE errors. The choice of “best” depends on whether raw aggregate proxy or preservation of correct actions is prioritized.

## Why the provenance arm was not promoted

The candidate reduced unmatched predicted calls from **528 to 343** and unmatched predicted WRITEs from **339 to 206**. No-call rows with any action fell from **16 to 13**, no-call WRITE errors from **12 to 9**, and false ASK rows from **61 to 48**. State-hash matches rose from **65 to 72**.

At the same time, matched gold calls fell from **137 to 107**, and matched gold WRITEs fell from **59 to 52**. There was one planner/inference failure in each arm. The frozen M2 gate required no loss of matched gold calls/WRITEs and zero inference failures, so this candidate failed the gate and was not promoted. The recorded candidate p95/max sample latency was 12.23/14.65 seconds; latency did not offset the quality/reliability failures.

The validation partition had already been used in earlier finalist screens. These metrics are development evidence only, and repeated analysis of the same partition does not create independent confirmation.

## Risks and open questions

- **False blocks:** an identifier may be valid but absent from the captured sources or represented differently. The guard can then prevent a legitimate operation.
- **False trust:** presence alone does not establish recency, correct entity binding, user intent, or authorization. Cross-app and conflicting-source behavior need focused cases.
- **ASR errors:** a wrong identifier produced by local speech recognition can be treated as grounded if it appears in the transcript.
- **Schema coverage:** field-name heuristics and reference annotations may miss identifiers or mark ordinary fields as identifiers. Schema quality affects coverage.
- **Multi-step tasks:** blocking one WRITE may prevent a necessary later step. A bounded READ-and-recheck recovery could help, but a separate recovery experiment showed no-call leakage; it must not be stacked in without fresh paired evidence.
- **Proxy limits:** local reconstructed scores may differ from the official evaluator and do not establish leaderboard or private-test performance.

## Next research steps

1. Analyze false blocks and missed identifier fields using private aggregates and synthetic/redacted fixtures; keep sample-level records out of public documentation.
2. Add tests for same-app references, cross-app references, IDs in context/history, actual READ results, absent values, conflicting values, and ASR-corrupted values.
3. If changing recovery or reference matching, preregister each as a separate one-factor experiment with fixed panels and gates. Do not combine them with this result by assumption.
4. Require no inference failures and no reduction in matched gold actions before considering another full-validation comparison. Keep the existing evaluator, denominator, and split fixed.
5. Keep this experiment separate from the Rust desktop assistant until an independently scoped integration is designed and measured.

## Public-record boundary

This note contains the idea, workflow, and aggregate metrics only. It excludes source code, datasets, audio/transcripts, sample identifiers, application state, per-sample predictions, tool arguments, and model weights. The official [Zalo AI Challenge 2026 regulations](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf) include separate restrictions on sharing contest source code during the competition; this documentation note is not a substitute for those requirements.

Related public project context: [LifeMate track and current research notes](../lifemate-2026.md).
