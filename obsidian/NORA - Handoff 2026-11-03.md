---
project: NORA
date: 2026-11-03
status: paused
tags: [nora, lifemate, handoff, evaluation]
---

# NORA — khó khăn và bàn giao 03/11/2026

**Goal đang paused, chưa đạt 99%/100%.** Lượt lập báo cáo không chạy model/training, tải weights, nộp bài, commit hoặc push.

Đọc [báo cáo đầy đủ](<D:/jarvis/docs/development/nora-handoff-2026-11-03.md>) trước khi tiếp tục. Báo cáo phân biệt kết quả đã audit, quan sát sơ bộ và giả thuyết chưa chạy; các raw artifacts vẫn giữ riêng, không đưa vào Git public.

| Mốc | Ground truth và giới hạn |
|---|---|
| Latest task evaluation | Hai arm trên cùng 123 engineering-validation samples đều 2,9810% local task score; không phải official score; không promote |
| Error review | Đã đọc đủ 123 cards; actor có lỗi ý định/thực thể/điều kiện/dừng độc lập với ASR; annotation của agent, chưa là expert semantic labels |
| SDK correction | 74/673 calls của baseline A trả ok=true, done=false; không diễn giải attempts thành successful effects |
| Drift baseline | HF base NF4 114/200 = 57% trên MCQ tổng hợp; không phải LifeMate/MMLU hoặc chứng nhận safety |
| V17 training | Sáu updates; full 5.177-token forward xong; guard dừng backward tại 508 MiB free so với margin 512 MiB; chưa full terminal audit độc lập/save/quality |
| Next model candidate | Qwen3-4B-Instruct-2507 mới small assets/range headers; chưa full HF weights/load/benchmark; không tự suy tốt hơn |
| Official submission | Chưa có đủ public run/submission/official score hoặc A30-runtime test |

Bài học chính: schema đúng, SDK nhận, effect hoàn tất và tác vụ đúng là bốn tầng khác nhau. Phải đo audio → actual SDK calls/state với mọi timeout được giữ, rồi mới kết luận. Tối ưu Rust/VRAM không thay được năng lực hiểu ý định của model.

Khi được resume: đóng V17 audit trước; sàng lọc base mới bằng đúng drift suite và diagnostic prefixes; đo lại cùng 123 audio → SDK; chỉ sau quality gate mới đầu tư full SFT/resume/export. Desktop 2 GB + API, contest local A30/64 GB và lattice training là ba scope riêng.

Giữ main và gate hiện hành chưa commit/push trước 100% thực. Không reset/clean working tree, đụng lantern-vn, đọc .env.local, dùng paid cloud/API cho contest hoặc đóng ứng dụng người dùng.

Tài liệu liên quan: [error review](<D:/jarvis/docs/development/lifemate-error-review-2026-11-03.md>), [project review](<D:/jarvis/docs/development/project-review-2026-11-03.md>), [ground truths](<D:/jarvis/ground-truths.json>), [compact current state](<D:/jarvis/.artifacts/nora-current-state.json>).
