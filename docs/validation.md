# Validation receipt — 01/11/2026

Phạm vi hiện tại: **Rust foundation, local native probes và offline reference policies**, không phải voice/app end-to-end acceptance. [Implementation scope](rust-foundation.md) và [measurement receipt](benchmarks/2026-11-01-windows.json).

## Rust foundation — kiểm tra 01/11/2026

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
