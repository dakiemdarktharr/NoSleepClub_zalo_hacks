# Validation receipt — 31/10/2026

Phạm vi: **research repository và offline reference policies**, không phải voice/app end-to-end acceptance.

## Đã kiểm tra

- Python 3.11: **21 tests passed** bằng `python -m unittest discover -s tests -v` với `PYTHONPATH=src`.
- Synthetic CLI replay: 9 events; technical/epoch/heartbeat im lặng; duplicate source IDs được loại; completion chưa verified bị chặn; permission xuất action card.
- CLI tiếng Việt chạy được khi redirected Windows stdout mặc định cp1252; CLI xuất UTF-8.
- Notification acknowledgements theo từng channel và goal; SQLite marker được đọc lại sau đóng/mở database. Test này không mô phỏng việc speaker đã phát rồi crash trước ack.
- Các file JSON parse được; không có local Markdown target bị thiếu trong bộ kiểm tra liên kết tương đối.
- Source ledger: **42 URL duy nhất**, 24 provenance records; trang shell/draft/historical/partial được đánh dấu.
- Hai nhánh nghiên cứu đã fact-check phần kiến trúc/voice/durability; không tìm thấy overclaim trọng yếu. Đây là review tài liệu, không phải independent security audit.

## Giới hạn

Chưa cài `jsonschema` trong môi trường validation; JSON schemas được kiểm tra cú pháp JSON và đọc thủ công, chưa chạy một Draft 2020-12 validator đầy đủ. Reference CLI dùng contract fixture đơn giản hơn production event schema; schema validation không xác thực producer hoặc cấp quyền.

Chưa có: microphone capture/STT/TTS benchmark, desktop installer, app connector, OS sandbox/IPC tests, LLM task routing evaluation, LangGraph/Temporal integration, actual crash/effect reconciliation, official LifeMate evaluator, production load/cost/security/privacy checks.

Tests thành công chỉ chứng minh các behavior fixtures hiện tại. Các SLO, cỡ mẫu và model lựa chọn là kế hoạch thử nghiệm. Xem [production gates](production-roadmap.md).
