---
title: NORA - Decisions
updated: 2026-11-01
tags: [nora, architecture, decisions]
status: proposed-implementation
---

# Những quyết định hiện tại

Đây là **design decisions**, không phải benchmark hoặc external ground truths. Xem [[NORA - Ground Truths]] để phân biệt nguồn và mức bằng chứng.

| Quyết định | Lý do | Điều cần chứng minh |
|---|---|---|
| NORA là codename | Dễ đọc/gọi, gắn orchestration và action | Trademark và wake-word FAR/FRR |
| Windows user-session tray app | Cần mic, UI và app local | Lifecycle, sleep/lock/resume, packaging |
| Rust application/control/audio + native engines | Bỏ Python resident runtime, kiểm soát lifecycle/buffers | Full quality/resource tests, native parser isolation; UI còn cần benchmark |
| Cascade trước speech-to-speech | Quan sát transcript và critical slots trước action | Useful-response latency và hội thoại |
| Hotkey/PTT trước custom wake word | Baseline nhanh, consent rõ | Model/data license thương mại, tiếng Việt |
| Một orchestrator mặc định | Giảm coordination/cost; đơn giản hóa recovery | Ablation tasks có nhánh độc lập |
| Rust/SQLite goal/effect primitives | Foundation đã có typed state/revisions/recovery | Supervisor ownership, WAL/backpressure, external effects và crash injection |
| Scoped broker trước mọi effect | Prompt/MCP roots không enforce quyền | Path/junction/TOCTOU, revocation, preview approvals |
| Event-gated selective updates | User muốn milestone hữu ích, ít log | Usefulness/omissions/duplicates/interruption |
| LifeMate adapter riêng desktop adapter | Simulator contract khác OS permissions | Devkit/evaluator và một app thật |
| Public research, contest assets tách quyền | Dataset và mã thi có điều kiện riêng | Track rules và forum-sharing gate |

Mọi runtime/provider version phải pin khi implementation bắt đầu. Chưa provision cloud, tích hợp payment hoặc chọn model thắng bake-off. Tauri capabilities không sandbox Python/Rust; supervisor không bảo đảm máy đang tắt vẫn làm việc.

Các ADR gốc: [ADR-0001](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0001-windows-local-first.md), [ADR-0002](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0002-evidence-gated-updates.md), [ADR-0003](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0003-public-research-and-contest-assets.md).
