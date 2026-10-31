# NORA trên laptop phổ thông: ngân sách tài nguyên

Ngày cập nhật: 2026-10-31. **User requirement accepted; implementation và benchmark chưa thực hiện.** Mục tiêu thị trường là laptop khoảng 15–25 triệu đồng; giá bán không được dùng thay cho kiểm tra CPU, RAM, GPU và driver thực tế.

## Phạm vi ngân sách

User đặt mục tiêu **2 GB RAM, GPU 4 GB VRAM, SSD 15 GB**, chấp nhận LLM gọi API. Giả định RAM/SSD là ngân sách riêng của NORA, không phải tổng bộ nhớ/ổ đĩa của máy. GPU 4 GB được xem là giới hạn thiết bị hỗ trợ, không phải yêu cầu mọi máy phải có GPU rời.

Ngân sách RAM phải tính UI/WebView đang mở, broker, audio/model workers, orchestrator và mọi process do NORA tạo. Báo thêm system commit, working set, shared GPU memory và số process; không chỉ đo một Python PID. Browser do NORA spawn phải tính vào task budget; không loại nó khỏi phép đo để làm đẹp số.

App người dùng đang chạy như Excel/Chrome có footprint riêng. Nếu task đòi mở thêm browser nặng, code execution hoặc train model vượt ngân sách, NORA phải chờ, dùng phương án nhẹ hơn hoặc đề xuất compute budget riêng; không bảo đảm mọi task đều nằm trong 2 GB. Model LLM remote không chiếm local model RAM nhưng context, tools và responses vẫn cần giới hạn.

## Kết luận khả thi

Ngân sách này là mục tiêu kỹ thuật đáng thử cho một trợ lý hybrid có flow hẹp và một người dùng. **Chưa có bằng chứng stack local voice đầy đủ đạt peak RAM 2 GB với chất lượng/latency mục tiêu.** 15 GB disk có thể dự trù bằng cách đóng gói bản inference gọn, model packs chọn lọc và quota cache/log. GPU 4 GB cần backend/precision phù hợp và headroom cho Windows/app khác.

Không dùng cấu hình development nhiều toolchains, PyTorch/CUDA packages và nhiều bản weights làm bộ cài bắt buộc cho end user. Khuyến nghị 32 GB RAM trước đó là cấu hình phát triển có dư địa, không phải minimum đã đo cho khách hàng.

## Thay đổi thiết kế được đề xuất

| Thành phần | Hướng tối ưu | Tình trạng |
|---|---|---|
| Tray/broker | Rust resident, WebView chỉ mở khi cần | Chưa triển khai |
| VAD | Silero ONNX trên CPU | Chưa benchmark trên client |
| ASR mặc định để thử | whisper.cpp + large-v3-turbo Q8; thử Q5 khi cần | Chưa pin/build/đo |
| TTS | VieNeu v3 Turbo CPU/ONNX, không PyTorch/CUDA cho TTS | Chưa đo peak RSS/commit |
| LLM | API được user cấu hình; context và output bounded | Chưa chọn provider/model |
| Agent workflow | Một orchestrator, worker logic/async; không nhân bản model/process theo agent | Policy hướng thiết kế |
| State/search | SQLite, paginated tools, streaming/chunked documents | Chưa tích hợp |
| Browser | API/app adapter trước; giới hạn tab, lifecycle và task memory budget | Chưa triển khai |

whisper.cpp hỗ trợ native C/C++, Windows, CPU/GPU backends và quantization. Bộ model của tác giả runtime có `ggml-large-v3-turbo-q8_0.bin` khoảng **874 MB**, Q5 khoảng **574 MB**, bản không quantize khoảng **1,62 GB**. Đây là **kích thước file**; không dùng nó để suy ra peak RAM/VRAM hoặc chất lượng tiếng Việt. [Runtime](https://github.com/ggml-org/whisper.cpp), [model artifacts](https://huggingface.co/ggerganov/whisper.cpp/tree/main)

Chọn Q8 làm ứng viên đầu để đánh giá trước khi giảm số bit thêm hoặc đổi sang model nhỏ. Không giả định Q8/Q5 giữ nguyên accuracy; cần paired tests với cùng audio/decoding settings. GPU backend phải được chọn theo card/driver: không coi mọi GPU 4 GB có hiệu năng hoặc khả năng CUDA giống nhau.

VieNeu v3 Turbo có CPU ONNX path không cần PyTorch. Tác giả yêu cầu VNNI cho CPU INT8 và cảnh báo chất lượng sai trên CPU không hỗ trợ. Dùng FP32 trước trên máy chưa xác minh. Nano là preview có đánh đổi chất lượng, nhất là English/code-switching; chưa chọn làm default. [VieNeu](https://github.com/pnnbao97/VieNeu-TTS)

Zipformer Vietnamese 30M là ứng viên CPU nhẹ cần so sánh, không phải thay thế đã chứng minh tương đương Whisper. Model card là benchmark tác giả; cần audit model/data rights và code-switching/critical-slot evaluation trước khi chọn. [Model card](https://huggingface.co/hynt/Zipformer-30M-RNNT-6000h)

## Chế độ mở rộng độ tương thích

Local voice + LLM API là hướng ưu tiên thử nghiệm. Nếu không đạt 2 GB hoặc CPU/GPU không đáp ứng, có thể cung cấp chế độ speech cloud để giữ chất lượng model và giảm tải local. Đây là **tùy chọn thiết kế**, không có upload tự động: user phải đồng ý audio/transcript nào được gửi, tới dịch vụ nào, quota/chi phí và retention tương ứng.

Không tự đổi sang voice cloud khi model local thiếu bộ nhớ. Local-only preference vẫn phải được tôn trọng; khi không đáp ứng thì nói rõ, cho phép nhập text hoặc chọn model/latency tradeoff. LLM API cũng không tự cấp quyền gửi mọi file, transcript hay context ra ngoài.

## Targets, chưa phải số đo

| Target | Điều kiện cần đo |
|---|---|
| Idle RAM khoảng 250 MB hoặc thấp hơn | Tray mode, UI đóng; nêu model nào còn resident |
| Peak RAM NORA ≤2 GB | Cả cây process, kể cả khởi động, model loading, phát/nghe chồng nhau và goal execution |
| GPU memory mục tiêu khoảng ≤3 GB trên card 4 GB | Tính headroom Windows; theo dõi cả dedicated/shared GPU memory |
| Installed runtime/models mục tiêu ≤10 GB | Giữ phần còn lại của 15 GB cho bounded cache/log/update staging |
| UI/audio không đứng khi goal dài | Bounded concurrency, backpressure, streaming/chunked I/O |

Những con số này là **acceptance targets do thiết kế đặt ra**, không phải benchmark suy ra từ upstream. Nếu không đạt phải thay kiến trúc/profile hoặc thảo luận lại ngân sách; không coi swap/pagefile là đạt RAM target.

## Chất lượng phải được giữ bằng phép đo

So sánh Q8/Q5 với baseline không quantize trên cùng corpus Việt Bắc/Trung/Nam, noise, app/file names, số tiền/người nhận/ngày giờ và English–Vietnamese code-switching. Đo WER/CER, critical-slot accuracy và verified task success; không chỉ tốc độ. LLM không được sửa transcript theo suy đoán rồi coi là dữ liệu đã xác nhận.

TTS nghe thử theo câu ngắn, tên riêng, số/ngày, code-switching; đo lần phát đầu sau idle, gián đoạn, played-audio context và CPU khi ASR cũng hoạt động. Không chỉ đo warm steady-state hoặc preset câu dễ.

Profile resource cần gồm startup/model load, idle 30 phút, hội thoại liên tục, interruption, một research goal, disk quota, browser launch, low-memory, sleep/resume và update. Giới hạn memory có thể giết process nên effects phải có receipt/reconciliation; đừng chỉ đặt hard cap rồi coi crash là xử lý tốt.

Không có phép đo nào ở trên đã được chạy trong bản reference hiện tại. Bước triển khai tiếp theo là một audio benchmark harness/native spike trước khi chốt model pack hoặc minimum hardware.
