# LifeMate: chuẩn bị đánh giá ngoài nhóm 12 mẫu

Ngày 04/11/2026. Đã kiểm tra lại phân chia cố định bằng ID, đối chiếu với whitelist
thực của lượt SFT và chạy preflight của CLI Rust đã biên dịch:

| Tập | Số mẫu | Trùng tập huấn luyện | Vai trò |
|---|---:|---:|---|
| Huấn luyện | 1.082 | — | Whitelist của SFT đang chạy |
| Validation đầy đủ | 153 | 0 | Gồm 30 mẫu đầu và 123 mẫu đã dùng trong phát triển |
| Holdout ngoài | 310 | 0 | Xác nhận riêng cho candidate được đóng băng |

Tổng 1.545 ID khớp dữ liệu train chính thức. Nhóm 12 mẫu R20 thuộc validation.
Không lựa chọn mẫu theo điểm, độ dài, lỗi hoặc số tool calls. Không loại mẫu
bất thường hoặc thất bại khỏi mẫu số.

CLI chấp nhận ID của cả hai tập với đúng partition; từ chối chọn holdout bằng
partition development và từ chối hash sai. Các kiểm tra dừng trước khi nạp model,
không tạo submission/metrics và không chấm chất lượng. Script chỉ dùng ID và
checksum; không giải mã nhãn hoặc đọc thư mục môi trường ứng dụng.

153 mẫu validation không phải kiểm thử mù: chúng đã được sử dụng trong quá trình
phát triển. Việc đối chiếu ID cũng chưa chứng nhận toàn bộ lịch sử truy cập của
310 mẫu holdout. Nếu dùng kết quả holdout để sửa tiếp, các lượt sau phải công bố
rằng tập đó đã được tái sử dụng.

Lượt audio thực cần source Linux mới, checkpoint đã nghiệm thu và cấu hình được
đóng băng sau R20. Giữ timer 30 giây, scorer gốc, toàn bộ history và đúng một
ToolEnv/mẫu; phân biệt hoàn toàn với chẩn đoán transcript có thời gian rộng hơn.
Chưa có lượt chạy hoặc điểm mới trên hai tập này, chưa có benchmark A30 hay điểm
leaderboard. Không lấy 12 mẫu hoặc preflight làm bằng chứng đạt mục tiêu.

Evidence và protocol: `.artifacts/lifemate-v2-broad-eval-v1/`; script tái kiểm tra
ID: `.artifacts/prepare-broad-evaluation-v1.py`. Không chạy lại script vào cùng
thư mục để tránh ghi đè bằng chứng đã lưu.
