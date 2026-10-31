# NORA · NoSleepClub

**Natural-language Orchestration, Reasoning & Action** — đọc “Nô-ra”, gọi “Nora ơi”.

NORA là định hướng sản phẩm trợ lý giọng nói tiếng Việt chạy nền trên máy tính: nhận một yêu cầu, chọn cách thực thi phù hợp, làm việc trong quyền được cấp và báo tiến độ khi có điều đáng nói.

**Trạng thái: Rust foundation, ngày 01/11/2026. Chưa phải ứng dụng production.** Đã có 5 crates cho policy/resources, audio/native inference, VieNeu Rust host, SQLite state/effects và CLI có scoped file write/readback. Silero và VieNeu FP32 + native G2P đã chạy model thật. Chưa có voice loop hoàn chỉnh, desktop installer, LLM integration, OS permission broker hoặc bộ sinh submission LifeMate. Mốc 2 GB RAM cho toàn bộ NORA vẫn cần kiểm chứng.

## Đọc từ đây

| Tài liệu | Nội dung |
|---|---|
| [Rust foundation đã xây](docs/rust-foundation.md) | Phạm vi 5 crates, build/run, native dependencies, tối ưu, kiểm tra và phần còn thiếu |
| [Số đo thực tế](docs/benchmarks/2026-11-01-windows.json) | Component RAM/timing, FP32 waveform comparison và giới hạn phép đo |
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
| [Ngân sách client nhẹ](docs/lightweight-client.md) | Mục tiêu 2 GB RAM, GPU 4 GB, SSD 15 GB; chưa có benchmark toàn sản phẩm |
| [Chính sách cập nhật](docs/progress-policy.md) | Khi nào nói, hiện card hoặc giữ im lặng |
| [Quyền và bảo mật](docs/security-permissions.md) | App/folder grants, approval, isolation, prompt injection |
| [Lộ trình và release gates](docs/production-roadmap.md) | Đường từ reference core đến beta và production |
| [Nguồn và phương pháp](docs/research/methodology.md) | Phạm vi nghiên cứu, bằng chứng và hạn chế |
| [Source ledger](docs/research/sources.md) | Nguồn chính thức gắn với từng kết luận |
| [Ground truths](ground-truths.json) | Facts, observations, decisions và câu hỏi tách riêng |
| [Ghi chú Obsidian](obsidian/NORA%20-%20Home.md) | Bộ note Markdown có wikilinks |

## Build Rust foundation

Rust pin 1.99.1. Windows native build cần MSVC C++ tools, CMake/Ninja và PowerShell 7 đã cài. Default/core tests không cần model, microphone hoặc paid API.

```powershell
./scripts/Build-Rust.ps1 -Action Test
./scripts/Build-Rust.ps1 -Action Clippy
./scripts/Build-Rust.ps1 -Action Release
./target/x86_64-pc-windows-msvc/release/nora.exe replay examples/progress-events.jsonl
```

Core portable: `cargo test --locked --workspace`. [Hướng dẫn native inference](docs/rust-foundation.md) và [asset manifests](assets/manifests/README.md) chỉ rõ models/runtime cần cung cấp riêng. Không có Python interpreter trong Rust runtime; ONNX Runtime, whisper.cpp và SQLite vẫn là native dependencies.

Validation local: **84 Rust tests passed**, format/Clippy passed; 9 replay events khớp Python reference. Cargo audit sau cập nhật ZIP 9.0.0: 0 vulnerability advisories theo RustSec snapshot đã ghi, giữ 1 warning unmaintained qua tokenizer. Đây không phải security certification hoặc kiểm thử toàn sản phẩm. [Validation receipt](docs/validation.md).

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

Windows-first: UI/tray → Rust supervisor/permission broker → Rust goal/audio runtime và typed app adapters. LLM/worker roles dùng API; chưa tích hợp provider. [ADR-0005](docs/adr/0005-rust-client-proposal.md) chấp nhận hướng Rust cho foundation, thay Python resident workers trong đề xuất ban đầu. Audio theo STT → agent → TTS; push-to-talk/hotkey trước, wake word sau khi kiểm thử tiếng Việt và giấy phép. Một orchestrator mặc định; worker song song chỉ khi nhánh độc lập đáng chi phí.

Tauri UI vẫn là ứng viên cần benchmark với native UI. State primitives hiện dùng Rust/SQLite; không có LangGraph/Temporal runtime dependency. Không cần cloud hosting để truy cập app/folder local.

Hướng khác biệt đang kiểm chứng: **NORA Giữ Ý**, giữ mục tiêu/tham số và tiếp tục phần hợp lệ khi nội dung bên ngoài có xung đột hoặc chỉ dẫn độc hại. Đây là giả thuyết sản phẩm/workflow, chưa có algorithm novelty hoặc kết quả vượt baseline. Alpha scope: voice/text, scoped file/note, research và synthetic merchant workflow. CLI note đã có write/readback thật; full Giữ Ý, OA/order và các adapters khác vẫn chưa tích hợp.

## Công khai và dữ liệu cuộc thi

Repo public chứa nghiên cứu, policy reference và Rust foundation độc lập. Không commit dataset, audio cá nhân, credentials, private-test outputs, native binaries hoặc model weights. Real-model validation dùng upstream assets local; chưa tải hay sử dụng dữ liệu cuộc thi cho foundation này.

[Thể lệ Zalo AI Challenge 2026](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf), mục 4 và 8, đặt điều kiện riêng cho dữ liệu và chia sẻ mã thi. Public GitHub repo tự nó không chứng minh đã đáp ứng yêu cầu công khai mã thi trên diễn đàn chính thức. Cần đọc quy định từng track trước khi thêm competition solution; xem [ghi chú LifeMate](docs/lifemate-2026.md).

## Giấy phép

Mã và tài liệu gốc: [MIT](LICENSE); phần VieNeu port giữ [Apache-2.0 và NOTICE](THIRD_PARTY_NOTICES.md). Giấy phép này không cấp quyền với dữ liệu cuộc thi, model weights, preset voices hay thương hiệu bên khác. NORA là codename chưa kiểm tra nhãn hiệu. Cảm hứng JARVIS/FRIDAY chỉ thuộc phong cách tương tác; không phân phối tài sản Iron Man/Marvel.
