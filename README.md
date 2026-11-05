# NORA · NoSleepClub

## Bản bàn giao model — 05/11/2026

**[Tải checkpoint LifeMate V2 step64](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/releases/tag/lifemate-v2-step64-research-20261105)** · [Hướng dẫn bàn giao và việc cần làm tiếp](docs/handoff/lifemate-v2-step64/README.md). Gồm PEFT adapter, checkpoint optimizer/RNG, source và hồ sơ kiểm chứng. Đã hoàn tất 64/1.082 update; 504 tensor adapter khớp checkpoint. **Adapter này chưa được chấm chất lượng, chưa production-ready.** Xuất bản theo yêu cầu bàn giao ngày 05/11; không phải xác nhận đạt 100%.

Pipeline sau train dừng vì lỗi import trong bước kiểm tra adapter V2. Bản kiểm tra V3 đã sửa và qua trên trọng số thật; GGUF step64, R20, audio rộng và A30 vẫn chưa chạy. Các kết quả bên dưới là lịch sử nghiên cứu; xem tài liệu bàn giao để biết trạng thái mới nhất.

[![Rust validation](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/actions/workflows/rust.yml)

**Natural-language Orchestration, Reasoning & Action** — đọc “Nô-ra”, gọi “Nora ơi”.

NORA là trợ lý giọng nói tiếng Việt chạy nền trên Windows: nhận yêu cầu, thực hiện các công cụ được hỗ trợ trong quyền đã cấp, giữ goal qua checkpoint và báo tiến độ khi có điều đáng nói.

**LifeMate V2 đang phát triển; mục tiêu >90% chưa đạt.** [Kiến trúc và thí nghiệm V2](docs/research/lifemate-v2-architecture.md) ghi rõ bằng chứng và phần còn thiếu. R14 trên 12 mẫu phát triển dùng transcript chuẩn đạt **39,72% → 51,01%** khi đưa cùng ví dụ train lên trước yêu cầu hiện tại. Số mẫu đúng hoàn toàn giảm từ 3 xuống 2; lỗi hết lượt model giảm từ 1 xuống 0. Đây là chẩn đoán hạn 180 giây trên bộ đã dùng nhiều vòng; **cả hai vẫn 0% khi áp 30 giây trên lattice**. Cả 95 lời gọi thực và trạng thái đều replay đúng qua SDK. Chưa có điểm BTC, kiểm chứng A30 hoặc kết quả đủ để nghiệm thu. Các lượt audio 30 giây V2 gần nhất cũng đạt 0%/12; điểm 2,9810%/123 bên dưới thuộc baseline 2B trước đó.

Rust đã có bindings từ evidence trước đó, kiểm namespace tại nguồn, phản hồi outcome, truy hồi ví dụ và công cụ chuẩn bị dữ liệu train. Bộ đóng gói hội thoại giữ đủ nhãn và giảm khoảng 67,4% token lặp; dữ liệu binding có 652 tham số ID từ quan sát trước đó. Cả 3.377 prefix / 1.082 tác vụ khớp trong supervised replay **không dùng model**. R13 xác nhận E5 được tích hợp đúng nhưng giảm điểm từ 39,72% xuống 31,22%, nên giữ BM25. R15 đã chấm cặp Q3/Q4 cùng cấu hình 28 lớp GPU/8 lớp CPU: 16,67%/13,89%, mỗi arm 8/12 lỗi hoặc quá hạn; không chọn cấu hình này. R17 Qwen3.5-4B với cùng BM25 đạt 9,68%, đã bị loại; R18 FunReason-MT đạt 16,87% trong interface JSON hiện tại, cũng chưa vượt đối chứng. Lỗi trace mất kết quả cuối block khi chạm trần calls đã được sửa và qua 28 test workflow. Build Linux mới đã qua 149 test, không lỗi; export/native adapter đã kiểm tra riêng. Huấn luyện 4B đầy đủ và kiểm chứng audio/A30 vẫn chưa hoàn tất.

Lượt kiểm tra dung lượng huấn luyện trajectory V6 đã qua ba phép so sánh số học trên 4B, nhưng backward của mẫu đầy đủ 7.881 token bị dừng ở 494 MiB VRAM còn trống, dưới ngưỡng 512 MiB. Lượt này có **0 bước optimizer**, không có adapter được nghiệm thu; không có CPU OOM hoặc swap của tiến trình. V7 tiếp tục thất bại ở backward với yêu cầu cấp thêm 124 MiB bị allocator từ chối; vẫn 0 bước optimizer, không có CPU OOM/swap và đã dọn sạch tiến trình. V8 chia RMSNorm theo đoạn token đã qua so sánh loss/toàn bộ 504 gradient trên 4B và hoàn tất 2 update của chuỗi đầy đủ 7.881/5.581 token; VRAM trống thấp nhất 924 MiB, không nới giới hạn. Kiểm tra resume trên tiến trình 4B mới đã khớp chính xác adapter/optimizer/RNG; export F32 GGUF giữ nguyên 504 tensor và runtime Rust đã nạp/chạy adapter. Lượt SFT theo thứ tự cố định đã bắt đầu trên 64 mẫu đầu trong 1.082 train tasks, lưu checkpoint mỗi update. Chưa có điểm chất lượng từ adapter đã huấn luyện hoặc một full pass hoàn tất. Mốc này khác với các lượt huấn luyện 2B/V6 lịch sử bên dưới.

Lượt SFT dài đầu tiên đã dừng sau 6 checkpoint vì bộ ghi thống kê CUDA chạm trần 8.000 mẫu; không có CPU OOM/swap và VRAM trống thấp nhất khoảng 992 MiB. Đã kiểm tra checkpoint và resume trên lattice bằng các đoạn tối đa 2 tác vụ, giữ nguyên worker, phép tính, optimizer/RNG và giới hạn tài nguyên. Chuỗi qua bước 16 đã được kiểm chứng; bộ điều khiển đang chạy nối tiếp tới mốc 64, kiểm tra chuỗi checkpoint trước mỗi lần tiếp tục và dừng khi có lỗi. Chưa hoàn tất mốc 64 hoặc đo chất lượng adapter. Đối chiếu riêng template cho thấy cả 3.377 prefix train hiện có khớp giữa HF và GGUF trong Python Jinja; điều này chưa chứng nhận native tokenizer/logits hoặc chất lượng tác vụ.

Phép thử native cấu hình context 16K với LoRA F32 đã hoàn tất 4 request tổng hợp khi tắt CUDA Graphs và dùng prefill 128/32: VRAM trống thấp nhất 274 MiB, vượt ngưỡng 256 MiB. Ba cấu hình trước đã bị guard dừng và được giữ lại. Cấu hình mới được chọn cho cả đối chứng và adapter trong bài đo R20 chưa chạy; đây là kiểm tra tài nguyên riêng, chưa có ASR, điểm tác vụ hoặc benchmark A30, và không thay đổi mặc định production.

[Đối chiếu lattice và A30](docs/research/lifemate-lattice-vs-a30-2026-11-04.md): 12 mẫu R14 mất 39,49–92,86 giây, 99,98% thời gian trong bước gọi model. A30 có cơ sở phần cứng để nhanh hơn đáng kể, nhưng chưa đo NORA trên A30; timeout trên GTX 1650 không chứng minh timeout trên máy chấm. Public Submissions nhận JSONL, không cung cấp phiên chạy model A30. Chất lượng 51,01% khi nới thời gian vẫn cần cải thiện.

**Trạng thái: unsigned validation candidate, cập nhật 03/11/2026.** Desktop gồm 11 crates: tray/PTT tiếng Việt, Responses API (`gpt-6-luna`, `xhigh`), goal/checkpoint/effect ledger, broker app/folder và voice workers local trong Windows AppContainer. Crate thứ 12, `nora-lifemate`, là adapter thi offline đang kiểm chứng. Xem [phạm vi/cách chạy desktop](docs/production-integration.md), [validation desktop](docs/validation.md), [quyết định tích hợp](docs/adr/0006-integrated-rust-runtime.md) và [LifeMate](docs/lifemate-2026.md). Kết quả một máy không chứng nhận production trên mọi laptop.

## Một nhánh duy nhất

Toàn bộ mã, tài liệu, scripts và kiểm chứng được duy trì trực tiếp trên **`main`**. Các mốc foundation và desktop integration được giữ trong lịch sử commit. Ngày 05/11/2026 người dùng yêu cầu đưa model hiện tại lên GitHub để bàn giao, cho phép công bố bản nghiên cứu này trước ngưỡng 100%. Mục tiêu chất lượng 100% vẫn chưa đạt; bản bàn giao không phải bản production. CI chạy khi push lên `main`.

## Pipeline và khả năng hiện có

PTT → Silero VAD/resample → Whisper large-v3-turbo **Q5_0** → durable goal → **`gpt-6-luna` / `xhigh`** qua Responses API → typed tools/approval → receipt verification → VieNeu-TTS v3 Turbo **FP32** → local playback. WAV được chọn đi vào ASR sau bước kiểm tra/resample. Models chạy CPU; ASR dùng tối đa 8 threads. Một native ASR/TTS worker resident tại một thời điểm.

- Giữ **Ctrl+Shift+Space** để nói; có system tray, text input, WAV import và quiet mode. Startup không tự mở microphone hoặc gọi mạng.
- Goal có checkpoint, input journal và effect receipts; câu trả lời bổ sung vào đúng goal, kết quả có thể đọc lại sau restart.
- Đọc/liệt kê/tìm kiếm trong folder được cấp quyền, tạo note mới có preview/readback, calculator, giờ local, reminders, public research có duyệt và mở EXE đã đăng ký theo hash.
- Quyền read/create/cloud riêng; approval một lần gắn đúng proposal. Native ASR/TTS dùng AppContainer và Job Objects. Không có tool shell, xóa file, gửi tin nhắn hoặc điều khiển GUI bất kỳ.
- Chỉ đọc câu hỏi, yêu cầu xác nhận, trở ngại, phát hiện hoặc kết quả đáng biết; không đưa hidden reasoning, epoch hay heartbeat không đổi ra UI/audio.

## Tài nguyên đã đo

Một lượt **GUI → imported synthetic WAV → ASR → API → TTS** có kết quả sau:

| Chỉ số | Quan sát |
|---|---:|
| Tổng working set đỉnh trong NORA Job | 1.288.851.456 bytes |
| Tổng private commit đỉnh đồng thời | 1.689.206.784 bytes |
| Kernel Job peak commit | 1.702.887.424 bytes |
| GPU dedicated / shared được sample | 5.120.000 / 33.841.152 bytes |
| Gói local mới, kèm Cargo notice bundle | 1.203.779.356 bytes |

Ngân sách là **riêng NORA**: root Job cap 2.000.000.000 bytes commit, native child 1.700.000.000; không phải tổng RAM Windows. Model inference chạy CPU. Máy đo có khoảng 16 GB RAM và GPU 8 GB, nên chưa chứng nhận GPU vật lý 4 GB. SSD không tính developer toolchains/build caches; package tính đủ logical size kể cả hardlinks. [Receipt tài nguyên](docs/benchmarks/2026-11-01-integrated.json) và [receipt đóng gói mới](docs/supply-chain/validation.json) ghi rõ từng snapshot.

Fixture synthetic 1,68 giây: từ launch tới final text khoảng **16,2 giây**, software playback drain **25 giây**. Đây là một lượt OS cache không flush, chưa mở microphone và không chứng minh human audibility, WER hoặc realtime SLO. Live-mic corpus, target-device/clean-machine testing, signing và redistribution clearance vẫn là các release gates còn mở.

## Chế độ thi LifeMate offline

Desktop giữ **2 GB RAM + LLM API**. Bản thi dùng LLM local, SDK BTC nguyên bản và cấu hình đích **A30 / 64 GB RAM**; thực nghiệm hiện chạy trên lattice GTX1650. Chưa có kết quả A30 hoặc điểm public NORA. Mục tiêu 99%/100% chưa đạt.

| Kết quả đã kiểm chứng | Phạm vi / quyết định |
|---|---|
| Q4 compact-typed 8,33%; few-shot 11,67% | Cùng 30 development; nonempty-perfect 0/0, few-shot thêm 2 deadline failures; không promote |
| Q4 compact-typed 8,33%; alias 6,67% | Cùng 30 development, nonempty-perfect 0/0; không promote |
| Q3 + turbo 8,33%; Q3 + PhoWhisper 3,33% | Cùng 30 development; mỗi arm 1 failure, SDK replay khớp; Pho không đạt task gate |
| Q3 baseline 8,33%; clarification v30 5,00% | Cùng 30 development; nonempty-perfect 0/0, failure 1/1; không promote |
| PhoWhisper Q5 CER local 20,06% so với turbo 24,37% | ASR-only 30 samples/32 utterances; chưa phải task score |
| HF/GGUF 2B format/token parity 3.861/3.861 | Không inference hoặc chứng minh chất lượng/export numerical parity |
| V11 single-backward train 945/1.940 tokens qua guard | Mốc lịch sử: 2.413 tokens bị guard dừng; 0 optimizer ở V11 |
| V13 QLoRA một update thực trên full945 tokens, save/reload qua | All text linears, rank16, 16,8M trainables; chưa full training/quality gain |
| V14 QLoRA liên tục: 3 bước hoàn tất, dừng ở ngữ cảnh 2.655 token | GPUfree 530.579.456 bytes thấp hơn guard 512 MiB; không lưu adapter hoặc gọi là train đủ |
| V32, 123 validation: CPU output / GPU output đều **2,9810%** | SDK replay đủ hai arm; failures79/78, nonempty-perfect2/2; không promote GPU placement |
| V15/V16 chuyển frozen table sang CPU, quản lý cache | Giảm CUDA allocated970MiB, full1.940-token numeric control khớp; V16 qua6updates rồi guard dừng ở5.177tokens; chưa fullSFT |
| HF base: 114/200 câu hồi quy tổng hợp đúng, **57%** | 199unique inputs; raw logit ranking, không phải điểm LifeMate hoặc production safety certification |

Đã đọc và ghi chẩn đoán đủ123validation cards:45lượt lặp lời gọi đã thực thi,45lượt có provenance refusal và16lượt chưa có quyết định model. Đây là phân tích của Codex, không phải nhãn human hoặc điểm mới. Bốn HTTP400 có cùng-run server logs vượt cửa sổ6.144tokens. [Báo cáo lỗi và ưu tiên sửa](docs/development/lifemate-error-review-2026-11-03.md) tách lỗi ASR, chọn tác vụ/tham số, grounding, điều khiển vòng lặp và ngân sách. Không nới guard hoặc đổi metric để cải thiện điểm.

Best historical local mirror là **13,5556%/30** từ một profile Colab khác. Tập 30 đã dùng chọn cấu hình nên không phải held-out estimate. Technical tests và SDK replay không thay thế task accuracy. Clarification v30 chặn 8 đề xuất sai nhưng không tăng quality. Backend bỏ qua `uniqueItems`; bản sửa v31 dùng enum các danh sách không trùng cho schema nhỏ, giữ runtime guard. V31 qua 2 native probes tổng hợp; chưa có điểm tác vụ của v31.

Adapter kiểm input, chỉ đưa audio/context/history/catalog được cấp vào model, gọi validate/call trên SDK, rồi xuất call list và state hash thực. Mỗi mẫu có hạn 30 giây; failures không bị bỏ khỏi denominator. Gold/references dùng trong scoring/training riêng, không phải inference input. Scorer lịch sử từng deserialize cả holdout rows trước khi lọc; scorer selected-only mới giữ công thức metric và chỉ giải mã selected development. Không gọi điểm local là điểm BTC.

Xem [kiến trúc hiện có](docs/architecture.md), [LifeMate và receipts](docs/lifemate-2026.md), [thí nghiệm kiến trúc](docs/research/nora-architecture-experiments.md) và [quy trình tiếp tục](docs/development/project-review-2026-11-03.md). Dataset, weights, traces/predictions và gradients vẫn ở vùng riêng tư. Không commit/push trước gate 100% thực đã thỏa thuận.

## Đọc từ đây

| Tài liệu | Nội dung |
|---|---|
| [Desktop integration](docs/production-integration.md) | Bản chạy thật, setup assets, quyền, lệnh kiểm chứng và các giới hạn hiện tại |
| [Validation mới nhất](docs/validation.md) | Test, API thật, whole-app voice/resource receipts và các release gates còn mở |
| [SBOM và notice export](docs/supply-chain/README.md) | Graph build Windows, notice hashes, kiểm tra đóng gói và các khoảng trống giấy phép còn mở |
| [Rust foundation lịch sử](docs/rust-foundation.md) | Mốc 5 crates trước desktop integration; build/run, native dependencies và kiểm tra |
| [Số đo tích hợp](docs/benchmarks/2026-11-01-integrated.json) | Toàn Job trong synthetic GUI/ASR/API/TTS journey và giới hạn phép đo |
| [Số đo foundation lịch sử](docs/benchmarks/2026-11-01-windows.json) | Component RAM/timing và FP32 waveform comparison ở mốc cũ |
| [Workflow và repositories](docs/strategy/workflow-and-repositories.md) | Voice/goal/update loops, repos chính, ứng viên, phần tự xây và đường LifeMate |
| [Đánh giá Rust cho client nhẹ](docs/research/rust-client-feasibility.md) | Phần RAM/CPU/disk có thể tối ưu, giới hạn inference/UI, kế hoạch A/B và [ADR đề xuất](docs/adr/0005-rust-client-proposal.md) |
| [Kế hoạch novelty và production](docs/strategy/2026-10-31-plan.md) | NORA Giữ Ý, VNG problem fit, prior art đã loại, demo và quyết định cần kiểm chứng |
| [PRD mới](docs/strategy/nora-guarded-completion-prd.md) | Flow, quyền app/folder, baseline backlog, OA/order scope và chính sách updates |
| [Threat model](docs/strategy/nora-security-threat-model.md) | 24 nhóm rủi ro, controls, residual risks và tests cần xây |
| [Evaluation và roadmap](docs/strategy/nora-evaluation-plan.md) | Safe completion, matched baselines, audio/resource gates; [backlog/milestones](docs/strategy/nora-delivery-roadmap.md) |
| [Nguồn nghiên cứu novelty](docs/research/novelty-sources.md) | 70 records / 66 URL đã mở hoặc có trích xuất, độ tin cậy và giới hạn |
| [Báo cáo nghiên cứu](docs/report-vi.md) | Kết luận, kiến trúc, lựa chọn nền tảng và điểm chưa biết |
| [LifeMate 2026](docs/lifemate-2026.md) | Đề thi, hợp đồng đầu ra, mốc thời gian và giới hạn công khai |
| [Kiến trúc](docs/architecture.md) | Desktop, voice, goal runtime, adapters, memory |
| [Ngân sách client nhẹ](docs/lightweight-client.md) | Thiết kế ngân sách; số đo tích hợp mới nằm trong validation |
| [Chính sách cập nhật](docs/progress-policy.md) | Khi nào nói, hiện card hoặc giữ im lặng |
| [Quyền và bảo mật](docs/security-permissions.md) | App/folder grants, approval, isolation, prompt injection |
| [Lộ trình và release gates](docs/production-roadmap.md) | Đường từ reference core đến beta và production |
| [Nguồn và phương pháp](docs/research/methodology.md) | Phạm vi nghiên cứu, bằng chứng và hạn chế |
| [Source ledger](docs/research/sources.md) | Nguồn chính thức gắn với từng kết luận |
| [Ground truths](ground-truths.json) | Facts, observations, decisions và câu hỏi tách riêng |
| [Ghi chú Obsidian](obsidian/NORA%20-%20Home.md) | Bộ note Markdown có wikilinks |

## Build và chạy Rust desktop

Rust pin 1.99.1. Windows native build cần MSVC C++ tools, CMake/Ninja và PowerShell 7 đã cài. Export notice cần developer Python 3.11+; Python không được đóng vào runtime. Máy chạy cần Windows x64, CPU AVX2/F16C/FMA, driver OpenGL 3.3 và Microsoft Visual C++ x64 runtime. Default/core tests không cần model, microphone hoặc paid API.

```powershell
git clone https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks.git
cd NoSleepClub_zalo_hacks
git switch main

./scripts/Build-Rust.ps1 -Action Test
./scripts/Build-Rust.ps1 -Action Clippy
./scripts/Build-Rust.ps1 -Action Release
./scripts/Prepare-VoiceAssets.ps1 -Destination D:/nora-assets
python scripts/export_dependency_notices.py --output .artifacts/dependency-notices
./scripts/New-NoraPortable.ps1 -Executable ./target/x86_64-pc-windows-msvc/release/nora-app.exe -AssetSource D:/nora-assets -Destination D:/NORA -NoticesDirectory .artifacts/dependency-notices
# Điền D:/NORA/config/.env.local rồi chạy D:/NORA/Launch-Nora.cmd.
./target/x86_64-pc-windows-msvc/release/nora.exe replay examples/progress-events.jsonl
```

Chọn thư mục export/package mới; scripts không ghi đè bản đã tồn tại. File key giữ riêng ở `D:/NORA/config/.env.local`, hoặc truyền `-LlmConfig` để tham chiếu file đã có mà không sao chép nội dung key. Trong app, chọn folder/EXE ở mục quyền truy cập và duyệt proposal trước thao tác. Chưa bật wake word.

Core portable: `cargo test --locked --workspace`. [Hướng dẫn native inference](docs/rust-foundation.md) và [asset manifests](assets/manifests/README.md) chỉ rõ models/runtime cần cung cấp riêng. Không có Python interpreter trong Rust runtime; ONNX Runtime, whisper.cpp và SQLite vẫn là native dependencies.

Mốc desktop tích hợp ghi **188 Rust tests passed**, Clippy `-D warnings` và release build; journey dùng API thật và assets đã hash. Lần rà workspace mới, gồm LifeMate, có **287 default-feature tests pass, 3 ignored**; không thay thế native/live-voice verification. Cargo audit trên graph tích hợp ghi 0 vulnerability advisories và 4 unmaintained warnings trong snapshot đã lưu. Đây không phải security certification. Chi tiết phạm vi và các phép đo nằm trong [validation receipt](docs/validation.md) và [review mới](docs/development/project-review-2026-11-03.md).

Partial CycloneDX build SBOM ghi **357 packages**, export có **251 license/notice text blobs**, còn **18 metadata/packaged-text gaps**. Schema, hash/path/lock checks và CI Windows/Linux đã qua; chưa phải binary SBOM đầy đủ hoặc giấy phép phân phối mọi native/model/voice asset. [Inventory, bằng chứng và cách tái tạo](docs/supply-chain/README.md).

## Chạy Python reference core

Chỉ cần Python 3.11+; lõi dùng standard library, không có lệnh mạng hay quyền điều khiển máy tính.

```powershell
$env:PYTHONPATH = 'src'
python -m unittest discover -s tests -v
python -m nora_policy replay examples/progress-events.jsonl
```

CLI chỉ mô phỏng quyết định thông báo dưới dạng JSON; không phát giọng nói và không thực thi task. Reference core kiểm tra router, lọc event, quiet mode và chống báo lặp qua notification ledger. Nó không triển khai toàn bộ durable goal engine hoặc sandbox.

Reference validation: **21 tests passed** và replay 9 events. Python được giữ để đối chiếu behavior; không phải worker bắt buộc của hướng Rust.

## Hướng triển khai đã chọn

Windows-first: UI/tray → Rust supervisor/permission broker → Rust goal/audio runtime và typed tools. LLM/worker roles dùng Responses API; một orchestrator và các lượt nghiên cứu công khai tách biệt, có giới hạn chung. [ADR-0005](docs/adr/0005-rust-client-proposal.md) ghi quyết định nền tảng. Audio theo STT → agent → TTS; push-to-talk/hotkey, chưa bật wake word.

Bản desktop chọn native egui; không có browser thường trú, local LLM hoặc Python interpreter trong runtime desktop. State dùng Rust/SQLite, không có LangGraph/Temporal dependency. Không cần cloud hosting để truy cập app/folder local; LLM và public research desktop dùng API có thể tính phí. LifeMate dùng local LLM và Python SDK bridge riêng như mô tả phía trên.

Hướng khác biệt đang kiểm chứng: **NORA Giữ Ý**, giữ mục tiêu/tham số qua gián đoạn và không cho nội dung bên ngoài tự cấp quyền. Bản desktop đã có original-goal checkpoints, follow-up input journal, exact approvals, effect receipts và phục hồi an toàn. Chưa có bằng chứng algorithm novelty hoặc vượt baseline; OA/order và arbitrary GUI automation chưa tích hợp. Contest adapter đã chạy end-to-end trên lattice; chất lượng tác vụ và các release gates còn chưa đạt.

## Công khai và dữ liệu cuộc thi

Repo public chứa nghiên cứu, policy reference và Rust foundation độc lập. Dữ liệu thi đã tải riêng vào `.artifacts/` để kiểm chứng LifeMate. Không commit dataset, audio cá nhân, credentials, predictions/traces riêng tư, native binaries hoặc model weights. Foundation desktop trước đó được kiểm chứng bằng upstream assets và synthetic fixtures, không dùng dữ liệu cuộc thi.

[Thể lệ Zalo AI Challenge 2026](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf), mục 4 và 8, đặt điều kiện riêng cho dữ liệu và chia sẻ mã thi. Public GitHub repo tự nó không chứng minh đã đáp ứng yêu cầu công khai mã thi trên diễn đàn chính thức. Cần đọc quy định từng track trước khi thêm competition solution; xem [ghi chú LifeMate](docs/lifemate-2026.md).

## Giấy phép

Mã và tài liệu gốc: [MIT](LICENSE); phần VieNeu port giữ [Apache-2.0 và NOTICE](THIRD_PARTY_NOTICES.md). Giấy phép này không cấp quyền với dữ liệu cuộc thi, model weights, preset voices hay thương hiệu bên khác. NORA là codename chưa kiểm tra nhãn hiệu. Cảm hứng JARVIS/FRIDAY chỉ thuộc phong cách tương tác; không phân phối tài sản Iron Man/Marvel.

CLI audio đã nhận adapter LoRA có hash và cấu hình prefill; 149 kiểm thử Windows qua, 3 kiểm thử bỏ qua từ trước. Chưa chạy audio với checkpoint mới hoặc build Linux cho thay đổi này. Xem [phạm vi kiểm chứng](docs/development/lifemate-audio-adapter-cli-2026-11-04.md).
