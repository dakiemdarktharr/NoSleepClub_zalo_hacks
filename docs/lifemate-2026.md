# LifeMate 2026 và NORA

Kiểm tra nguồn ngày **31/10/2026, Asia/Saigon**. Đây là ghi chú chuẩn bị, chưa đăng ký thi hoặc gửi submission.

## Chọn track

**LifeMate — Agentic AI That Turns Your Voice Into Action** phù hợp với yêu cầu trợ lý giọng nói thực hiện task nhiều bước. DataMate tập trung truy vấn dữ liệu tabular; không phải ưu tiên chính của sản phẩm này. [Trang challenges](https://challenge.zalo.ai/#challenges)

## Hợp đồng đã xác minh ở portal

| Thành phần | Nội dung công bố |
|---|---|
| Input | Audio tiếng Việt, có thể xen tiếng Anh phổ biến |
| Context | Current time, timezone, location, user profile |
| Apps | App tools có JSON schemas; chỉ gọi tools được cung cấp |
| History | Những lượt trước trong cùng hội thoại, nếu có |
| Environment | Mô phỏng dữ liệu app, chỉ đọc/thay đổi qua tool calls |
| Output | Ordered `tool_calls` với name/arguments |
| Thiếu thông tin | `sys.ask_missing_info` nêu intended tool và missing parameters |
| Không app hỗ trợ | Danh sách `[]` |
| Kiểm chứng state | `state_hash` lấy từ `env.get_state_hash()` |

Các trường và tên trên lấy từ [LifeMate overview](https://challenge.zalo.ai/portal/lifemate). Không suy từ ví dụ overview ra toàn bộ tool catalog hoặc công thức điểm. Không hardcode expected tool calls theo ID public test, không tự tính hash thay environment.

## Adapter dự kiến, chưa triển khai

```text
audio + context + history + provided schemas
  → ASR/transcript uncertainty
  → structured intent + slot checks
  → choose tool/read state
  → validate arguments against provided schema
  → execute simulator tool
  → observe state / next dependent step
  → final tool_calls + env.get_state_hash()
```

Competition mode chỉ dùng tools/environment chính thức. Desktop mode cần grants/approval/OS isolation riêng. Không đem hành vi simulator như banking transfer sang tài khoản thật. Không thêm UI notification events vào output `tool_calls` nếu contract không cho phép.

Khi thiếu thông tin, giữ các effects hợp lệ đã thực hiện và hỏi đúng tham số còn thiếu theo devkit semantics. Không mặc định đoán người nhận, địa chỉ hoặc số tiền từ memory không được context xác nhận. Với unsupported request, không cài app hoặc bịa tool để ép hoàn tất.

## Lịch công bố

| Mốc GMT+7 | Nội dung |
|---|---|
| 26/10/2026 | Bắt đầu, công bố devkit/data/evaluator |
| **16/11/2026 23:59** | Nộp giải pháp: training/testing code và models |
| 20/11/2026 09:00 | Công bố private test |
| **23/11/2026 23:59** | Nộp kết quả private |
| 02/12/2026 | Công bố hai đội đầu mỗi track |
| 12/12/2026 | Công bố kết quả chung cuộc |

Nguồn: [timeline trang chính](https://challenge.zalo.ai/#timeline) và [PDF thể lệ mục 3](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf). BTC có thể thay đổi lịch; kiểm tra lại trước từng mốc. FAQ ghi khóa thay thành viên từ 17/11, sau mốc nộp giải pháp 16/11; cần theo dõi thông báo thay vì tự diễn giải hai mốc là giống nhau.

## Public repo và giấy phép

Đây là phần đọc tóm tắt các điều khoản ảnh hưởng dự án, không thay nguyên văn hoặc tư vấn pháp lý. [PDF tiếng Việt chính thức](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf) là nguồn cần đọc trước khi đăng ký/tải dữ liệu.

| Mục | Điều ảnh hưởng cách làm |
|---|---|
| 4.1 | Dữ liệu cuộc thi giới hạn cho mục đích phi thương mại/học thuật được nêu; có yêu cầu xử lý/xóa dữ liệu cá nhân sau cuộc thi |
| 4.3 | Không phân phối dataset/dữ liệu cá nhân ra bên ngoài theo giới hạn của thể lệ |
| 4.4 | External data phụ thuộc hạng mục cho phép và cần chứng minh quyền/đồng thuận |
| 4.5 | Open-source code phải có giấy phép OSI không hạn chế thương mại, trừ khi có cho phép bằng văn bản phù hợp |
| 5.1 | Tối đa 5 bài/ngày, chọn một bài cuối để chấm chính thức |
| 5.2 và 6.2 | Freeze solution trước private test; cần khả năng chạy lại để xác minh |
| 8.2 | Chia sẻ mã thi trong thời gian thi có điều kiện công khai trên diễn đàn chính thức, trừ ngoại lệ được cho phép rõ |

Repo public hiện chỉ gồm product research và policy reference không dựa trên dataset cuộc thi, chưa tạo submission. Không upload dataset, raw audio, private outputs, weights không rõ quyền hoặc credentials. MIT trong repo chỉ áp dụng cho mã/tài liệu gốc của repo.

**Trước khi thêm competition inference/training code:** kiểm tra quy định track, phân loại phần mã là mã thi, xử lý yêu cầu công khai trên diễn đàn chính thức hoặc lấy xác nhận phù hợp từ BTC. Public GitHub một mình không được coi là đã đáp ứng điều kiện đó. Không đăng diễn đàn hay liên hệ BTC tự động trong công việc này.

Quyền commercial của sản phẩm NORA phải đánh giá riêng: dữ liệu cuộc thi không tự trở thành training asset thương mại chỉ vì đội đã tham gia hoặc đoạt giải. Code license, weights license và data rights là ba kiểm tra khác nhau.

## Chưa được xác minh

- Devkit version, sample IDs, schema/tool catalog đầy đủ và exact evaluator metric.
- Allowed external models, hosted APIs, Internet access, hardware, runtime/memory limits và reproducibility requirements theo track.
- Quy định có cho phép gửi competition audio/context tới model cloud cụ thể không.
- Bản solution cần đóng gói thế nào và endpoint submission chính thức.

Không chọn hosted model mặc định cho competition path khi các điểm này chưa rõ. Người dùng cần tự hoàn tất bước đăng ký/điều khoản nếu portal yêu cầu; không chấp thuận thay trong phiên nghiên cứu này.

## Thứ tự thực hiện đến 16/11

1. Xác minh devkit/terms/access và tạo private workspace cho tài nguyên hạn chế.
2. Dựng baseline tool executor + evaluator replay trước tối ưu giọng nói.
3. So sánh ASR theo critical-slot accuracy và final-state task success.
4. Chạy ablation router, dependency handling, missing-info, unsupported requests và state hash.
5. Freeze dependency/model/config/seed, lưu provenance, hướng dẫn reproduce; dành thời gian cho kiểm tra submission trước deadline.

Không coi lịch thi 16 ngày từ 31/10 đến 16/11 là đủ để đồng thời hoàn thiện một desktop assistant production đa ứng dụng. Competition baseline và production milestones cần có acceptance gates riêng.
