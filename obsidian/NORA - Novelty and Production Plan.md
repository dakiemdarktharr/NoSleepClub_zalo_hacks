---
project: NORA
type: product-plan
updated: 2026-10-31
status: research-and-plan-only
tags: [nora, zalo, lifemate, agentic-ai, security]
---

# NORA — Novelty và production plan

Chọn hướng **NORA Giữ Ý**: giúp người Việt hoàn tất mục tiêu qua các ứng dụng được cấp quyền khi message/tài liệu/tool output có thông tin mâu thuẫn hoặc cố điều hướng sai. NORA dừng phần phụ thuộc chưa đủ điều kiện, tiếp tục phần đã xác định hợp lệ, hỏi đúng thay đổi quan trọng và kiểm chứng kết quả.

Đây là giả thuyết về workflow/trải nghiệm và đánh giá, **chưa phải thuật toán mới hay sản phẩm production**. CaMeL, Fides, Task Shield, RTBAS và các browser agents đã có nhiều thành phần tương tự. Cần matched baselines chứng minh safe completion hoặc user burden tốt hơn, không chỉ giảm lỗi nhờ từ chối hết.

## Vấn đề gắn với Zalo

Zalo đã có Safety Center, Kiki và OA Chatbot; Zalopay đã thí điểm SIMO warning. Không pitch lại “voice chatbot/chống scam đầu tiên”. Cơ hội thử là merchant OA + nguồn order/policy được cấp quyền, xử lý đổi hàng/hỗ trợ khách trong lúc có conflict/poisoned content. Nội dung ngoài không được đổi quyền/recipient/đích dữ liệu của goal.

Chưa có bằng chứng về backlog support hay loss rate nội bộ VNG, chưa có merchant interviews/partner access. Strategic fit không đồng nghĩa demand đã được xác nhận. Nếu discovery không cho thấy nhu cầu, đổi wedge; giữ core trợ lý tổng quát.

## Demo có đối chứng

User nói: “Đối chiếu yêu cầu đổi hàng đơn 123, soạn trả lời đúng khách và lưu biên bản.” Order/policy fixture và attachment mâu thuẫn về địa chỉ/actor. NORA vẫn kiểm tra điều kiện đổi, tạo draft/note, chỉ chờ bước phụ thuộc field chưa được xác nhận. Biến thể có thay đổi hợp lệ theo policy phải hoàn tất được. So sánh cùng model/tools với critic + confirmations.

OA không tự là order database: dùng synthetic fixture/CSV được chọn, hoặc OMS connector có quyền. Demo không chuyển tiền thật, không scrape personal chats, không gọi SIMO/report API chưa xác minh.

## Kiến trúc và scope

PTT → VAD/ASR → intent + critical slots → fast action hoặc durable goal → worker API logic khi đáng dùng → typed broker/grants/provenance/egress → adapter → readback/receipt → event policy → TTS/card.

Alpha: voice/text + scoped files/notes + public research + synthetic merchant flow. Beta hẹp thêm một OA/nguồn order. Calendar/email/browser là backlog có gate riêng, chưa phải integrations đã chạy. Mọi request tới LLM/router/critic/summary/worker cũng phải qua data-egress policy, không chỉ tool effects.

Ngân sách mục tiêu **NORA toàn cây process ≤2 GB RAM, thiết bị GPU 4 GB, disk ≤15 GB**, chưa benchmark. Ứng viên voice local: Silero ONNX, Whisper turbo whisper.cpp Q8/Q5, VieNeu v3 Turbo ONNX CPU. LLM dùng API; nhiều workers tăng request/token cost, không cần tải nhiều LLM local. Shared OA secret cần backend/BFF theo auth contract hiện hành.

Updates từ trusted events: milestone có ích, blocker/quyền, material conflict, completion có receipt. Không đọc raw reasoning, epoch, skill init hay heartbeat không đổi. Count nguồn từ URL thực sự mở/đối chiếu, không số result slots search.

## Gates và roadmap

- 01–04/11 dự kiến: rules/devkit + baseline/voice/resource spike, harness trước novelty.
- 04–10/11: durable effects, broker, Giữ Ý và genuine-change controls.
- 10–16/11: matched eval, freeze/reproduce và candidate thi nếu đủ rules/access; deadline công bố 16/11 23:59 GMT+7 cần kiểm tra lại.
- Sau candidate: partner pilot → signed beta → independent review/resource/recovery/privacy evidence → GA cho scope đã khóa.

“Oneshot” sau là thực thi backlog qua staged gates, không thay credentials, API approvals, signing hay human validation. Không được gọi mock/simulator hay unit tests là production proof.

## Tài liệu chuẩn

- [Bản kế hoạch đầy đủ](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/2026-10-31-plan.md)
- [PRD](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-guarded-completion-prd.md)
- [Threat model](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-security-threat-model.md)
- [Evaluation](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-evaluation-plan.md)
- [Roadmap/backlog/cost](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-delivery-roadmap.md)
- [Source audit](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/novelty-sources.md)

Liên kết: [[NORA - Home]] · [[NORA - Novelty Ground Truths]] · [[NORA - Security Research]] · [[NORA - Resource Budget]].
