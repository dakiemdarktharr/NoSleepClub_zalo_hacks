---
date: 2026-11-03
status: active
---
# NORA — Project Review 2026-11-03

Mục tiêu formal 100% và checkpoint 99% đều chưa đạt. Main chưa commit/push phần đang kiểm chứng. Desktop giữ 2 GB + LLM API; bản thi offline đang thử trên lattice GTX 1650, chưa đo A30 hoặc có public score NORA.

Đã rà cấu trúc 12 crates, desktop/API, LifeMate, quyền broker, goal/checkpoint, voice lifecycle và update policy. Inventory/hash không thay cho security audit từng dòng. Simple/complex router còn là module nhận structured facts, chưa thấy integration call site runtime.

Hai cấu hình v32 chạy đủ 123 validation ngoài bộ 30 cũ đều đạt local mirror **2,9810%**; failures 79/78, nonempty-perfect 2/2. SDK replay calls/results/state khớp ở cả hai. 117 initial inputs quan sát được giống bytes; 6 lỗi trước model không tính là bằng nhau. GPU output giảm median model time khoảng 5,4%, không qua gate 20%; không promote.

Codex đã đọc và ghi chẩn đoán đủ 123 private cards, rồi tạo sáu deterministic proxy graders. Có 45 mẫu lặp lời gọi đã thực thi, 45 mẫu có provenance refusals, 16 mẫu chưa có quyết định model. Bốn server-log context overflows vượt 6.144 tokens. Lỗi ASR, intent/condition, grounding, tham số, completion và budget đều cần xử lý; không coi đây là human labels hoặc semantic-judge certification. Capability drift baseline còn thiếu.

V13 có một optimizer update thực và same-base-process adapter save/reload trên full 945 tokens. V14 dừng guard sau ba updates. V15/V16 chuyển frozen tied table sang CPU và quản lý unused cache, giảm CUDA allocated 970 MiB. V16 có full 1.940-token logits/loss/toàn 372 gradients khớp resident control; qua sáu updates đến 3.790 tokens rồi dừng ở 5.177 tokens vì GPU free khoảng 486 MiB < 512 MiB. Không lưu adapter V16, chưa full-length capacity, full SFT, optimizer resume, native export hoặc quality gain. Independent CPU table hash, source pins, cleanup và failure archives đã kiểm; own swap/OOM quan sát bằng 0.

Cặp 30 trước đây cũng không promote: Q3 turbo 8,3333% vs Pho 3,3333%; clarification v30 8,3333% vs 5,0000%. Sửa decoder v31 qua hai native synthetic probes nhưng chưa có task score riêng. Không dùng test pass, gradient parity hay CER để thay điểm tác vụ.

Đang chuẩn bị/kiểm tra đối chiếu sáu runtime prefixes qua native GGUF và HF NF4 không grammar, để chẩn đoán model/backend; không phải accuracy benchmark. Các mẫu đã được chọn hậu kiểm, không dùng làm bằng chứng tổng quát hóa.

Chi tiết: [[NORA - Home]], `D:/jarvis/docs/development/project-review-2026-11-03.md`, `D:/jarvis/docs/development/lifemate-error-review-2026-11-03.md`, `D:/jarvis/docs/lifemate-2026.md`. GT171–174 ghi kết quả terminal mới; GT160–170 giữ lịch sử. Dataset, weights, traces, labels và credentials không public trong repo.


Cập nhật: sáu prefixes qua HF/NF4 đã hoàn tất và audit. Lỗi chọn tác vụ quan trọng vẫn có ở cả HF và GGUF; không thể quy mọi lỗi cho grammar/backend. Bộ hồi quy 200 MCQ tổng hợp đã đo bằng raw logits: 114/200 = 57%, gồm thời gian 24, số lượng 15, evidence 35, safety/completion 40 trên 50 câu mỗi nhóm. 199 unique inputs và một duplicate; không phải điểm LifeMate/MMLU/an toàn production. GT175–177 ghi kết quả và làm rõ 74 SDK calls `done=false` ở 14 mẫu: không coi lời gọi đã gửi là effect thành công. Hai baseline native123 và HF200 được giữ riêng; chưa fullSFT/capacity/resume/export.
