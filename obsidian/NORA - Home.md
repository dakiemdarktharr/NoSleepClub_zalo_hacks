---
title: NORA - Home
created: 2026-10-31
updated: 2026-11-01
tags: [nora, nosleepclub, zalo-ai-challenge, research]
status: desktop-validation-candidate
---

# NORA

**Natural-language Orchestration, Reasoning & Action**. Đọc “Nô-ra”, gọi “Nora ơi”. Codename chưa kiểm tra nhãn hiệu hoặc hiệu quả wake word.

Một trợ lý giọng nói tiếng Việt chạy nền trên máy tính: nhận một yêu cầu, chọn task flow phù hợp, làm trong quyền được cấp, kiểm chứng kết quả và chỉ cập nhật khi có thay đổi đáng biết. Cảm hứng JARVIS/FRIDAY ở phong cách tương tác và sự chủ động có giới hạn.

**Trạng thái ngày 01/11/2026:** bản desktop Rust 11 crates, tray/PTT, LLM API `gpt-6-luna`/`xhigh`, broker quyền và AppContainer voice workers. 188 tests và Clippy đã qua; kiểm chứng API/giọng nói/tài nguyên mới ở [[NORA - Desktop Integration 2026-11-01]]. Đây là unsigned validation candidate, chưa phải chứng nhận production hoặc official LifeMate submission. [[NORA - Rust Foundation]] là mốc lịch sử 5 crates.

## Bản đồ ghi chú

- [[NORA - Desktop Integration 2026-11-01]] — bản chạy, API thật, đo tài nguyên và các release gates còn mở.
- [[NORA - Supply Chain 2026-11-01]] — partial SBOM, notice export và kiểm tra đóng gói không cần biên dịch.
- [[NORA - Workflow and Repositories]] — ba vòng workflow, upstream repos và phần NORA cần tự xây.
- [[NORA - Rust Foundation]] — mã đã triển khai, build/test, model manifests, phép đo và production gates.
- [[NORA - Rust Architecture Research]] — đánh giá Rust/native engines, UI và benchmark tài nguyên; mới là đề xuất.
- [[NORA - Novelty and Production Plan]] — phương án Giữ Ý, VNG problem fit, demo, scope và roadmap mới.
- [[NORA - Novelty Ground Truths]] — facts/decisions/hypotheses của vòng nghiên cứu novelty.
- [[NORA - Security Research]] — incidents PHP/HF/Korea và threat model 24 nhóm rủi ro.
- [[NORA - Resource Budget]] — target 2 GB RAM, GPU 4 GB, disk 15 GB; chưa benchmark.
- [[NORA - Ground Truths]] — facts có nguồn, observations, decisions và unknowns được phân loại.
- [[NORA - Decisions]] — kiến trúc đã chọn và lý do.
- [[NORA - Open Questions]] — các bước cần chứng minh trước khi triển khai.
- [[NORA - Research Report]] — bản báo cáo đầy đủ để đọc trong vault.
- [[NORA - Sources]] — bảng nguồn và hạn chế bằng chứng.

[Repo public NoSleepClub_zalo_hacks](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks) · [Kế hoạch novelty mới](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/2026-10-31-plan.md) · [Lộ trình production](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-delivery-roadmap.md)

## Hướng sản phẩm

Windows-first user-session tray app. Native egui UI → Rust permission broker/supervisor → Rust goal/audio hosts → typed app/file adapters; LLM/worker roles dùng Responses API. Cascade STT → agent → TTS; hotkey/push-to-talk, chưa có wake word. Runtime dùng Rust/SQLite, không cần Python resident worker, browser thường trú hoặc LangGraph dependency.

Một prompt tạo một goal có acceptance checks. Thiếu quyền thì nhánh cần quyền chờ; không mất yêu cầu gốc. Một orchestrator mặc định; workers chỉ khi nhánh độc lập có ích. Độ phức tạp task và rủi ro hành động là hai trục riêng.

## Quy tắc giao tiếp

Nói khi có milestone/finding/blocker/permission/completion đã xác minh. Im lặng với epoch, skill initialization, retries đã xử lý và heartbeat không đổi. Source counts chỉ tính tài liệu đã đọc và loại trùng. Không đọc raw reasoning hoặc lấy lời model làm proof of completion.

`Dừng nói` khác `tạm dừng goal` và `hủy goal`. Quiet mode và người dùng đang nói ảnh hưởng timing; status question lấy state mới nhất.

## Liên hệ cuộc thi

Track phù hợp: [LifeMate 2026](https://challenge.zalo.ai/portal/lifemate). Điểm mẫu kết hợp state hash với precision/recall của tool calls; matching call không xét thứ tự, nhưng output phải giữ thứ tự thực thi để replay. Public ranking chỉ tính cột SLM. Hạn giải pháp được công bố tại ngày kiểm tra: **16/11/2026 23:59 GMT+7**. Cần đọc lại lịch và devkit trước nộp.

Public repo chứa nghiên cứu và policy reference độc lập. Không đưa dataset/audio/private outputs lên repo. Mã thi trong thời gian cuộc thi có điều kiện chia sẻ riêng tại mục 8.2; GitHub public không tự đáp ứng điều kiện công khai trên diễn đàn chính thức. [Thể lệ PDF](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf)
