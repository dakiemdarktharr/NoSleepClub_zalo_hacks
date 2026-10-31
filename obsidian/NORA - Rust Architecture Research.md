---
project: NORA
type: architecture-research
updated: 2026-11-01
status: research-with-foundation-followup
tags: [nora, rust, hardware, architecture]
---

# NORA — Rust và ngân sách phần cứng

**Cập nhật 01/11:** hướng Rust đã được chấp nhận cho foundation, host/policy/state/audio đã port và có component measurements. Xem [[NORA - Rust Foundation]]. Nội dung nghiên cứu bên dưới là baseline ngày 31/10; chưa có whole-product A/B hoặc chứng nhận hardware budget. whisper.cpp/ONNX Runtime vẫn là native dependencies, Python không nằm trong hướng runtime mới.

## Facts đã đối chiếu

- whisper.cpp vốn C/C++; whisper-rs là Rust bindings. Đổi host không tự đổi model weights/VRAM.
- ort chủ yếu bọc ONNX Runtime. Shared allocator, threadpool và device I/O tuning là lựa chọn backend; Python cũng có các khả năng tương ứng.
- VieNeu CPU dùng ONNX; sea-g2p và tokenizers đã có lõi Rust. TTS host còn sampling/KV/multi-session/streaming cần port và so chất lượng.
- Tauri chạy UI trong WebView; frontend Rust/WASM không loại WebView2. Native UI phải benchmark riêng.
- Rust không có tracing GC nhưng vẫn có thể leak/giữ caches; FFI không được bảo đảm an toàn chỉ vì caller là Rust.

## Điều chưa biết

Chưa có số MB/% RAM tiết kiệm thật, chưa chứng minh local voice toàn stack đạt 2 GB, chưa có quality parity cho TTS port, chưa chốt UI/backend/CPU support matrix. Search snippet có thể cũ; file VieNeu Việt được đọc trực tiếp hiện ghi fp32 default như bản Anh. Pin source/model/precision trước khi triển khai.

## Các arm cần đo

A: Rust broker + Python host gọn → B: Rust orchestration → C: Rust audio host cùng native engines → D: native UI thay Tauri. Khóa models/precision/features/grants/concurrency; không để phương án mới thắng bằng cách bỏ recovery/security.

Đo unloaded/warm idle, startup, ASR/TTS overlap/barge-in, long goal/browser, crash/resume và update. Working set/private commit báo riêng toàn cây process; GPU dedicated/shared và disk/cache/staging đều tính. Không coi pagefile hay weights file size là bằng chứng đạt budget.

Tài liệu chuẩn: [Báo cáo Rust](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/rust-client-feasibility.md) · [ADR đề xuất](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0005-rust-client-proposal.md).

Liên kết: [[NORA - Home]] · [[NORA - Resource Budget]] · [[NORA - Novelty and Production Plan]].
