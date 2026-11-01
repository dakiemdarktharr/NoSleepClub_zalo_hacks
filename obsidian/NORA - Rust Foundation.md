---
project: NORA
type: implementation-receipt
updated: 2026-11-01
status: foundation-not-production
tags: [nora, rust, implementation, benchmark]
---

# NORA — Rust foundation

Đã xây `nora-core`, `nora-audio`, `nora-vieneu`, `nora-store`, `nora-cli`. Windows MSVC release build, 84 tests, format và Clippy qua. Chưa có desktop/LLM/OS broker integration hoặc chứng nhận NORA đạt trần 2 GB RAM, GPU 4 GB, SSD 15 GB.

Rust quản lý buffers, model loading, sampling/KV/codec streaming, native G2P, router/update rules, budget/approval types, SQLite goal/effect recovery primitives và scoped note có write/readback thật. Không có Python interpreter trong Rust runtime; ORT/whisper.cpp/SQLite vẫn là native dependencies. Graphs/weights không bị viết lại hay quantize để tạo lợi thế hình thức.

## Số đo thực tế

Windows 11 x64, Ryzen AI 5 340 6c/12t, CPU ORT 1.31.1; một process, không GPU/children; OS cache không flush.

- VieNeu FP32 + G2P: peak WS 1.160.454.144 bytes (~1,16 GB decimal); peak commit 1.179.000.832 bytes. Load ~4,52 s; first chunk ~408 ms sau load; synthesis ~1,12 s cho 1,68 s audio, chưa phát loa.
- Silero thật: peak WS ~46,05 MB, 32 silence frames; không phải accuracy benchmark.
- One-phrase greedy waveform comparison với reference: 80.640 samples khớp length; max abs 0 trong lần cuối, lần trước ~1,79e-6. Dùng phonemes từ Rust, không kiểm G2P độc lập hoặc human quality.

Không coi component peak là toàn bộ assistant idle/active memory. Whisper ASR host đã build/test guards nhưng chưa benchmark model thật; 4 GB GPU và ASR/TTS residency vẫn là release gates.

## Security và bước sau

ZIP nâng 6 → 9.0.0 để xử lý 3 RustSec advisories. Cargo audit cuối: 0 vulnerability advisories theo DB snapshot, còn 1 warning unmaintained `paste 1.0.15` qua tokenizer. Không bao phủ mọi native binary/model vulnerability. Asset manifests giữ SHA-256/size/revision, không phân phối weights hoặc tự xác thực publisher. VieNeu port giữ Apache-2.0 NOTICE.

Tiếp theo: mic/PTT/ASR/LLM/action/update loop; supervisor/IPC/OS grants/secret store; Giữ Ý và external-effect reconciliation thật; UI/tray; full-process hardware/quality/security corpus; signed installer/update và operations gates. Native kernel không được preempt giữa call; interrupt audio khác hủy goal.

[Implementation/build guide](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/rust-foundation.md) · [Measurement receipt](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/benchmarks/2026-11-01-windows.json) · [[NORA - Home]] · [[NORA - Workflow and Repositories]].
