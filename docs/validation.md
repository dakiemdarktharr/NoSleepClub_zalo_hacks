# Validation receipt — 01/11/2026

Phạm vi mới nhất: **bản desktop Rust tích hợp, API thật và synthetic voice journey**, unsigned validation candidate. [Phạm vi/cách chạy](production-integration.md), [ADR tích hợp](adr/0006-integrated-rust-runtime.md) và [receipt JSON](benchmarks/2026-11-01-integrated.json). Các phần foundation/reference bên dưới là lịch sử.

Bổ sung không cần build/voice test: [SBOM và notice packaging](supply-chain/README.md) cho graph build Windows, 357 packages, 251 text blobs và 18 metadata/packaged-text gaps. Schema và tamper/path/lock checks qua; EXE giữ nguyên SHA. Gói mới thêm notice bundle có 1.203.779.356 bytes, chưa được cấp trạng thái redistribution-cleared. Các số đo voice/RAM bên dưới giữ nguyên phạm vi và binary đã kiểm chứng.

## Desktop integration — kết quả mới nhất 01/11/2026

Windows x64 MSVC, Rust 1.99.1: **188 tests passed**, format, Clippy toàn workspace/all-targets/all-features với `-D warnings`, release build passed. Bốn script PowerShell parse thành công. Tests có mock Responses server, goal steering/recovery, exact one-use approval, revoke/stale sessions, cap-std containment và Windows junction, arithmetic/resource bounds, bounded UI queues/draft recovery, notice lifecycle, native model data paths và crash khi chờ duyệt. Đây không phải independent security audit.

Paid API thật dùng đúng **`gpt-6-luna`, `xhigh`**, không fallback hoặc tự retry:

| Hành trình | Kết quả | API calls / input / output tokens |
|---|---|---|
| Tính `(17*23)+11` bằng tool | 402; kết quả bền vững | 2 / 1.889 / 27 |
| Tạo note có exact preview/approval | Ghi mới, đọc lại đúng nội dung | 2 / 2.295 / 665 |
| Thiếu thời gian nhắc việc | Hỏi lại; hủy không tạo reminder | 1 / 920 / 68 |
| Public research → note có duyệt | Query đúng topic được duyệt; metadata nguồn thật; note ghi/readback | 4 / 27.490 / 2.660 |
| Câu trả lời giọng nói vào goal đang chờ | Giữ một goal; transcript đúng fixture; marker được consume | 2 / 1.803 / 85 |
| GUI → ASR → API → TTS | Goal completed, một surface và một speech ack | 1 / 844 / 62 |

Đóng/mở supervisor khôi phục kết quả 402 từ local store. Những API checks đầu tiên và lượt lỗi được giữ riêng, không tính là lượt qua. Lượt research ban đầu với trần 48.000 input tokens đã có note receipt nhưng bị từ chối request trả lời cuối do reservation không vừa. Cấu hình mẫu/local được đặt **96.000 input tokens/goal**; vẫn giữ 24 calls, 12.000 output tokens, timeout 60 giây, output mỗi response tối đa 2.048. Đây là thay đổi trần chi phí API; không tăng trần RAM. App không tự tăng budget hoặc giấu failure.

### Phép đo toàn ứng dụng

Máy đo: Windows 11 Home build 26200; Ryzen AI 5 340, 6 cores/12 threads; RAM khả dụng phần cứng khoảng 16 GB; RTX 5050 Laptop **8.151 MiB**, kèm Radeon 840M. Models chạy **CPU**. Đây không phải kiểm thử trên máy tổng RAM 2 GB hoặc GPU vật lý 4 GB.

| Chỉ số | Quan sát ở lượt GUI release cuối |
|---|---:|
| Tổng working set đỉnh của process set trong NORA Job | 1.288.851.456 bytes |
| Tổng private commit đỉnh đồng thời | 1.689.206.784 bytes |
| Kernel Job peak commit counter | 1.702.887.424 bytes |
| Số process đồng thời lớn nhất | 3 |
| GPU dedicated / shared peak được sample | 5.120.000 / 33.841.152 bytes |
| Package local gồm EXE, 16 assets, config template, notices | 1.202.320.731 bytes |
| Package + post-run synthetic state snapshot | 1.202.439.515 bytes |

RAM được sample 20 ms từ **Job PID set thực tế**, cộng đồng thời từng process, không cộng các lifetime peak rời nhau. Có 2.894 samples, 0 counter errors; bao gồm child native và các helper trong job. GPU có 53 samples, khoảng **1.154 ms/sample** do Windows counter, 0 errors, cộng các adapters. Sampling có thể bỏ sót peak ngắn. Root Job cap là **2.000.000.000 bytes commit**, child native cap **1.700.000.000**; đó không phải một working-set quota. External user applications đã được duyệt, Windows và máy chủ LLM có footprint riêng. NORA không spawn browser để loại khỏi phép đo.

WAV synthetic “Xin chào, tôi là Nora.” dài 1,68 giây, được tạo từ preset TTS; **không mở microphone**. Từ launch: goal admission/transcript khoảng **14.313 ms**, final text **16.226 ms**, software playback drain **25.015 ms**. ASR dùng tối đa 8 threads và build AVX2; mốc 2 threads trước đó cần khoảng 29 giây tới admission. Đây là hai lượt riêng trên OS cache không flush, không phải A/B có kiểm soát hoặc percentile SLO. Latency còn cao cho hội thoại tức thời; không gọi kết quả này là realtime chất lượng production. Drain không chứng minh người dùng nghe được âm thanh. Một completion speech ack, không còn ba lần narration cho lời chào. Lease/scratch còn lại sau thoát: 0. Framebuffer riêng 1120×760 đã xem, theme tiếng Việt đọc được; không chụp desktop hoặc cửa sổ key editor.

### Confinement, assets và dependency review

Release AppContainer probe thật qua: đọc scratch được phép, ngoài scope đọc/ghi bị OS error 5, môi trường test/API-key không lọt, localhost không kết nối được; parent positive file/network controls qua; profile/ACL cleanup qua. Network failure ở host này là `TimedOut`, không bịa socket error 10013. Probe RAM thật: job 64 MB từ chối allocation 96 MB; kernel peak có thể tính cả attempt bị từ chối, nên receipt ghi riêng actual process counters.

Asset setup xác minh **16 files, 1.177.282.839 bytes**, download thật lại pinned config/voice catalog/ORT archive và kiểm hash. Tensors lớn được hardlink từ bản đã verify cho setup roundtrip. Portable packaging đã chạy; `--doctor` xác minh cả VAD/ASR/TTS và LLM config mà không gọi mạng. Template key trống; package local tham chiếu file key riêng, không sao chép key. Không có weights/DLL/recordings/API keys trong GitHub source commit. Hardlink vẫn tính đủ logical bytes; developer toolchains/build caches không nằm trong runtime package. Disk là snapshot/bounded setup, chưa phải filesystem quota.

Cargo audit0.22.2: **0 vulnerability advisories, 4 unmaintained warnings**: `paste 1.0.15`, `zbus_names 4.3.5`, `zvariant 5.15.1`, `zvariant_derive 5.15.1`. Không suppress warnings. RustSec DB `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, 1.395 advisories, snapshot 30/10/2026. Lock SHA-256 `f1421061e55ee5ae4400be148da9cce12096781c25dd197223fc211f69795d73`. Không bao phủ đủ native DLL/models/Git-source vulnerabilities hoặc cấp quyền redistribution.

### Những gate còn mở

Chưa chứng nhận **production-ready**: chưa có representative Vietnamese live-mic corpus/WER/critical-slot accuracy, AEC/far-field/barge-in, human audibility, physical 4 GB GPU/target-laptop matrix, clean-machine/missing-runtime recovery, long soak/exhaustive crash injection, real third-party app launch workflow, full third-party/voice/data redistribution clearance, signed installer/update. Bản hiện tại mở EXE đã đăng ký và duyệt, không điều khiển GUI bất kỳ. Chưa có Zalo OA/OMS integration hoặc official contest submission adapter. “Giữ Ý” chưa có kết quả vượt matched baseline. Intent/claim guards là heuristics có receipts, không phải semantic proof.

CI Windows/Linux của commit `79e3d1` đã qua; commit đó có trước các sửa SIMD/sandbox/UI cuối. Kết quả CI của source mới được dẫn qua GitHub Actions/PR, không suy ra từ kết quả local.

## Lịch sử Rust foundation — kiểm tra 01/11/2026

- Windows x64 MSVC, Rust 1.99.1: `Build-Rust.ps1 -Action Test` với workspace all features: **84 tests passed** — core 27, audio 19, store 13, CLI 9, VieNeu 16. Không mở microphone, gọi paid API hoặc tải weights trong tests.
- `Build-Rust.ps1 -Action Clippy`: workspace/all-targets/all-features, `-D warnings` passed. `Format` passed; `Release` build passed. GNU check có giới hạn native Whisper linking được ghi trong crate README; MSVC là Windows native path đã xác minh.
- Rust/Python replay 9 synthetic events bằng nhau ở JSON values, gồm simulation marker. Python reference **21 tests passed**. Tests cho bounded parsing/resources, receipt identity, stale revisions/cancel, unknown effects qua reopen/recovery, foreign DB không bị nhận nhầm, scoped note write/readback/no-overwrite, PCM/resampler và TTS host data/control paths.
- Model thật: Silero ONNX CPU xử lý 32 frame silence; reset recurrent state đã kiểm tra. VieNeu FP32 + native sea-g2p sinh 80.640 float32 samples/48 kHz, preset Hải Đăng, câu “Xin chào, tôi là Nora.”, greedy, EOS 21 frames. Không phát audio hoặc đo chất lượng bằng người nghe.
- Binary cuối: TTS process peak working set **1.160.454.144 bytes**, peak sampled OS commit **1.179.000.832 bytes**; load 4.517,742 ms; first chunk 407,6691 ms sau load; tổng synthesis 1.120,2911 ms. Fresh process, OS cache không flush, intra2/inter1, CPU backend. Chỉ một process TTS+G2P, không gồm ASR/UI/browser/LLM/children/GPU. Không coi đây là kết luận NORA đạt 2 GB.
- Reference comparison dùng cùng graphs/ORT 1.31.1 và source blobs đã kiểm hash. Binary cuối khớp length và waveform trong case này (max abs/RMSE 0); lần trước max abs 1,79e-6/RMSE 9,69e-8. Không kiểm G2P độc lập; dùng Rust phonemes và bỏ denoiser initialization không dùng ở preset. Đây là numerical check của một phrase, không phải quality parity/feature parity toàn SDK hoặc chứng minh tốc độ Rust/Python.
- Cargo audit 0.22.2 sau upgrade ZIP 9.0.0: exit 0, **0 vulnerability advisories**, **1 unmaintained warning** `paste 1.0.15` qua `tokenizers 0.22.2` (`RUSTSEC-2024-0436`). Không suppress warning. RustSec DB commit `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, 1.395 advisories; Cargo.lock SHA-256 `62b8e94c168080c8bb663ece718d2941e8bb1e37ea28cdf1a52e42439f0424b4`. Native DLL/model/Git source vulnerabilities không được audit này bao phủ đầy đủ.
- CI workflow được thêm cho Linux core và Windows native; kết quả local không tự là kết quả GitHub Actions. Asset manifests chỉ ghi hashes/sizes/revisions, không commit model/runtime binaries.

Chưa kiểm chứng: ASR với model thật/Vietnamese corpus, microphone-to-speaker loop, AEC/far-field, LLM task completion, app APIs, OS broker/sandbox/IPC, full Giữ Ý security, crash injection tại external effect thật, signed installer/update, 4 GB GPU backend và whole-product 2 GB RAM/15 GB disk. SQLite recovery tests chỉ mô phỏng persisted interrupted records; không tạo external exactly-once guarantee. `journal_size_limit` không hard-cap active WAL.

## Lịch sử kiểm tra research/reference — 31/10/2026

### Đã kiểm tra ở vòng research

- Python 3.11: **21 tests passed** bằng `python -m unittest discover -s tests -v` với `PYTHONPATH=src`.
- Synthetic CLI replay: 9 events; technical/epoch/heartbeat im lặng; duplicate source IDs được loại; completion chưa verified bị chặn; permission xuất action card.
- CLI tiếng Việt chạy được khi redirected Windows stdout mặc định cp1252; CLI xuất UTF-8.
- Notification acknowledgements theo từng channel và goal; SQLite marker được đọc lại sau đóng/mở database. Test này không mô phỏng việc speaker đã phát rồi crash trước ack.
- Các file JSON parse được; không có local Markdown target bị thiếu trong bộ kiểm tra liên kết tương đối.
- Source ledger nền: **42 URL duy nhất** ở vòng nghiên cứu ban đầu; trang shell/draft/historical/partial được đánh dấu. Ground-truth records đã tăng từ 24 lên **43** ở vòng lập kế hoạch novelty, vẫn tách facts/decisions/hypotheses/unknowns.

### Kiểm tra tài liệu kế hoạch novelty

- Ba source ledgers mới parse JSON hợp lệ: 70 records, 66 URL sau chuẩn hóa slash cuối; không coi chúng là 66 nghiên cứu độc lập hoặc full papers. [Source audit](research/novelty-sources.md).
- Các ID nguồn được dẫn trong 43 ground-truth records đều phân giải tới ledgers; không có GT ID trùng.
- Bộ Markdown kế hoạch/research mới, README và notes mới không có target tương đối bị thiếu; fences sơ đồ cân bằng.
- Có phản biện riêng về product/prior art và security/evaluation. Đã sửa demo có conflict trong workflow, phân biệt OA với OMS, giới hạn adapter scope, model-request egress, trial reset, harness dependency và memory counters.
- Phiên này chỉ thay Markdown/JSON nghiên cứu/thiết kế; không sửa policy implementation/tests, không tải models/datasets, không chạy voice/security/production benchmark. Kết quả 21 tests ở trên thuộc reference core hiện hữu, không chứng minh các khả năng được đề xuất đã triển khai.
- Hai nhánh nghiên cứu đã fact-check phần kiến trúc/voice/durability; không tìm thấy overclaim trọng yếu. Đây là review tài liệu, không phải independent security audit.

### Giới hạn của vòng research ban đầu

Chưa cài `jsonschema` trong môi trường validation; JSON schemas được kiểm tra cú pháp JSON và đọc thủ công, chưa chạy một Draft 2020-12 validator đầy đủ. Reference CLI dùng contract fixture đơn giản hơn production event schema; schema validation không xác thực producer hoặc cấp quyền.

Chưa có: microphone capture/STT/TTS benchmark, desktop installer, app connector, OS sandbox/IPC tests, LLM task routing evaluation, LangGraph/Temporal integration, actual crash/effect reconciliation, official LifeMate evaluator, production load/cost/security/privacy checks.

Tests thành công chỉ chứng minh các behavior fixtures hiện tại. Các SLO, cỡ mẫu và model lựa chọn là kế hoạch thử nghiệm. Xem [production gates](production-roadmap.md).
