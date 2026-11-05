# NORA — báo cáo khó khăn, bài học và bàn giao

Cập nhật ngày 03/11/2026, múi giờ Asia/Saigon. Đây là báo cáo từ mã nguồn, kết quả và receipts đã lưu; lượt báo cáo này không chạy thêm model, training hoặc submission. Goal hiện **paused**, chưa hoàn thành. Khi người dùng yêu cầu tiếp tục, dùng phần bàn giao dưới đây; không tự chạy lại thí nghiệm đang có kết quả terminal.

## 1. Kết luận cần đọc trước

**NORA chưa đạt 99% hoặc 100%, chưa có điểm public/private của BTC và chưa được chứng nhận production-ready toàn bộ.** Kết quả mới nhất trên 123 mẫu engineering validation là **2,9810298103% theo bản chấm local mô phỏng metric đã lưu**. Đây là điểm tác vụ tổng hợp, không phải tỷ lệ nhận đúng từng câu nói. Không đổi tên phép đo thành official accuracy.

| Phép đo | Kết quả thực | Giới hạn diễn giải |
|---|---|---|
| Native audio → LLM → SDK, 123 mẫu, CPU output | 2,9810%; 79 failure/timeout; 2 mẫu nonempty-perfect | Local mirror, không phải điểm BTC; failures được giữ trong denominator |
| Cùng 123 mẫu, chuyển output projection sang GPU | 2,9810%; 78 failure/timeout; 2 nonempty-perfect | Điểm từng mẫu: 0 tăng, 0 giảm, 123 bằng; median model time giảm khoảng 5,4%, chưa qua gate 20% |
| Bộ 200 câu MCQ tổng hợp, HF base NF4 chưa adapter | 114/200 = 57%; 199 input duy nhất | Baseline phát hiện suy giảm năng lực, không phải LifeMate/MMLU hoặc chứng nhận an toàn |
| Historical Colab profile, 30 mẫu | 13,5556% local task score | Khác cohort/profile/model; không dùng so trực tiếp với 123 mẫu để kết luận mức suy giảm |
| Training V13 | Một update trên đủ 945 tokens; save/reload adapter trong cùng base process đã audit | Chưa fresh-process full-model restore, optimizer resume hoặc đo quality |
| Training V17 gần nhất | Sáu updates; đã xong forward 5.177 tokens; guard dừng khi backward | Quan sát sơ bộ từ parent receipt/raw report; chưa hoàn tất terminal audit độc lập; không có adapter save hoặc quality gain |

Hai arm 123 mẫu có cùng actual initial input bytes ở 117 mẫu quan sát được. Sáu mẫu còn lại đều lỗi trước model; không coi hai input bị thiếu là bằng nhau. Cặp thử không được promote và không đổi default. Các phép đo ở những hàng khác nhau có backend và mục tiêu khác nhau, không trộn thành một điểm chung.

**Nguyên nhân chất lượng thấp quan trọng nhất:** actor còn chọn sai ý định, sai thực thể, sai điều kiện, quản lý tiến độ/dừng yếu; ASR và ngân sách thời gian làm vấn đề nặng thêm. Kết quả hiện có không ủng hộ cách giải thích rằng chỉ cần Rust nhanh hơn, JSON chặt hơn hoặc sửa một prompt là đủ.

## 2. Những khó khăn khi tái hiện cách BTC chấm

### 2.1. Ba môi trường có ba giới hạn khác nhau

| Phạm vi | Ngân sách/đường chạy đã thống nhất | Điều không được suy ra |
|---|---|---|
| Desktop sản phẩm | Windows Job cap 2.000.000.000 bytes commit, GPU 4 GB, gói SSD 15 GB; LLM API | Không phải tổng RAM hệ điều hành và mọi ứng dụng; API không được đưa vào đường thi offline |
| Bản thi | Local LLM; cấu hình đích A30/64 GB của BTC được người dùng cho phép | Chưa có máy A30 hoặc lượt Docker/runtime chính thức để kiểm chứng |
| Máy nghiên cứu lattice | GTX 1650 4 GiB, SM75, RAM 16 GB; training pilot dùng scope 8 GiB, swap 0 | Không phải bằng chứng desktop vừa 2 GB hoặc bản thi đáp ứng latency trên A30 |

Windows từng thiếu bộ nhớ commit do các ứng dụng đang mở, nên build/inference thật bị hạn chế. Người dùng yêu cầu tiếp tục phần độc lập và không tự đóng app; sau đó GPU research chuyển sang lattice. Colab cần đăng nhập/xác minh, quyền lưu notebook trên Drive và từng hết quota GPU miễn phí. Đó là trở ngại môi trường, không phải kết quả chấm điểm. Không có paid cloud được dùng để vượt rào cản này.

### 2.2. Có dataset chưa có nghĩa đã có submission

Bản yêu cầu và dataset đã lưu có train 1.545 mẫu, public test 634 mẫu, không có public gold. Protocol đã đọc quy định local model, LLM dưới 5B/all models dưới 6B, context không quá 16.384 và hạn mức 30 giây/mẫu; môi trường đích A30/16 CPU/64 GB. Trước lần nộp sau này phải đối chiếu lại rules hiện hành và ngày phát hành model, không dùng báo cáo này thay bản đề mới nhất.

Public Submissions cần file kết quả đúng định dạng cho đủ 634 mẫu. Đã tải dữ liệu, chuẩn bị adapter hoặc mở trang nộp bài không đồng nghĩa đã sinh đủ kết quả, upload hay nhận điểm. **Chưa có official submission/score NORA.** Public test cũng không thể dùng làm gold để sửa từng đáp án.

### 2.3. SDK chạy được khác với làm đúng tác vụ

Luồng kiểm thử có ý nghĩa là audio + context/history + catalog được cấp → ASR → local actor → original SDK → actual executed calls và final state hash. History có những effect đã xảy ra; chạy lại lời gọi cũ có thể tạo thêm tác động ngoài ý muốn. IDs phải đến từ dữ liệu được cấp hoặc tool observations, không đọc hidden state/gold để giúp actor.

Bản chấm local dùng quy tắc matching đã công bố trong tài liệu, gồm NFC/lower/whitespace và fuzzy token cho trường tự do; chi tiết punctuation của BTC chưa đủ để khẳng định tương đương hoàn toàn. Các trường không tự do vẫn phải khớp theo contract. State mismatch có thể làm điểm bằng 0 dù một số calls đúng; extra calls hoặc optional defaults cũng có thể làm lệch trace/state. Một read như email.read có thể có effect riêng, nên không mặc định mọi thao tác đọc đều vô hại.

**Bài học:** luôn giữ riêng bốn tầng: output parse được; SDK nhận lời gọi; effect đúng ý và hoàn tất; scorer/official server chấm đạt. Test pass ở tầng trước không chứng minh tầng sau.

## 3. Lỗi thấy được trên audio và actor

Đã đọc đủ 123 diagnostic cards và ghi nhận xét mở, sau đó dùng sáu bộ kiểm tra deterministic cho speech, phạm vi tool, grounding, args, control flow và budget. Đây là annotation của agent và các proxy kiểm tra, chưa phải nhãn chuyên gia độc lập hoặc semantic judge được hiệu chuẩn.

| Bề mặt lỗi | Quan sát từ trace | Cần thay đổi cách đánh giá/thiết kế |
|---|---|---|
| ASR tiếng Việt | Latest-turn CER micro 18,6898% trên 117 transcripts có quan sát; sai tên, số điện thoại, thời gian; có câu quảng cáo/kết video bị hallucinate | Đo critical slots và audio → SDK; giữ sáu missing-ASR cases trong task denominator; không đoán lại bằng gold |
| Chọn ý định | Nghe được yêu cầu chuyển khoản nhưng actor chọn trả hóa đơn | So khả năng hiểu tác vụ của base model trước khi chỉnh parser hoặc train dài |
| Gán số vào tham số | Yêu cầu điều hòa 24 độ và nhắc việc lúc 7 giờ nhưng đề xuất độ sáng đèn 24/tốc độ quạt 7 | Bắt buộc liên kết số với đúng đối tượng, đơn vị và điều kiện |
| Chọn thực thể/điều kiện | Sai điểm đi/đến, phương tiện, ngày; hủy thêm đơn ngoài yêu cầu | Kiểm phạm vi tác động và ý định người dùng, không chỉ ID tồn tại |
| Grounding | Dùng ID đã thấy nhưng sai namespace, loại thực thể hoặc subtype | Phân biệt contact/beneficiary/chat/menu-item/device; provenance toàn cục hiện chưa đủ |
| Thiếu dữ kiện | Bỏ lookup phụ thuộc, điền số minh họa hoặc không hỏi lại | Đánh giá chuỗi lookup → hỏi thiếu thông tin → effect riêng, giữ đúng schema đích |
| Điều khiển tiến độ | Lặp lời gọi hoặc lặp đề xuất bị từ chối, xong việc chính vẫn làm tiếp; đôi khi dừng thiếu bước | Theo dõi subgoal/effect receipts, tránh retry effect đã thành công; đo cả over-action và under-action |
| Thời gian/context | Baseline A có 61 generation deadline, 8 turn-budget, 4 HTTP400 và 6 ASR deadline | Phân bổ budget toàn hành trình; giữ prompt/history/catalog trong context thực, dành thời gian kết thúc |

Baseline A có 16 mẫu chưa có quyết định model, gồm sáu lỗi ASR trước model. Có 45 mẫu lặp identical executed calls, 35 mẫu lặp nonempty proposals và 45 mẫu có tổng 240 provenance refusals. Các nhóm có thể chồng lấp, không cộng thành phân bố nguyên nhân độc lập. Không có refusal không chứng minh grounding đúng; có refusal cũng không tự chứng minh guard là nguyên nhân gốc.

Server log cùng run ghi bốn context overflow requests 6.340/6.250/6.614/6.302 tokens vượt context 6.144. Chưa join request task ID với từng sample; không gán chắc mọi HTTP400 của mọi lượt vào context overflow, và không suy có/không truncation chỉ từ sự vắng mặt của một log flag.

### Hiệu chỉnh quan trọng: `ok=true` chưa phải thành công

Trong 673 SDK calls của baseline A, **74 calls ở 14 mẫu trả `ok=true, done=false`**. Đọc messages cho thấy các ca như email không hợp lệ, ID sai loại, item không thuộc menu hoặc điều kiện ứng dụng chưa đạt. Không được kể những attempts này là đã tạo nhiều draft, đặt đồ ăn hoặc tiết lộ vị trí thành công. 599 calls còn lại không có explicit failure cũng không tự chứng minh mỗi call làm đổi state.

Ghi chú ban đầu được giữ nguyên cùng errata V2, không sửa traces hoặc điểm để hợp với diễn giải mới. Đây là bài học về audit: phải đọc semantic receipt và final state, không đếm tên function hoặc cờ `ok` thay cho effects.

## 4. Các hướng đã thử và điều chúng thực sự chứng minh

| Hướng | Kết quả trong đúng profile đã thử | Quyết định |
|---|---|---|
| Thêm few-shot compact typed | 8,3333% → 11,6667% trên 30 mẫu; nonempty-perfect vẫn 0; thêm hai failures | Không qua frozen promotion gate, không promote |
| Đổi descriptive aliases | 8,3333% → 6,6667% trên 30 mẫu | Không promote |
| PhoWhisper thay turbo dưới cùng Q3 actor | 8,3333% → 3,3333%; mỗi arm một failure | Không promote từ CER đơn thuần |
| Schema-bound clarification V30 | 8,3333% → 5%; tám repeated-params refusals ở một mẫu; có extra ask ở zero-action task | Không promote; ghi riêng lỗi decoder và lỗi policy |
| Sửa uniqueness V31 | Unit/integration và hai native synthetic probes qua | Chưa có task score, chưa claim gain |
| CPU-output → GPU-output V32 | Cùng 2,9810%/123; median model time giảm khoảng 5,4% | Không qua gate latency 20%, không đổi default |
| Hard lexical/semantic tool routing | Bỏ sót tools cần thiết và thêm độ trễ trong các probes đã lưu | Không đưa lại hard filter chỉ vì retrieval recall có vẻ tốt |
| PlanTyped/native/xLAM probes | Format/probe có thể qua nhưng task chất lượng vẫn thấp trong các cấu hình đã thử | Chưa đủ để bác bỏ cả họ architecture; xLAM còn giới hạn license research/noncommercial |

Pinned llama.cpp bỏ qua `uniqueItems`; không có warning không có nghĩa constraint được thực thi. V31 bổ sung mọi ordered subset không rỗng cho schema nhỏ, có expansion budget và runtime fallback. Đây là decoder compatibility fix, không tự giải quyết sai ý định.

### Kiểm tra xem lỗi có chỉ do GGUF/grammar không

Đã chọn hậu kiểm sáu runtime prefixes để chẩn đoán, không phải mẫu ngẫu nhiên ước lượng accuracy. Native Q4 bỏ grammar vẫn sai một số ý định. HF NF4 sau khi sửa backend cũng sai chuyển khoản → trả hóa đơn và điều hòa/nhắc việc → đèn/quạt. Có một ca HF kết thúc đúng hơn native, nhưng chưa có task benchmark HF tương ứng.

HF V1 thất bại trước real prefix vì đường efficient SDPA/GQA không xử lý đúng cấu hình Q/KV heads khi mask trống. V2 thêm explicit causal mask, qua các controls rồi lỗi wrapper: tokenizer trả BatchEncoding nhưng code giả định Tensor. V3 lấy `input_ids` rõ ràng, kiểm token equivalence trên cả sáu prefixes, hoàn thành outputs/EOS và được audit CPU độc lập. Giữ cả V1/V2 failures.

CPU exact-control đầu tiên còn bị khác kernel tự chọn; sau khi ép cùng MATH kernel và cùng inputs mới exact. Cache next-token argmax khớp full recompute, nhưng max logit difference 0,015625: không gọi đó là numerical identity. HF NF4/native Q4 khác quantization, kernels và preparation; chưa cô lập được từng nguyên nhân.

**Bài học:** không quy hết cho backend khi sai ngữ nghĩa còn tái hiện ở backend khác; cũng không khẳng định hai backend tương đương vì tokenizer hoặc một argmax khớp.

## 5. Khó khăn phần cứng và fine-tuning trên GTX 1650

Rust giảm overhead ở lifecycle/UI/broker, nhưng phần lớn VRAM thuộc weights, activations, KV/cache và kernels. Đổi ngôn ngữ không tự làm một model đủ năng lực vừa giới hạn hay hiểu đúng tool. NORA vẫn dùng native whisper.cpp/llama.cpp/ONNX Runtime; Python còn ở original contest SDK và research/training tooling. Không có Python runtime shipping trong desktop path đã đóng gói, nhưng không gọi toàn bộ ML stack là Rust thuần.

GTX 1650 SM75 không hỗ trợ BF16 native. Training pilot dùng NF4 frozen body, FP16 compute/tied table, FP32 norm/gates và LoRA; không giả vờ tương đương BF16. Pin versions của Torch/Transformers/PEFT/bitsandbytes và checkpoint là cần thiết vì GQA/GDN, quantization, tokenizer và wheel/kernel support không tự tương thích.

Các trở ngại chính đã gặp:

- **Finite loss nhưng gradient không finite.** Static scale 128 gây NaNs; scale 8 giúp finite nhưng vẫn phải kiểm gradient parity. Loss đẹp không phải bằng chứng optimizer update đúng.
- **Selective logits có thể đổi phép tính.** Chọn rows trước head làm đổi GEMM shape/kernel và gradients; CPU FP32 head riêng chưa đủ. Dùng global tiles 128 và full target/EOS, kiểm exact controls theo đúng scope. Không cắt prefix/context hoặc bỏ mẫu dài để gọi fit.
- **Frozen tied embedding/head rất lớn:** bảng FP16 [248320, 2048] chiếm 1.017.118.720 bytes. CPU transport phải giữ tie, shared backing, shape/stride/storage offset, gồm expanded views, và trả đúng dữ liệu khi backward.
- **Vision weights không dùng:** đã đo release tiết kiệm 190.087.680 CUDA allocated bytes, giữ text storage và synthetic logits. Không tính VRAM tiết kiệm chỉ bằng parameter count × dtype.
- **Cache khác live allocation.** `empty_cache` có thể giảm reserved memory, không xóa activations còn sống. V16 có cache-release events nhưng vẫn chưa qua ngữ cảnh dài nhất.
- **Swap guard phải có attribution.** V12 dừng do global swap-out tăng dù own sampled swap 0; không đủ căn cứ đổ cho app khác. Protocol sau dùng kernel scope MemoryMax 8 GiB/MemorySwapMax 0 và theo dõi global vmstat riêng.
- **Save/reload cần kiểm inventory thật.** PEFT rút gọn 186 full target paths thành 12 suffixes; auditor đầu giả định spelling nguyên vẹn nên từ chối. Auditor sửa bằng resolve actual keys và 372 saved tensors, không nới coverage.
- **Một process reload chưa là resume hoàn chỉnh.** V13 lưu/nạp lại adapter trong cùng base process đã qua; fresh-process model/optimizer resume và native adapter export chưa kiểm chứng.

| Pilot | Mốc thực đạt | Điểm dừng/kết luận |
|---|---|---|
| V13 | Một update đủ 945 tokens, save/new-adapter reload và CPU tensor audit | Một bước khả thi; chưa full SFT/quality |
| V14 | Ba updates | Guard dừng backward 2.655 tokens |
| V15 | Full 945-token logits/loss/372 gradients exact resident control; một update | Guard dừng backward 1.940 tokens; không adapter save |
| V16 | Full 1.940-token control exact; sáu updates đến 3.790 tokens | Guard dừng ở 5.177 tokens, GPU free 509.607.936 bytes; terminal audit hoàn tất |
| V17 | Offload Adam moments giữa các bước; sáu updates đầu loss/norm exact V16; forward 5.177 tokens xong | Guard dừng backward tại 532.676.608 bytes = 508 MiB, dưới margin 512 MiB đúng 4 MiB; full terminal audit còn thiếu |

V17 có sáu offloads, mỗi lần giảm 33.693.696 CUDA allocated bytes; năm restores trước dừng. Parent receipt báo failed/guard stop và owned process group đã rỗng. Child report còn chữ `running` do bị SIGTERM trước lần ghi tiếp; không dùng chữ đó để restart training. Số liệu sơ bộ: elapsed khoảng 793 giây, min sampled host available 8.586.145.792 bytes, peak cgroup 4.940.103.680 bytes, own sampled swap/OOM 0. Đây **không phải CUDA OOM được xác nhận**, cũng không chứng minh training không thể vừa nếu đổi cách làm. Chưa save adapter, full bảy updates hoặc quality evaluation.

Lazy causal mask/CausalBias, offload mask theo layer, safe optimizer checkpoint codec và dense 4B là các hướng cần kiểm chứng sau khi resume; chưa triển khai/chạy để gọi hiệu quả. Không hạ memory guard hoặc giảm dữ liệu chỉ để báo pass.

## 6. Những sai số trong phương pháp đã phát hiện và sửa

1. **Đánh giá quá hẹp.** Bộ 30 mẫu đã dùng để xem/sửa nhiều lần; chuyển sang 123 mẫu bộc lộ thêm nhiều lỗi. Các mẫu validation đã gold-replay/tokenize nên là engineering validation, không phải blind holdout. Không gọi cải thiện trên mẫu đã xem là generalization.
2. **Biên dữ liệu cần ghi đúng.** Development/outer là 1.235/310; internal train/validation là 1.082/153. Actor data có 3.377 train records từ 1.082 mẫu, 484 validation records từ 152 mẫu; train_1696 bị quarantine khỏi actor trajectories nhưng không bị loại khỏi đánh giá. Scorer cũ từng deserialize cả holdout hậu kiểm trước khi lọc; chưa có bằng chứng gold vào inference, nhưng không thể nói historical holdout chưa từng được đọc vào memory. Selected-only loader đã sửa phạm vi.
3. **Đừng dùng số test làm accuracy.** V31 Windows workspace 287 pass/0 fail/3 ignored; V32 Windows package 109/0/3 và Linux release/whisper all-targets 111/0/3. Khác scopes, không cộng. Chúng chứng minh contracts/code paths tương ứng, không chứng minh nghe/nói hoặc 99% tác vụ.
4. **Giữ mọi thất bại.** Không bỏ timeout, missing ASR, tool no-op, backend initialization failure hoặc context dài khỏi denominator/phạm vi báo cáo để nâng điểm. Các lần auditor sai giả định được giữ cùng correction; traces và metric không đổi.
5. **Probe đúng chưa là task đúng.** Tokenizer HF/GGUF đã khớp 3.861 records; synthetic probes/gradient parity/save-reload qua không phải bằng chứng model hiểu tác vụ. SFT loss, training capacity và task score là ba deliverables khác nhau.
6. **Resource telemetry có giới hạn.** GPU snapshots 1 giây có thể bỏ lỡ transient peaks; RSS cộng của nhiều process có thể đếm shared pages nhiều lần. Dùng PSS/cgroup/allocated/reserved/free đúng tên, không đổi file size thành VRAM hoặc Job commit thành physical RAM.
7. **Trạng thái job phải dựa vào tiến trình thật.** Tool handle mất/đang chạy không đủ. Dùng parent terminal receipt, PID/PGID/start ticks và cleanup; không kill mọi Python hoặc chạm compositor/app không thuộc thí nghiệm. Chạy Python `-I` để tránh helper `inspect.py` che stdlib; giữ stdout JSON/UTF-8 riêng để tránh lỗi Windows cp1252.
8. **Cần capability baseline sớm hơn.** Bộ 200 MCQ hiện có 57%; thời gian 24/50, số lượng 15/50, evidence 35/50, safety/completion 40/50. Chỉ là các template tổng hợp, nhưng đủ cho thấy cần sàng lọc base model trước khi đầu tư thêm nhiều vào prompt/kernel engineering. Lẽ ra wider task baseline và đọc hơn 100 errors nên diễn ra sớm hơn.

## 7. Các rủi ro sản phẩm và cybersec còn mở

Desktop đã có một hành trình Windows synthetic WAV → ASR → API → TTS, Job peak commit 1.702.887.424 bytes; gói mới có notices khoảng 1.203.779.356 bytes. Đây là một journey quan sát được, không phải chứng nhận mọi workflow/mọi laptop trong trần 2 GB. WAV chào hỏi 1,68 giây có launch → final text khoảng 16,2 giây và playback drain khoảng 25 giây; chưa là live-mic latency hoặc bằng chứng người dùng nghe được/hiểu được.

Các tools thực có scoped folder reads/search, preview/create-new note, calculator, time/reminders, approved public research và hash-registered EXE launch. Chưa có arbitrary GUI automation, shell, gửi tin nhắn, Zalo OA/OMS production integration. Policy phân biệt simple/complex nhận TaskFacts đã cấu trúc, nhưng chưa xác minh runtime classifier/call-site hoàn chỉnh và automatic multiworker graph. Không báo các phần này đã xong chỉ vì có module Rust.

AppContainer/Job/grants/approvals/receipts đã có negative controls trong phạm vi test, nhưng semantic authorization còn thiếu. ID đã thấy không chứng minh người dùng cho phép tác động lên thực thể đó; schema hợp lệ không chứng minh đúng người nhận, số tiền hoặc điều kiện. Tool/app/web text phải là dữ liệu không đáng tin, không được nâng thành quyền thay user. Không bỏ guards để tăng benchmark score, không log secrets hoặc cho model đọc `.env.local`.

Những gate production còn cần: live-mic tiếng Việt có critical slots/tên riêng, far-field/AEC/barge-in và audibility; long soak/crash/recovery; hành trình với ứng dụng thật; clean-machine/native runtime checks; thiết bị Windows 4 GB VRAM đích; installer/signing/update; license/redistribution của models/voices/native DLLs; kiểm supply-chain ngoài Cargo; kiểm semantic security và tích hợp Zalo thực. Ý tưởng novelty bảo mật chưa thay được bằng chứng triển khai và thử nghiệm người dùng.

## 8. Model tiếp theo và thứ tự bàn giao khi được resume

### Cho Codex/agent nhận việc tiếp

- Giữ `main`, không tạo branch. Theo gate hiện hành, chưa commit/push phần LifeMate trước kết quả 100% thực; không tự coi báo cáo hoặc unit tests là đạt gate. Working tree có nhiều phần sửa/untracked, gồm crate LifeMate; không reset/clean để lấy lại HEAD. `lantern-vn/` không thuộc nhiệm vụ.
- Goal đang paused; không tự tải weights, chạy GPU hoặc nộp bài từ báo cáo này. Không đọc/copy `.env.local`, không dùng API/paid cloud cho contest và không đóng ứng dụng của người dùng.
- Đọc báo cáo này, error review, current-state và receipts mới trước các đoạn nhật ký cũ. Một số snapshots lịch sử có chữ active/running/chưa launch; terminal mới có scope rõ ràng mới là nguồn quyết định.
- V17 đã terminal theo parent receipt, không restart chỉ vì child report `running`. Khi được resume, việc đầu tiên là đóng audit/collection độc lập V17, không tính thêm một lần GPU chạy là audit.
- Tách desktop API/2 GB, contest local/A30 và research lattice. Giữ original SDK, split, all failures, source/asset pins, input byte checks, metrics và promotion gates cố định trước mỗi cặp thử.
- Hướng thay architecture được phép; không bắt buộc giữ actor hiện tại. Tuy nhiên soft routing, constrained planning hay base mới phải chứng minh bằng cùng audio → SDK benchmark; không thay benchmark sau khi thấy kết quả.

### Cho lựa chọn base model tiếp theo

Ứng viên đã xem metadata là **Qwen/Qwen3-4B-Instruct-2507**, revision `cdbee75f17c01a7cc42f958dc650907174af0554`, dense/non-thinking. Theo actual safetensors headers, có 4.022.468.096 BF16 stored parameters. Đây là ứng viên để thử khả năng hiểu tác vụ và đường kernel đơn giản hơn, **chưa có bằng chứng tốt hơn, vừa VRAM hoặc đáp ứng 30 giây**.

Trạng thái stage chính xác: bảy small assets/metadata và ba bounded range headers đã có; **chưa tải đủ ba weight shards ~8,045 GB, chưa load HF 4B, chưa train hoặc đo drift/task**. GGUF 4B/Q3 có từ thử trước là artifact khác; đừng nhầm với HF stage mới. `download-plan.json` chưa được tạo và full-download script chưa chạy.

Index khai `total_size` lớn hơn actual tensor-header data 655.360 bytes. Header counts khớp API và weight map; giữ nguyên index, kiểm đủ full-file LFS SHA-256 trước load. Sai khác metadata này tự nó không chứng minh model độc hại. Không dùng `trust_remote_code` hoặc sửa provenance để lờ qua sai khác.

Thứ tự có thể review được sau resume:

1. Hoàn tất V17 terminal audit, ghi rõ failed capacity pilot, không quality gain.
2. Pin/verify đầy đủ ứng viên và kiểm eligibility theo rules; chạy cùng 200 drift items và sáu diagnostic prefixes trước. Sáu prefixes chỉ chẩn đoán; không dùng làm accuracy gate độc lập.
3. Chạy cùng 123 audio → SDK trong profile đã freeze, giữ deadline/failures. Đo actor, ASR, latency, state và over-action riêng; chỉ promote khi vượt gate đã đăng ký trước.
4. Nếu chất lượng base đáng tiếp tục, mới đầu tư full-length SFT capacity, safe fresh-process optimizer resume, held-out checkpoint selection và native export. Giữ outer holdout khỏi tuning; ghi rõ lịch sử tiếp xúc labels.
5. Đánh giá submission format và đủ 634 real public outputs trong runtime được phép; sau đó mới nộp và nhận official score. Production desktop vẫn có release gates riêng.

**Điều học được quan trọng nhất:** tối ưu bộ nhớ và tính đúng của runtime rất cần, nhưng không thay được khả năng hiểu ý định. Quy trình tiếp theo nên bắt đầu bằng benchmark rộng, đọc lỗi và sàng lọc model; sau đó mới dành công sức tối ưu sâu ứng viên có triển vọng. Không có kết quả hiện tại cho phép hứa chắc 99%/100% trên bài ẩn.

## 9. Điểm vào bằng chứng và tài liệu

Raw data, labels, predictions, weights và training outputs giữ trong `.artifacts`/lattice, không thêm vào Git public. Liên kết raw dưới đây phục vụ workspace; chúng không phải tài liệu public đã phát hành.

| Nguồn | Dùng để kiểm tra |
|---|---|
| [Error review](lifemate-error-review-2026-11-03.md) | 123 samples, error taxonomy, SDK no-op errata, backend/drift audits |
| [Project review](project-review-2026-11-03.md) | 12 crates, runtime/call-site gaps, lịch sử tests và pilots |
| [Validation ledger](../lifemate-validation.json) | Requirements, scopes, publication state và historical snapshots |
| [Architecture](../architecture.md), [production integration](../production-integration.md), [validation](../validation.md) | Desktop thực tế và release gates |
| [Ground truths](../../ground-truths.json) | GT171–177: wider123, V15/V16, open coding, no-op correction, HF backend và drift baseline |
| [V32 paired summary](../../.artifacts/lifemate-eval-v32/placement-paired-summary.json) | Hai arm và quyết định không promote |
| [123 error surface](../../.artifacts/lifemate-eval-v32/error-surface-baseline/summary.json), [errata](../../.artifacts/lifemate-eval-v32/open-coding-errata-v2.json) | Proxy counts, tránh diễn giải attempts thành effects |
| [HF drift summary](../../.artifacts/lifemate-eval-harness-v1/runs/hf-base-v1/summary.json) | 57% trên 200 MCQs, không phải LifeMate |
| [V17 parent receipt](../../.artifacts/qlora-sustained-v17-preaudit/run-receipt.json), [child report](../../.artifacts/qlora-sustained-v17-preaudit/report.json) | Terminal guard stop và sáu updates; chưa độc lập audit toàn bộ |
| [4B header inspection](../../.artifacts/qwen3-4b-official-stage-v1/header-inspection.json) | Header-only stage, metadata mismatch, full weights còn thiếu |
| [Compact current state](../../.artifacts/nora-current-state.json) | Pause/handoff mới; các fields lịch sử không tự trở thành latest |

Không có code/runtime change, GPU run, submission, commit hoặc push được thực hiện trong lượt lập báo cáo này.
