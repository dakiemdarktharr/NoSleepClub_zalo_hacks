# NORA · NoSleepClub

**Natural-language Orchestration, Reasoning & Action** — đọc “Nô-ra”, gọi “Nora ơi”.

NORA là trợ lý giọng nói tiếng Việt chạy nền trên Windows: nhận yêu cầu, thực hiện các công cụ được hỗ trợ trong quyền đã cấp, giữ goal qua checkpoint và báo tiến độ khi có điều đáng nói.

**Trạng thái: unsigned validation candidate, ngày 01/11/2026.** Có 11 crates: desktop/tray/PTT tiếng Việt, Responses API (`gpt-6-luna`, `xhigh`), goal/checkpoint/effect ledger, broker app/folder và voice workers local trong Windows AppContainer. Xem [phạm vi/cách chạy](docs/production-integration.md), [kết quả kiểm chứng mới nhất](docs/validation.md) và [quyết định tích hợp](docs/adr/0006-integrated-rust-runtime.md). Mốc foundation bên dưới là lịch sử; kết quả một máy không chứng nhận production trên mọi laptop.

## Đọc từ đây

| Tài liệu | Nội dung |
|---|---|
| [Desktop integration](docs/production-integration.md) | Bản chạy thật, setup assets, quyền, lệnh kiểm chứng và các giới hạn hiện tại |
| [Validation mới nhất](docs/validation.md) | Test, API thật, whole-app voice/resource receipts và các release gates còn mở |
| [SBOM và notice export](docs/supply-chain/README.md) | Graph build Windows, notice hashes, kiểm tra đóng gói và các khoảng trống giấy phép còn mở |
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
| [Ngân sách client nhẹ](docs/lightweight-client.md) | Thiết kế ngân sách; số đo tích hợp mới nằm trong validation |
| [Chính sách cập nhật](docs/progress-policy.md) | Khi nào nói, hiện card hoặc giữ im lặng |
| [Quyền và bảo mật](docs/security-permissions.md) | App/folder grants, approval, isolation, prompt injection |
| [Lộ trình và release gates](docs/production-roadmap.md) | Đường từ reference core đến beta và production |
| [Nguồn và phương pháp](docs/research/methodology.md) | Phạm vi nghiên cứu, bằng chứng và hạn chế |
| [Source ledger](docs/research/sources.md) | Nguồn chính thức gắn với từng kết luận |
| [Ground truths](ground-truths.json) | Facts, observations, decisions và câu hỏi tách riêng |
| [Ghi chú Obsidian](obsidian/NORA%20-%20Home.md) | Bộ note Markdown có wikilinks |

## Build và chạy Rust desktop

Rust pin 1.99.1. Windows native build cần MSVC C++ tools, CMake/Ninja và PowerShell 7 đã cài. Default/core tests không cần model, microphone hoặc paid API.

```powershell
./scripts/Build-Rust.ps1 -Action Test
./scripts/Build-Rust.ps1 -Action Clippy
./scripts/Build-Rust.ps1 -Action Release
./scripts/Prepare-VoiceAssets.ps1 -Destination D:/nora-assets
./scripts/New-NoraPortable.ps1 -Executable ./target/x86_64-pc-windows-msvc/release/nora-app.exe -AssetSource D:/nora-assets -Destination D:/NORA
# Điền D:/NORA/config/.env.local rồi chạy D:/NORA/Launch-Nora.cmd.
./target/x86_64-pc-windows-msvc/release/nora.exe replay examples/progress-events.jsonl
```

Core portable: `cargo test --locked --workspace`. [Hướng dẫn native inference](docs/rust-foundation.md) và [asset manifests](assets/manifests/README.md) chỉ rõ models/runtime cần cung cấp riêng. Không có Python interpreter trong Rust runtime; ONNX Runtime, whisper.cpp và SQLite vẫn là native dependencies.

Validation hiện tại: **188 Rust tests passed**, Clippy `-D warnings` và release build; kiểm chứng tích hợp dùng API thật và assets đã hash. Cargo audit trên graph tích hợp ghi 0 vulnerability advisories và 4 unmaintained warnings trong snapshot đã lưu. Đây không phải security certification. Chi tiết phạm vi và các phép đo nằm trong [validation receipt](docs/validation.md).

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

Bản hiện tại chọn native egui; không có browser thường trú, local LLM hoặc Python interpreter trong runtime. State dùng Rust/SQLite, không có LangGraph/Temporal dependency. Không cần cloud hosting để truy cập app/folder local; LLM và public research dùng API có thể tính phí.

Hướng khác biệt đang kiểm chứng: **NORA Giữ Ý**, giữ mục tiêu/tham số qua gián đoạn và không cho nội dung bên ngoài tự cấp quyền. Bản desktop đã có original-goal checkpoints, follow-up input journal, exact approvals, effect receipts và phục hồi an toàn. Chưa có bằng chứng algorithm novelty hoặc vượt baseline; OA/order, arbitrary GUI automation và official contest submission adapter vẫn chưa tích hợp.

## Công khai và dữ liệu cuộc thi

Repo public chứa nghiên cứu, policy reference và Rust foundation độc lập. Không commit dataset, audio cá nhân, credentials, private-test outputs, native binaries hoặc model weights. Real-model validation dùng upstream assets local; chưa tải hay sử dụng dữ liệu cuộc thi cho foundation này.

[Thể lệ Zalo AI Challenge 2026](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf), mục 4 và 8, đặt điều kiện riêng cho dữ liệu và chia sẻ mã thi. Public GitHub repo tự nó không chứng minh đã đáp ứng yêu cầu công khai mã thi trên diễn đàn chính thức. Cần đọc quy định từng track trước khi thêm competition solution; xem [ghi chú LifeMate](docs/lifemate-2026.md).

## Giấy phép

Mã và tài liệu gốc: [MIT](LICENSE); phần VieNeu port giữ [Apache-2.0 và NOTICE](THIRD_PARTY_NOTICES.md). Giấy phép này không cấp quyền với dữ liệu cuộc thi, model weights, preset voices hay thương hiệu bên khác. NORA là codename chưa kiểm tra nhãn hiệu. Cảm hứng JARVIS/FRIDAY chỉ thuộc phong cách tương tác; không phân phối tài sản Iron Man/Marvel.
