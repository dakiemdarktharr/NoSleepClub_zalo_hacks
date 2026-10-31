# NORA viết bằng Rust: tối ưu được phần nào của phần cứng?

**Đối chiếu 31/10/2026. Nghiên cứu/đề xuất, chưa port code hoặc benchmark NORA.** Đánh giá theo ngân sách client 2 GB RAM, thiết bị GPU 4 GB VRAM, SSD 15 GB; LLM được phép dùng API. Không đánh đồng cấu hình máy phát triển với bộ cài của người dùng.

## Kết luận

**Nên ưu tiên Rust cho mã ứng dụng được phân phối tới laptop:** tray, broker, goal runtime, API clients, state, adapters và audio host. Hướng này tạo cơ hội giảm overhead, dependency footprint và kiểm soát tài nguyên tốt hơn kiến trúc có Python resident. Tuy nhiên, phần lớn memory/compute của voice vẫn thuộc model và engine native; đổi ngôn ngữ điều phối không tự giảm weights, KV cache, graph workspace hay VRAM.

Khuyến nghị thực tế là **Rust application + inference libraries đã được kiểm nghiệm**, gồm whisper.cpp và ONNX Runtime. Chưa có lý do đủ mạnh để viết lại toàn bộ neural inference engine bằng Rust. Mục tiêu hữu ích là bản cài không cần Python, giữ chất lượng và đạt số đo phần cứng; không phải tỷ lệ phần trăm dòng code Rust.

Đây là khuyến nghị kiến trúc mới ở trạng thái [Proposed — ADR-0005](../adr/0005-rust-client-proposal.md), chưa thay thế các ADR trước hoặc chứng minh ngân sách 2 GB đã đạt.

## 1. “Toàn Rust” có hai nghĩa

| Phương án | Runtime được phân phối | Nhận định |
|---|---|---|
| A. Rust broker + Python orchestration/audio host gọn | Python interpreter, dependency cần dùng và native libraries | Khả thi, phát triển nhanh; phải tối ưu resident processes/imports/lifecycle |
| B. Mã ứng dụng Rust, gọi C/C++ inference qua bindings/FFI | Rust executables + native runtime/DLLs + models; có thể không cần Python | Hướng ưu tiên đánh giá cho NORA lightweight |
| C. Cả ứng dụng lẫn inference framework đều dùng Rust | Ví dụ triển khai model qua Candle hoặc backend Rust khác | Phải xác minh operator/model/quantization/GPU support; rủi ro port và chất lượng cao hơn |

whisper-rs là bindings cho whisper.cpp; ort thường gọi ONNX Runtime. Dùng các crates đó vẫn có native C/C++ bên dưới. Một executable Rust cũng có thể phụ thuộc runtime DLL, GPU driver và UI processes. [whisper-rs](https://docs.rs/whisper-rs/latest/whisper_rs/), [ort](https://ort.pyke.io/).

Candle chứng minh có lựa chọn ML framework Rust và hỗ trợ Whisper, nhưng không chứng minh nó thay thế tương đương mọi graph/model hoặc backend của NORA. Cần benchmark cùng weights/precision/task trước khi đổi engine. [Candle](https://github.com/huggingface/candle).

## 2. Lợi ích nào có cơ sở, lợi ích nào chưa được đo?

| Tài nguyên/chỉ tiêu | Rust có thể giúp | Thành phần không tự thay đổi |
|---|---|---|
| RAM idle | Bỏ Python resident và object/framework state không cần thiết; dữ liệu có kiểu, lifecycle rõ | Model đang loaded, native allocators, WebView và các process khác |
| Peak RAM | Bounded buffers/queues, hạn chế copies, chia sẻ model/session có chủ đích | Weights, activations, scratch, KV cache và graph initialization theo backend |
| CPU | Parsing/filtering/audio control và vòng điều khiển có thể nhanh hơn | Tensor kernels đã chạy native; external app/network latency |
| VRAM | Quản lý GPU buffers/contexts và transfers có thể tốt hơn nếu thực sự thay implementation | Cùng model/precision/backend vẫn cần weights và workspace tương ứng |
| SSD/installer | Không cần Python environment; chọn đúng dependencies/features | Model packs, ORT/GPU libraries, cache, updates và rollback |
| Cold start | Có thể bỏ interpreter/import overhead | Đọc weights, tạo/optimize graph, GPU initialization và load voice assets |
| Battery | Event-driven idle, thread/concurrency policy, không polling thừa | CPU/GPU inference và backend thread spinning vẫn phải tune |
| API cost | Có thể giảm request thừa nhờ thiết kế tốt | Token/voice/search pricing không giảm chỉ vì client viết Rust |

Đây là phân tích kiến trúc, **không phải kết quả giảm RAM X% hoặc tăng tốc Y lần**. Nếu Python hiện chỉ là một host gọn gọi cùng native engine, khoảng chênh có thể nhỏ hơn nhiều so với khi so Rust với một môi trường Python/PyTorch/Web UI đầy đủ. So với NORA hiện tại chỉ có policy reference, chưa tồn tại end-to-end baseline để đưa con số thật.

Chỉ phần chạy trên laptop ảnh hưởng trực tiếp ngân sách client. Viết BFF/OA backend bằng Rust có thể thay đổi tài nguyên server, không tự giảm RAM laptop; LLM API cũng vẫn có latency/chi phí/data policy như trước.

Rust quản lý ownership mà không cần tracing GC, nhưng không bảo đảm hết memory leaks hoặc mọi allocation được trả ngay về OS. Rc cycles, retained buffers/caches, allocators và native libraries vẫn có thể giữ memory. [Ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html), [reference cycles](https://doc.rust-lang.org/book/ch15-06-reference-cycles.html).

## 3. Vì sao voice memory không biến mất?

Khi tính riêng từng memory domain, phân rã cần đo là:

~~~text
RAM = CPU model/state + native workspace/cache
    + audio/document buffers + app/runtime state + UI/process overhead

VRAM = GPU model/state + GPU workspace/cache + UI/GPU contexts

Disk = application/native libraries + model packs
     + bounded cache/log + update staging/rollback
~~~

Rust chủ yếu tác động phần app/runtime, copies và lifecycle. Quantization tác động representation của weights; backend/graph optimization tác động compute/workspace; UI và model residency là quyết định kiến trúc. Đây là các biến khác nhau. GPU offload cũng có thể còn host allocations; không coi weights ở VRAM nghĩa là CPU RAM bằng zero.

ONNX Runtime có tuning cho shared allocator và thread pools, còn device tensor/I/O binding giúp quản lý copies. Tài liệu cũng nêu Python API có các cơ hội tối ưu device tensors tương tự C++. Vì thế một cải thiện do đổi allocator/I/O policy không nên được quy toàn bộ cho Rust. [Memory](https://onnxruntime.ai/docs/performance/tune-performance/memory.html), [C API](https://onnxruntime.ai/docs/get-started/with-c.html), [device tensors](https://onnxruntime.ai/docs/performance/device-tensor.html).

## 4. Đánh giá từng thành phần NORA

| Thành phần | Đường Rust thực tế | Điểm cần nghiệm thu |
|---|---|---|
| Silero VAD | ONNX Runtime qua ort; upstream có ví dụ Rust | Sample rate/frame/recurrent state đúng, reset theo session, noise/barge-in |
| Whisper large-v3-turbo | whisper-rs hoặc C ABI gọi whisper.cpp | Cùng model pack/quantization/decoding và backend; no duplicate model contexts |
| VieNeu v3 Turbo | Port host/control loop sang Rust, giữ ONNX graphs và assets phù hợp | Text/phoneme/tokenizer, sampling, KV state, codec/streaming và cancellation parity |
| API LLM/workers | Async Rust HTTP/streaming, typed messages và bounded concurrency | Model request egress, scope, tokens, retry/cancel, không key trong context |
| Goal engine | Explicit state machine + durable local state/effect ledger | Crash/replay/idempotency/UNKNOWN reconciliation, migrations và locks |
| App/file/OA adapters | Typed Rust adapters, Windows APIs hoặc official HTTP APIs | Grant/auth, canonical target, provider eligibility/readback, no bypass |
| UI | Tauri on-demand hoặc native Rust UI để so sánh | Accessibility, mic/approval UX, active/idle memory, GPU contexts |

Silero README có link Rust example; model không buộc phải được chạy bởi Python. whisper.cpp vốn là C/C++ và hỗ trợ quantization. Viết host Rust không phải viết lại ASR. [Silero](https://github.com/snakers4/silero-vad), [whisper.cpp](https://github.com/ggml-org/whisper.cpp).

Workers gọi LLM API là logical tasks với state/permissions riêng, không cần một OS process hoặc local model cho mỗi vai trò. Cả Python và Rust đều có thể làm async I/O; Rust không làm mạng/provider nhanh hơn một cách tự động. Có Rust LLM libraries như Rig, nhưng SDK không tự cung cấp durable goal semantics hoặc NORA security boundaries. Không đưa coding-agent shell/plugin/rebuild capabilities vào scope chỉ vì thư viện có sẵn. [Rig](https://github.com/0xPlaygrounds/rig).

## 5. VieNeu là phần port cần chú ý nhất

Đã đọc source CPU engine và manifest. Đường CPU dùng ONNX Runtime; preprocessing có sea-g2p và tokenizers, đều có lõi Rust. Vì thế bỏ Python khỏi bản cài có cơ sở kỹ thuật. sea-g2p có C ABI cho host khác; vẫn phải chốt API/assets/license phù hợp. [VieNeu dependencies](https://github.com/pnnbao97/VieNeu-TTS/blob/main/pyproject.toml), [sea-g2p](https://github.com/pnnbao97/sea-g2p), [tokenizers](https://github.com/huggingface/tokenizers).

Host TTS còn điều khiển nhiều ONNX sessions, embedding/head arrays, sampling, past-key/value và streaming codec state. Port cần mirror behavior, không chỉ gọi một file model. Bản preset voice là scope nhỏ hơn cloning; encoder/denoiser phục vụ cloning chỉ nên load khi tính năng thực sự cần. [CPU engine source](https://github.com/pnnbao97/VieNeu-TTS/blob/main/src/vieneu/_v3_turbo_engine/onnx_runtime_lite.py).

Dự kiến test port: phoneme/token IDs, shapes/state transitions, deterministic intermediate tensors khi có thể; sau đó nghe chấm names/numbers/code-switching và task outcomes. RNG/float order có thể khác, nên không yêu cầu mọi waveform stochastic giống từng bit hoặc suy một câu nghe đúng thành chất lượng tương đương.

README Anh và file README Việt được đọc trực tiếp hiện cùng ghi fp32 mặc định; source chọn CPU provider và default fp32 graph folder. Một search snippet cũ vẫn ghi INT8 default, nên phải pin release/assets và cấu hình explicit. Upstream cảnh báo INT8 cần VNNI. Tác giả còn báo cold load CPU khoảng 14–19 giây trong benchmark trên máy của họ: đây không phải số đo NORA, nhưng đủ để không hứa cold start rất ngắn chỉ nhờ đổi ngôn ngữ. [README Anh](https://github.com/pnnbao97/VieNeu-TTS), [README Việt trực tiếp](https://github.com/pnnbao97/VieNeu-TTS/blob/main/README.vi.md).

Manifest có cả Web UI dependencies; package được cài không có nghĩa tất cả đã import/resident trong RAM. Cần so cùng chức năng cần dùng, tách footprint trên disk khỏi RAM. Không dùng full SDK/demo environment làm baseline bất lợi một cách cố ý.

CPU source cũng đã giới hạn thread counts và tắt thread spinning ở phần cấu hình được đọc. Port Rust phải bảo toàn rồi đo các lựa chọn này; nếu tạo nhiều sessions với defaults hoặc giữ bản copy tensors thừa, nó có thể nặng hơn host Python được tối ưu. Đây là lý do benchmark phải khóa session options, không chỉ cùng model name.

## 6. Tauri và native UI

Tauri core viết Rust nhưng UI chạy trong WebView; Windows dùng WebView2 với process riêng. Chuyển frontend sang Rust/WASM vẫn giữ engine browser. Vì thế “source ứng dụng toàn Rust” không tự loại WebView memory. [Tauri process model](https://tauri.app/concept/process-model/).

Có thể so với egui/eframe native, nhưng native UI vẫn có renderer/GPU context/buffers và trade-offs về UX/accessibility. Không mặc định native luôn nhẹ hơn ở mọi workload. Với NORA nền, thử tray và tạo/đóng UI theo nhu cầu; **hide window khác destroy WebView**. Phải đo sau khi đóng UI vì runtime có thể còn giữ process/resources. [egui](https://github.com/emilk/egui).

Ưu tiên giữ UI quyết định quyền dễ đọc và tin cậy. Đổi UI chỉ để giảm vài resource counters nhưng làm người dùng approve nhầm là một regression sản phẩm.

## 7. Kiến trúc Rust ứng viên

~~~mermaid
flowchart TD
    UI[Tray và UI theo nhu cầu] --> B[Rust broker: quyền, egress, effects, receipts]
    B --> G[Rust goal engine và API workers logic]
    B --> A[Rust audio host cô lập]
    A --> V[Silero qua ONNX Runtime]
    A --> W[Whisper qua whisper.cpp]
    A --> T[VieNeu host Rust và ONNX graphs]
    B --> D[Typed app/file/OA adapters]
    B --> S[Durable state và verification]
~~~

Broker giữ credentials và quyền hành động; audio host không cần full folder/app/API secrets. Không dồn mọi parser/native runtime vào broker chỉ để giảm số process. Shared allocator/threadpool chỉ áp dụng khi backend/API/lifecycle thực sự hỗ trợ và isolation không bị phá; đo từng phương án.

App-level concurrency: một orchestrator, bounded workers, document/audio queues giới hạn, không một runtime/model cho mỗi agent. Audio: VAD nhẹ resident; ASR/TTS residency thử cả warm và on-demand. Không unload/load mọi lượt một cách máy móc vì cold load có thể phá turn-taking/barge-in. ASR GPU + TTS CPU là một profile cần đo, không phải bảo đảm đạt 2 GB.

Rust safe code giúp giảm một số memory-safety bugs ở phần ứng dụng. FFI/native code vẫn cần audit/patch/isolation, và prompt injection/quyền sai là vấn đề logic không được borrow checker giải quyết. Các boundary tests trong threat model giữ nguyên. [Unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html), [NORA threat model](../strategy/nora-security-threat-model.md).

## 8. Rủi ro phát triển và packaging

- Port goal engine thay LangGraph phải giữ interrupts/resume, durable transitions, retries, cancellation, effect reconciliation và event policy; một async loop chưa tương đương một production workflow engine.
- Rust ownership/types giúp thiết kế dữ liệu, nhưng development/debug/FFI integrations có chi phí. Dự án hiện mới có Python policy reference, chưa có một app Python trưởng thành phải rewrite; có thể chọn runtime mới trước khi đầu tư integration lớn.
- Build/benchmark Release; pin crate/runtime/model revisions. Cân nhắc LTO/strip và chỉ features/providers cần dùng, đo cả binary và throughput. Cargo optimization flags không làm model weights nhỏ lại. [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).
- Không ship binary tối ưu riêng CPU của máy build cho toàn bộ laptop. target-cpu=native dùng host CPU; chọn compatibility baseline/runtime dispatch phù hợp và test thiết bị thiếu VNNI. [rustc codegen](https://doc.rust-lang.org/rustc/codegen-options/).
- Bộ cài có native libraries/driver requirements, không mặc định single executable. Không bundle toàn bộ CUDA toolkit, demo UI, training dependencies hoặc mọi model/provider nếu không cần. Kiểm tra quyền redistributables và update staging trong 15 GB.
- Source/docs ort có version/release-candidate wording thay đổi; maintainer recommendation không phải independent certification. Pin exact version và test Windows/backend/model integration; không tự lấy latest cho production.

Python vẫn có thể dùng cho research/train/export/evaluation trong môi trường phát triển, không đi vào installer. Nếu yêu cầu literal cả toolchain/research cũng Rust, phải tính thêm chi phí và không có lợi ích trực tiếp cho RAM của người dùng.

## 9. Benchmark cần chạy trước khi chốt

| Arm dự kiến, chưa xây | Giữ cố định | Phần khác để tách nguyên nhân |
|---|---|---|
| A | Same models/providers/features/Tauri UI | Rust broker + Python orchestration/TTS host gọn |
| B | Như A | Đổi orchestration sang Rust, TTS host chưa port |
| C | Như B | Đổi audio host sang Rust với cùng native engines/graphs |
| D | Như C | Native UI thay Tauri, cùng nội dung/trạng thái mở/đóng |

Đo local replay với cùng recorded/synthetic LLM responses để tách control overhead; live API track riêng cho end-to-end latency/cost. Khóa weights/precision/decoding/session options, data/grants, task/token/concurrency budgets và features. Không để B/C thắng bằng cách mất recovery/security/voice behavior.

Các tình huống tối thiểu: unloaded tray, warm resident voice, cold start, câu thoại dài, TTS streaming + user interruption, goal nhiều bước, document/browser footprint, crash/resume, disk/update staging. ONNX thread spinning/threadpool policy có thể ảnh hưởng idle CPU/battery ở cả hai host languages; tránh oversubscription và đo. [ORT threading](https://onnxruntime.ai/docs/performance/tune-performance/threading.html).

Acceptance kế thừa: full-process-tree RAM working set và private commit báo riêng, mỗi counter ≤2 GB thập phân theo protocol; GPU dedicated/shared báo rõ; total disk ≤15 GB; critical slots, TTS intelligibility, verified safe task success và p95 latency không suy giảm vượt ngưỡng đã chốt. Tính mọi process NORA spawn, không giấu browser/WebView/native workers hoặc dùng swap để báo đạt. [Evaluation protocol](../strategy/nora-evaluation-plan.md).

Không đặt trước phần trăm tiết kiệm hay kết luận GPU 4 GB chắc chắn đủ. Chỉ freeze runtime sau A/B có matched feature parity và resource traces. Với thông tin hiện tại, **Rust application runtime là hướng đáng ưu tiên thử cho production nhẹ**, còn full reimplementation inference engine là scope cần bằng chứng riêng.

Nguồn và reading limits: [rust-client-sources.json](rust-client-sources.json). Các kết quả upstream là self-reported, source inspection hoặc documentation; không có benchmark NORA mới trong phiên này.
