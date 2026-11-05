# LifeMate R14: lỗi quyết định còn lại sau khi cho đủ thời gian

Phân tích ngày 04/11/2026 từ archive R14 đã kiểm tra hash. Đây là đọc lại năm
trace của tập phát triển nội bộ đã được dùng nhiều vòng, không phải lần chạy
model mới, tập kiểm thử kín hoặc điểm mới. Kết quả R14 vẫn là 51,0149% trên
12 mẫu transcript với hạn chẩn đoán 180 giây; chưa có kết quả A30.

| Mẫu | Bằng chứng trong trace | Vấn đề cần đo sau SFT |
|---|---|---|
| `train_0084` | Đề xuất điểm đến “Công viên Thống Nhất” cùng block với bước tra lịch, trước khi có kết quả. Địa chỉ này không nằm trong input hiện tại nhưng có trong ví dụ được truy hồi `train_0276`. Sau đó lịch thật trả về địa chỉ trường học; lời đặt xe vẫn giữ địa chỉ công viên. | Dùng dữ liệu của ví dụ khác và thực hiện hành động phụ thuộc quá sớm. Sự trùng khớp nguồn hỗ trợ giả thuyết ảnh hưởng của ví dụ, chưa chứng minh quan hệ nhân quả. |
| `train_0182` | Người dùng yêu cầu chỉ tạo báo thức nếu giá mua SJC dưới 120 triệu. Tool trả 121 triệu; model vẫn tạo báo thức. | Nhận diện và thi hành điều kiện, chọn đúng phép so sánh và quyết định dừng. |
| `train_0203` | Yêu cầu thanh toán internet. Kết quả liệt kê cả hóa đơn TV và internet; model trả hóa đơn TV. Số dư ví đủ cho hóa đơn internet. | Chọn sai thực thể. Trace này không chứng minh lỗi do ví thiếu tiền hay tính toán số dư. |
| `train_0257` | Nhóm Gia đình có ba người được nêu tên; yêu cầu một phần mỗi người, không tính người dùng. Model đặt một phần. | Đọc số lượng từ dữ liệu và xử lý loại trừ. |
| `train_0269` | History rỗng; tìm “Lan” trả về hai người khác quan hệ. Model gọi người đầu tiên, không hỏi làm rõ. | Xử lý mơ hồ sau khi đã làm phần yêu cầu độc lập. Các khác biệt state/F1 khác có thể cùng tồn tại. |

Script kiểm tra bằng chứng đọc trực tiếp các trace trong archive, đối chiếu
corpus ví dụ và các quan sát SDK đã ghi. Nó không gọi model, không đọc file
environment, không thay scorer và không đưa các thẻ chẩn đoán này vào train.
Kết quả: `.artifacts/lifemate-v2-r14/error-mechanisms-v1.json`.

## Quyết định triển khai

Giữ phép so sánh R20 đã định trước: base và adapter học từ train dùng cùng
interface, không chèn ví dụ BM25 ở cả hai arm. Điều này cho phép đo thay đổi do
adapter; chưa thể dùng kết quả ấy để quy riêng mọi cải thiện cho việc bỏ BM25.
Nếu thử lại retrieval, đó phải là một arm riêng với checkpoint và các tham số
khác giữ cố định. Cần đo cả lỗi sử dụng giá trị của ví dụ trong tác vụ hiện tại.

Ưu tiên đánh giá khả năng chọn thực thể, giữ điều kiện, đọc đầy đủ observations
và hỏi khi mơ hồ. Runtime hiện có copy binding và kiểm namespace, nhưng các kiểm
tra đó không chứng minh thực thể được chọn đúng ý người dùng. Phép tính xác định
có thể hỗ trợ số lượng/ngày/so sánh khi policy chọn đúng nguồn và phép toán;
chỉ thêm bộ tính chưa giải quyết được các lỗi lựa chọn và nguồn dữ liệu ở trên.

Giữ nguyên chuỗi SFT đang chạy. Không thêm quy tắc cho năm ID, địa chỉ, ngưỡng
tiền hay tên liên hệ trong bảng. Sau bài đo adapter, cần đánh giá rộng hơn trên
holdout, audio thật và phần cứng đích trước khi kết luận đạt >90% hoặc 100%.
