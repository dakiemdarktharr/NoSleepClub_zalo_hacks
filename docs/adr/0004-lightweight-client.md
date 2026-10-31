# ADR-0004: Ngân sách client nhẹ cho laptop phổ thông

Date: 2026-10-31. Status: **User constraints accepted; architecture changes proposed, not implemented**.

## Context

User chấp nhận LLM API và muốn NORA nhắm laptop khoảng 15–25 triệu đồng. Ngân sách riêng cho assistant: 2 GB RAM, hỗ trợ GPU 4 GB VRAM, SSD 15 GB. Chất lượng không được giảm đáng kể chỉ để đạt số bộ nhớ.

## Decision

Giữ desktop-first và broker kiểm soát quyền. Ưu tiên đánh giá native/ONNX audio inference, không yêu cầu PyTorch/CUDA cho cả voice pipeline. Thử large-v3-turbo quantized qua whisper.cpp trước khi giảm model size; VieNeu CPU ONNX là ứng viên cần đo. LLM dùng adapter API có bounded context. Agent là các vai trò/async tasks, không phải một process/model riêng cho mỗi vai trò.

API/tool-first; browser, code execution và workload nặng phải có task budget riêng khi vượt ngân sách client. Cloud speech chỉ là tùy chọn cần consent, không mặc định upload. Không yêu cầu GPU rời trong mọi profile; CPU-only khả năng/latency cần test.

## Consequences

2 GB peak cho toàn cây process là gate cần chứng minh. Kích thước weights không phải peak RAM. GUI/model cold start và shared GPU memory phải tính. Mọi thay đổi backend/quantization/model cần paired quality tests. Nếu không đạt thì báo đúng giới hạn, không tuyên bố production-ready hoặc âm thầm đổi scope.

Xem [thiết kế và targets chi tiết](../lightweight-client.md). ADR này bổ sung ràng buộc tài nguyên cho ADR-0001, không chứng minh desktop/audio pipeline đã được xây.
