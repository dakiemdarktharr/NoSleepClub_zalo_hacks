# LifeMate — hồ sơ track Zalo AI Challenge 2026

**Kiểm tra portal:** 02/11/2026, Asia/Ho_Chi_Minh (GMT+7). Mô tả dưới đây lấy từ tab Description, Evaluation, Rule và Submission Guidance trên [portal chính thức LifeMate](https://challenge.zalo.ai/portal/lifemate). Phần Data yêu cầu đăng nhập.

## Mục tiêu bài toán

LifeMate yêu cầu xây agent biến yêu cầu nói bằng tiếng Việt thành hành động qua các app/dịch vụ kết nối. Agent phải lập kế hoạch, chọn tool và arguments, thực hiện các bước cần thiết để đưa app về trạng thái người dùng muốn; nếu thiếu dữ liệu thì hỏi phần còn thiếu; nếu không có app/tool nào xử lý được thì không hành động. Bài được chấm theo hành động/trạng thái, không theo câu trả lời văn bản.

Portal đặt bài toán trong bối cảnh thiết bị ít hoặc không có màn hình như loa thông minh, xe, TV, tai nghe, đồng hồ và robot gia đình. Các miền dữ liệu ví dụ gồm smart home, banking, ride-hailing, food delivery và email. Yêu cầu giọng nói được mô tả là có giọng tổng hợp lẫn giọng thật từ miền Bắc, Trung và Nam; capability hướng tới là voice-driven, multi-step tool use trên nhiều loại thiết bị.

## Input công bố

| Thành phần | Mô tả |
|---|---|
| Yêu cầu mới nhất | File audio người dùng nói bằng tiếng Việt, có thể xen các từ tiếng Anh thông dụng. |
| User context | Thời gian hiện tại, timezone, location và hồ sơ người dùng, ví dụ địa chỉ nhà/nơi làm việc. |
| Apps/tools | Các app đã kết nối trong sample; mỗi app cung cấp tool kèm JSON schema. Ví dụ miền: smart home, banking, gọi xe, giao đồ ăn, email. |
| Conversation history | Các lượt trước trong cùng hội thoại, nếu có. |
| Simulated environment | Chứa app data như thiết bị, tài khoản, nhắc việc, danh bạ; chỉ được đọc và sửa qua tool calls. |

Ví dụ câu hỏi trong Description: về nhà thì đặt điều hòa phòng khách 26 độ và đặt xe từ vị trí công việc về địa chỉ `user_profile.home`. Đây là ví dụ giải thích, không phải danh sách tool đầy đủ hoặc quy tắc phải hardcode.

## Output và cách tương tác

Agent tạo danh sách **có thứ tự** các tool call, mỗi call có tool `name` và `arguments`, cùng `state_hash` do execution environment tạo qua `env.get_state_hash()`. Các trường hợp được nêu:

- Thực thi các tool cần thiết để đạt trạng thái cuối mong muốn.
- Thiếu tham số thì gọi `sys.ask_missing_info` để nêu tool dự định và tham số còn thiếu. Việc hỏi có thể xảy ra sau khi một số tool khác đã chạy.
- Không app nào xử lý được thì trả danh sách tool calls rỗng `[]`.

Portal minh họa mỗi dòng của `submission.jsonl` là một JSON object có `id`, `tool_calls` và `state_hash`:

```json
{"id":"s_000123","tool_calls":[{"name":"smart_home.get_status","arguments":{}},{"name":"smart_home.climate_set","arguments":{"device_id":"ac_living_room","mode":"cool","temperature":26}},{"name":"ride.book","arguments":{"from":"227 Nguyễn Văn Cừ, Quận 5, TP.HCM","to":"123 Trần Hưng Đạo, Quận 5, TP.HCM"}}],"state_hash":"9f2c4e7a1b8d3f60"}
```

Trong ví dụ, agent đọc trạng thái/danh sách thiết bị để tìm điều hòa phòng khách, cài chế độ cool 26 độ rồi đặt xe. Hash trên chỉ là giá trị minh họa của portal, không dùng làm hash cho sample khác. Nguồn cho các phần trên: [LifeMate Description](https://challenge.zalo.ai/portal/lifemate).

## Evaluation chính thức

Mỗi sample đánh giá cả trạng thái cuối lẫn mức khớp tool calls với gold.

### Khớp trạng thái

Với sample có gold tool-call list không rỗng, `state_hash` nộp phải bằng gold hash; nếu khác, điểm sample bằng 0. Fuzzy/free-text fields không tham gia hash. Hash phải được tạo bởi phiên bản phù hợp của `zaic_tools` bằng `env.get_state_hash()`. Ban Tổ chức có thể replay `tool_calls` theo thứ tự; nếu replay cho kết quả khác, kết quả của Ban Tổ chức là kết quả cuối. Nhánh gold list rỗng được xử lý trước theo công thức dưới đây và không xét hash.

### Khớp tool calls

Một call khớp gold call nếu tool name giống nhau, mọi non-fuzzy argument giống hệt nhau và mọi fuzzy argument có token-F1 **> 0,6** sau khi chuyển chữ thường và chuẩn hóa Unicode NFC. Mỗi call chỉ match tối đa một call; **thứ tự không được tính trong phần matching tool-call**. Tuy vậy, tool calls vẫn phải được nộp theo thứ tự thực thi vì environment replay theo thứ tự để xác minh trạng thái.

Gọi `M` là số call match, `U` là số call nộp không match (bao gồm call bị schema error), `G` là danh sách gold calls, `N` là số sample. Xét nhánh gold rỗng trước; chỉ tính precision/recall khi `G` không rỗng:

```text
Nếu |G| = 0: Score_i = 1 khi tool_calls nộp rỗng, ngược lại 0
Nếu |G| > 0 và (state_hash sai hoặc M = 0): Score_i = 0
Nếu |G| > 0, state_hash đúng và M > 0:
    P = M / (M + U)
    R = M / |G|
    Score_i = 2 × P × R / (P + R)
Final score = (100 / N) × Σ Score_i, với i = 1..N
```

Thứ tự nhánh trong công thức hiển thị: nếu gold list rỗng, sample được 1 điểm khi danh sách nộp cũng rỗng, nếu không thì 0; nhánh này không xét hash. Chỉ khi gold list không rỗng mới áp dụng điều kiện hash và công thức harmonic mean ở trên. Câu mô tả trạng thái trên portal nói rộng rằng hash sai thì điểm mẫu bằng 0, nên có khác biệt với nhánh rỗng được đặt trước trong công thức; prototype cục bộ theo công thức hiển thị. Evaluator/server của BTC vẫn là thẩm quyền cuối cùng. `sys.ask_missing_info` được chấm như tool call bình thường. Công thức và định nghĩa đã được đối chiếu trực tiếp với tab Evaluation trên [portal LifeMate](https://challenge.zalo.ai/portal/lifemate) ngày 02/11/2026.

### Nhóm model trên public leaderboard

Portal hiển thị các cột **SLM** (<6B tham số), **LLM** (≥6B) và **APIs/Others**. Ghi chú của leaderboard nói thứ hạng public chỉ dựa trên điểm SLM; điểm LLM và APIs/Others để tham khảo. Giải pháp hợp lệ vẫn phải tuân rule của track/cuộc thi; đây là cách chia cột xếp hạng, không phải các thành phần cộng thêm vào công thức score ở trên. Nội dung leaderboard có thể thay đổi; kiểm tra ngày 02/11/2026.

## Giới hạn và yêu cầu kỹ thuật công khai

### Model, dữ liệu và inference

- Tổng số tham số của toàn hệ thống (ASR, LLM và thành phần khác) phải **dưới 6B**; riêng LLM **dưới 5B**; context length tối đa **16.384 tokens**.
- Được dùng public data và open-source model phát hành trước **26/10/2026**, nếu license cho phép.
- Có thể tạo synthetic data bằng dịch vụ/model khác trong quá trình phát triển, nhưng **không được dùng các dịch vụ/model đó khi inference**.
- Không được gán nhãn public test/private test thủ công hoặc dùng dịch vụ/model ngoài để tạo nhãn nhằm train/tune.
- Runtime tối đa **30 giây mỗi sample**; timeout hoặc lỗi thực thi được tính 0.
- Inference phải chạy **hoàn toàn offline**, không network và không external API/service.

### Môi trường chấm, truy cập dữ liệu app và tái lập

- Phần cứng scoring được công bố: **1 NVIDIA A30 GPU**, **Intel Xeon Gold 6442Y 16-core**, **64 GB RAM**.
- Không đọc, sao chép hoặc parse file trong `<split>_environment/`, gồm `state.json`. Đọc/ghi app data qua `zaic_tools` do ban tổ chức cung cấp.
- Dùng chính xác một `ToolEnv` cho mỗi sample; mọi call thực hiện trên environment đó phải có trong submission.
- Cố định random seeds (Python, NumPy, PyTorch, v.v.) và dùng decoding xác định cho ASR/LLM, ví dụ greedy hoặc temperature 0.
- Nếu chạy lại không cho kết quả đã nộp, BTC chạy Docker image 5 lần và lấy điểm trung bình làm điểm chính thức.
- Nếu bằng final score, BTC chấm lại tool calls bằng F1, bỏ qua so sánh state hash; nếu vẫn bằng, đội có thời gian xử lý trung bình/sample thấp hơn được xếp trên.

Nguồn: tab Rule trên [portal LifeMate](https://challenge.zalo.ai/portal/lifemate). Các giới hạn này là nội dung công bố của trang; cần lưu bản devkit/evaluator hiện hành để khóa phiên bản thực thi.

## Quy trình nộp bài riêng của LifeMate

### Vòng public — 26/10 đến 16/11

- Đội chạy giải pháp trên public test và nộp file kết quả JSONL tại mục Submission trên portal. Hạn mức công bố: tối đa 5 submission/ngày, reset mỗi ngày; điểm và thứ hạng hiện trên public leaderboard.
- Ngày **16/11**, nộp giải pháp hoàn chỉnh qua form **Solution Submission** trên portal dưới dạng package Docker. Hướng dẫn yêu cầu training code, inference code và data; do Docker thường lớn, đội cung cấp checksum và link Google Drive công khai để Ban Tổ chức tải.
- Mỗi đội được tối đa **2 lần nộp solution package**; Ban Tổ chức chỉ đánh giá lần nộp cuối. Hạn **23:59 GMT+7**.

### Vòng private — 20/11 đến 23/11

- Private test được phát hành lúc **09:00 GMT+7 ngày 20/11**. Đội chạy giải pháp đã nộp và nộp kết quả JSONL ở mục Submission trước **23:59 GMT+7 ngày 23/11**.
- Private leaderboard hiển thị theo thứ tự thời gian nộp. Ban Tổ chức đánh giá để xác minh, xem source code và phát hiện gian lận; top 2 đội được công bố ngày 02/12.
- Khi chạy chấm, private test được mount vào container ở `/data` (cùng cấu trúc với public test), inference script được chạy và output lấy tại `result/submission.jsonl`. Container phải offline.

Nguồn: tab Submission Guidance và Rule trên [portal LifeMate](https://challenge.zalo.ai/portal/lifemate), cùng [thể lệ mục 5–6](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf). Tên lệnh/entrypoint, Docker base image và schema đầy đủ cần lấy từ devkit/hướng dẫn theo tài khoản; không suy ra từ ví dụ JSONL.

## Phần chưa thể xác nhận từ trang công khai

Tab **Data** yêu cầu đăng nhập; khi đọc không có phiên đăng nhập, portal trả trạng thái cần sign-in. Vì vậy tài liệu này không ghi số lượng mẫu, file/audio format, sample IDs, data version, split size hoặc nội dung cụ thể của các bộ data. Cần xem tab Data và devkit chính thức theo tài khoản đội.

Các chi tiết cần khóa từ devkit/evaluator: danh mục tool/schema đầy đủ, phiên bản `zaic_tools`, cách khởi tạo và gọi `ToolEnv`, chính xác entrypoint Docker, quy tắc reset/sample loading và artifact checksum. Không lấy những con số hay quy tắc từ repository của đội khác làm quy định chính thức.

## Nguồn

- [Portal LifeMate — Description, Evaluation, Data, Rule, Submission Guidance, leaderboard và submissions](https://challenge.zalo.ai/portal/lifemate)
- [Zalo AI Challenge 2026 Regulations — PDF tiếng Việt](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf)
- [Trang chủ và timeline cuộc thi](https://challenge.zalo.ai/)
