---
title: NORA - Resource Budget
updated: 2026-11-01
tags: [nora, constraints, hardware, decisions]
status: integrated-validation-recorded
---

# Ngân sách cho NORA trên laptop phổ thông

**User requirement:** laptop khoảng 15–25 triệu đồng; ngân sách riêng NORA **2 GB RAM, GPU 4 GB VRAM, SSD 15 GB**; LLM có thể gọi API. Bản Rust tích hợp đã có synthetic whole-app measurements; số liệu/phạm vi mới nhất ở [[NORA - Desktop Integration 2026-11-01]]. Phần dưới giữ kế hoạch ban đầu; không dùng nó thay kết quả kiểm chứng hoặc chứng nhận mọi laptop.

[[NORA - Home]] · [[NORA - Decisions]] · [[NORA - Open Questions]]

Hướng ưu tiên: UI/tray nhẹ, Rust permission broker, native/ONNX audio, LLM API, bounded state và concurrency. Thử Whisper large-v3-turbo Q8 qua whisper.cpp, Q5 nếu cần; VieNeu Turbo CPU ONNX. Không chọn model nhỏ hoặc VieNeu Nano làm default trước khi so sánh chất lượng.

Model files Q8 khoảng 874 MB và Q5 khoảng 574 MB theo [artifact list](https://huggingface.co/ggerganov/whisper.cpp/tree/main); **không phải RAM khi chạy**. [whisper.cpp](https://github.com/ggml-org/whisper.cpp) hỗ trợ quantization và Windows CPU/GPU. [VieNeu](https://github.com/pnnbao97/VieNeu-TTS) có CPU ONNX; INT8 cần VNNI. Zipformer Vietnamese 30M là ứng viên khác cần benchmark/license audit, không được coi tương đương Whisper chỉ từ self-reported WER.

Cần đo toàn cây process gồm UI/WebView, workers và browser do NORA spawn. App đã mở của người dùng có footprint riêng. Task training/browser/code execution nặng không thể mặc nhiên hoàn thành trong cùng ngân sách 2 GB; cần plan/budget riêng hoặc phương án nhẹ hơn.

Targets để kiểm nghiệm: idle khoảng ≤250 MB; peak toàn cây ≤2 GB; GPU mục tiêu khoảng ≤3 GB để còn headroom trên card 4 GB; runtime/model packs ≤10 GB, phần còn lại cho bounded cache/log/update. Chưa có phép đo đạt các target này. Không dùng swap hoặc loại child process khỏi số đo để tuyên bố đạt.

Nếu local voice không đạt giới hạn, có thể đề xuất speech API; phải có consent riêng cho audio/transcript, destination và chi phí. Không tự chuyển sang cloud. LLM API permission cũng không cho phép upload mọi file.

Kiểm chứng cần giữ critical-slot accuracy/task success, giọng Việt vùng miền, code-switching, TTS tên riêng/số/ngày, barge-in và cold-start latency. Bộ đo phải gồm startup, idle, hội thoại, goal dài, browser, low-memory và sleep/resume.

[Thiết kế đầy đủ](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/lightweight-client.md) · [ADR-0004](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0004-lightweight-client.md)
