# ADR-0005: Runtime ứng dụng Rust cho client nhẹ

Date: 2026-10-31. Updated: 2026-11-01. Status: **Accepted for Rust foundation** theo yêu cầu triển khai Rust và commit của user. Phần Python resident workers trong đề xuất trước được thay bằng Rust; Windows-first, quyền tối thiểu và resource gates của ADR-0001/0004 vẫn giữ. Chưa chấp nhận production release hoặc chứng nhận hardware budget.

## Context

User hỏi liệu toàn dự án Rust có tối ưu phần cứng hơn cho ngân sách NORA 2 GB RAM, GPU 4 GB, disk 15 GB, sau đó yêu cầu rebuild các phần cần bằng Rust. Kiến trúc trước dùng Rust broker cùng Python goal/audio host. Ngày 01/11 đã có 5 Rust crates, native voice probes và một paired waveform check; chưa có app end-to-end hoặc matched whole-product A/B.

## Decision

Viết mã ứng dụng client bằng Rust: broker, goal state/effects, API clients/worker roles, event policy và audio host. Foundation đã triển khai core/audio/TTS/store/CLI; UI/broker/LLM/adapters còn ở bước sau. Dùng whisper.cpp và ONNX Runtime qua bindings. Không cần Python trong hướng runtime này; native C/C++ libraries vẫn là dependencies cần quản lý. [Implementation receipt](../rust-foundation.md).

Không viết lại inference framework chỉ để đạt “100% Rust”. VieNeu cần port host/preprocessing/sampling/KV/streaming theo reference và paired quality tests. UI Tauri vẫn có WebView; native UI là một arm benchmark riêng.

Giữ process isolation cho native audio/parsers, typed broker và data-egress checks cho mọi model request. Rust không thay authorization, prompt-injection defenses, runtime patches hoặc effect reconciliation.

## Alternatives and trade-offs

| Phương án | Lợi ích | Chi phí/rủi ro |
|---|---|---|
| Rust + Python host gọn | Giữ ecosystem/reference behavior, triển khai sớm | Python resident/dependency footprint cần đo |
| Rust app + native inference | Bỏ Python khỏi installer, control/lifecycle rõ | Port TTS/goal semantics, FFI/runtime compatibility và maintenance |
| Rust cả inference | Kiểm soát thêm engine/backend | Model/operator/quantization parity, GPU support, chi phí lớn; chưa chứng minh lợi ích |

Không có cơ sở cam kết giảm X% RAM hoặc tăng Y lần tốc độ. Model/state/workspace và WebView có thể chiếm phần lớn; cùng backend không tự giảm VRAM khi đổi host language.

## Gates before production freeze

Benchmark matched A/B/C/D trong [báo cáo Rust](../research/rust-client-feasibility.md), giữ feature/security parity; resource traces cả cold/warm/interruption/recovery; no material degradation ở critical-slot accuracy, TTS và verified task completion. Release phải qua licensing, packaging, compatibility, crash/revoke/unknown-effect gates.

Python research/train/export/reference tools được giữ ngoài bản cài end user. Nếu port thêm backend không tạo lợi ích đủ lớn so với maintenance cost, giữ native engine đã được kiểm chứng. Quyết định này chấp nhận hướng triển khai và phạm vi foundation, không tuyên bố Rust luôn nhanh hơn hoặc đạt budget chỉ vì đổi ngôn ngữ.
