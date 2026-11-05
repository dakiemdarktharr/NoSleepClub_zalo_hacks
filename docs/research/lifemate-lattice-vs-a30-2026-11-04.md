# LifeMate: phân biệt giới hạn GTX 1650 và máy chấm A30

Kiểm tra ngày 04/11/2026. Có cơ sở để kỳ vọng NORA chạy nhanh hơn đáng kể trên
A30, nhưng chưa có benchmark NORA trên A30 để xác nhận hệ số hoặc hạn 30 giây.
Không dùng timeout trên GTX 1650 làm bằng chứng rằng kiến trúc sẽ timeout trên
máy chấm. Đồng thời, nhanh hơn không chứng minh chọn đúng tool hay tham số.

## Phần cứng và cách chấm

Đã đọc lại API công khai mà frontend LifeMate sử dụng, không dùng cookie/token.
Rule vẫn ghi 1 NVIDIA A30, CPU Xeon Gold 6442Y 16 core, RAM 64 GB; mỗi mẫu tối đa
30 giây, toàn bộ inference offline. Public Submissions nhận JSONL do đội tự
chạy sinh ra. Nộp JSONL không cấp một phiên chạy model trên A30; BTC kiểm chứng
Docker ở quy trình chấm nghiệm thu sau đó.

Nguồn: [LifeMate — rules và submission guidance](https://challenge.zalo.ai/portal/lifemate).
Hai response API hiện tại trùng hash với bản đã lưu trước đó. Receipt và bản
phân tích nằm trong `.artifacts/lattice-a30-feasibility-20261104/`.

Lattice hiện báo GTX 1650, 4096 MiB VRAM, compute capability 7.5. NVIDIA công bố
A30 có 24 GB HBM2, băng thông 933 GB/s và Tensor Cores thế hệ 3. GTX 1650 không
có Tensor Cores. Dung lượng VRAM A30 gấp sáu lần mức danh định 4 GB của lattice;
đây không phải hệ số tăng tốc. Chưa đo băng thông thực tế trên lattice và chưa
có benchmark NORA trên A30 để suy ra hệ số tốc độ.

Nguồn: [A30](https://www.nvidia.com/en-us/data-center/products/a30-gpu/),
[GTX 16-series Tensor Cores](https://www.nvidia.com/en-us/geforce/graphics-cards/compare/).

## Bằng chứng từ NORA

Đọc lại archive R14 treatment có hash đã kiểm chứng, không chạy lại model hoặc
đổi scorer: 12/12 mẫu transcript hoàn tất với hạn 180 giây. Wall time từ
39,49 đến 92,86 giây; median 67,23 giây. Khoảng 99,98% tổng thời gian nằm trong
bước gọi model, bao gồm xử lý request/retrieval/HTTP/generation, không phải chỉ
thời gian CUDA kernel. Mọi mẫu đều quá 30 giây trên lattice.

Nếu giữ nguyên output và overhead, chỉ giả định bước model nhanh gấp 3 thì
11/12 thời gian text nằm dưới 30 giây; gấp 4 thì 12/12, khoảng 9,88–23,22 giây.
Đây là phân tích điều kiện từ log, **không phải đo trên A30**. Nó không tính
ASR thực, không bảo đảm cùng output qua backend/precision khác, không thay thế
benchmark audio từ đầu đến cuối, và không phải điểm được nộp.

R14 vẫn chỉ đạt 51,0149% khi cho đủ 180 giây, 2/12 tác vụ không rỗng đúng hoàn
toàn. Vì vậy hạn thời gian và chất lượng đều cần xử lý; loại bỏ penalty timeout
không tự đưa kết quả đó lên >90%. Các số phút/update của SFT hiện tại gồm
forward, backward và checkpoint, không phải độ trễ suy luận của trợ lý.

## Điều chỉnh cách nghiệm thu

- Lattice: kiểm correctness, học policy và chẩn đoán lỗi; luôn công bố riêng
  điểm theo hạn 30 giây trên máy này và điểm chẩn đoán với thời gian rộng hơn.
- A30: nghiệm thu hiệu năng bằng audio thật, đầy đủ history, original SDK,
  context tối đa 16384 và timer 30 giây. Không cộng một hệ số giả định vào điểm.
- Giữ nhánh cấu hình inference dành cho A30 độc lập với tối ưu huấn luyện 4 GB.
  Docker prototype đã nhắm CUDA architecture 80 cho llama.cpp và whisper.cpp,
  nhưng còn dùng checkpoint cũ và chưa build/benchmark trên A30; chưa đủ để nộp.
- Có thể thử Q4/Q5 hoặc FP16 trên A30 sau khi có checkpoint học được. Đây là
  ứng viên cần đo lại tài nguyên, thời gian và chất lượng, không tự động thay
  bản Q3 đối chứng hoặc kết luận Q4 nhanh/chính xác hơn từ thông số phần cứng.

Chưa có quyền truy cập A30 của BTC. Giữ nguyên mục tiêu >90% trung gian và
real100% để commit/push; không hạ mục tiêu vì GTX 1650 chậm.
