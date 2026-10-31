# ADR-0005: Đề xuất runtime ứng dụng Rust cho client nhẹ

Date: 2026-10-31. Status: **Proposed — nghiên cứu/khuyến nghị, chưa port và chưa thay thế ADR-0001/0004**.

## Context

User hỏi liệu toàn dự án Rust có tối ưu phần cứng hơn cho ngân sách NORA 2 GB RAM, GPU 4 GB, disk 15 GB. Kiến trúc trước dùng Rust broker cùng Python goal/audio host; hiện chỉ có Python policy reference, chưa có app end-to-end hoặc số đo so sánh.

## Proposed direction

Ưu tiên đánh giá mã ứng dụng client bằng Rust: broker, goal state/effects, API clients/worker roles, event policy và audio host. Dùng inference engines qua bindings như whisper.cpp và ONNX Runtime trước. Bản phân phối có thể không cần Python; native C/C++ libraries vẫn là dependencies cần quản lý.

Không viết lại inference framework chỉ để đạt “100% Rust”. VieNeu cần port host/preprocessing/sampling/KV/streaming theo reference và paired quality tests. UI Tauri vẫn có WebView; native UI là một arm benchmark riêng.

Giữ process isolation cho native audio/parsers, typed broker và data-egress checks cho mọi model request. Rust không thay authorization, prompt-injection defenses, runtime patches hoặc effect reconciliation.

## Alternatives and trade-offs

| Phương án | Lợi ích | Chi phí/rủi ro |
|---|---|---|
| Rust + Python host gọn | Giữ ecosystem/reference behavior, triển khai sớm | Python resident/dependency footprint cần đo |
| Rust app + native inference | Bỏ Python khỏi installer, control/lifecycle rõ | Port TTS/goal semantics, FFI/runtime compatibility và maintenance |
| Rust cả inference | Kiểm soát thêm engine/backend | Model/operator/quantization parity, GPU support, chi phí lớn; chưa chứng minh lợi ích |

Không có cơ sở cam kết giảm X% RAM hoặc tăng Y lần tốc độ. Model/state/workspace và WebView có thể chiếm phần lớn; cùng backend không tự giảm VRAM khi đổi host language.

## Acceptance before approval/freeze

Benchmark matched A/B/C/D trong [báo cáo Rust](../research/rust-client-feasibility.md), giữ feature/security parity; resource traces cả cold/warm/interruption/recovery; no material degradation ở critical-slot accuracy, TTS và verified task completion. Release phải qua licensing, packaging, compatibility, crash/revoke/unknown-effect gates.

Nếu Rust không tạo lợi ích đủ lớn so với development/maintenance cost, có thể giữ host Python gọn cho phần chưa đáng port. Python research/train/export tools không nhất thiết nằm trong bản cài end user. Quyết định cuối ghi bằng chứng và scope cụ thể, không dựa vào nhãn ngôn ngữ.
