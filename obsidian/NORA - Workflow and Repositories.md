---
project: NORA
type: workflow-and-dependency-plan
updated: 2026-10-31
status: proposed-not-integrated
tags: [nora, workflow, rust, repositories]
---

# NORA — Workflow và repositories

NORA có ba vòng phối hợp: nhận/trả giọng nói; thực thi goal có checkpoint; chọn updates từ trusted runtime events. Đây là phương án Rust đang đề xuất, chưa phải app đã tích hợp. Repo hiện vẫn chỉ có Python policy reference và tài liệu.

## Luồng chính

Setup mic/folder/apps/API/model manifests → PTT/capture → resampling/AEC theo profile → Silero VAD + endpointing → Whisper ASR → critical slots/context → router answer/action/goal → workers API logic khi đáng dùng → broker/grants/provenance/egress → typed adapter → readback/receipt → complete/replan/clarify → event policy → VieNeu TTS/card.

Mọi model request cũng cần disclosure policy. Thiếu quyền/thông tin chỉ chờ nhánh liên quan; workers không có quyền lớn hơn supervisor. Giữ Ý tiếp tục phần hợp lệ khi source conflict/injection nhưng không bỏ qua legitimate changes. Timeout sau effect cần reconciliation, không blind retry. Chỉ verified postconditions mới thành full completion.

## Các repo chính

Tauri/plugins; CPAL/Rubato; Silero; whisper.cpp + whisper-rs; ONNX Runtime/ort; VieNeu/sea-g2p/tokenizers; Tokio/reqwest/Serde; rusqlite; windows-rs. Dùng crates/bindings/assets đã pin, không ghép nguyên mọi repository. LLM provider/model chưa chốt.

**whisper-rs đã chuyển sang Codeberg:** [upstream hiện tại](https://codeberg.org/tazz4843/whisper-rs). GitHub cũ archived và có migration notice.

AEC (tonarino/webrtc-audio-processing), MCP Rust SDK, chromiumoxide, egui và Rig là ứng viên có gate, không phải tất cả đều cài mặc định. Wake-word model chưa chốt; PTT là profile đầu. CPAL chỉ audio I/O, không tự chống echo; browser vẫn chiếm RAM dù client CDP là Rust.

## NORA tự xây

Goal state/effects, grants/broker, Giữ Ý/provenance/field authority, targeted clarification, verification/reconciliation, notification policy, Rust audio/TTS host, adapters/OA gateway, LifeMate adapter và eval/release. Không framework nào tự cung cấp toàn bộ các semantics này.

Updates: milestone/finding/blocker/quyền/completion có evidence. Im lặng với epoch/skill init/heartbeat không đổi. Close UI/stop voice khác cancel goal. Sleep/offline lưu state; resume reconcile; máy tắt thì local NORA không chạy.

Tài liệu chuẩn và mọi link repo: [Workflow and repositories](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/workflow-and-repositories.md).

Liên kết: [[NORA - Home]] · [[NORA - Rust Architecture Research]] · [[NORA - Novelty and Production Plan]] · [[NORA - Resource Budget]].
