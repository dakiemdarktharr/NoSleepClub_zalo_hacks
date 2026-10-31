# NORA · NoSleepClub

**Natural-language Orchestration, Reasoning & Action** — đọc “Nô-ra”, gọi “Nora ơi”.

NORA là định hướng sản phẩm trợ lý giọng nói tiếng Việt chạy nền trên máy tính: nhận một yêu cầu, chọn cách thực thi phù hợp, làm việc trong quyền được cấp và báo tiến độ khi có điều đáng nói.

**Trạng thái: research + reference policy core, ngày 31/10/2026. Chưa phải ứng dụng production.** Repo chưa có microphone pipeline, desktop installer, LLM integration, app automation hoặc bộ sinh submission LifeMate. Các con số hiệu năng trong tài liệu là mục tiêu thử nghiệm, trừ khi ghi rõ là kết quả đo.

## Đọc từ đây

| Tài liệu | Nội dung |
|---|---|
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
| [Ngân sách client nhẹ](docs/lightweight-client.md) | Mục tiêu 2 GB RAM, GPU 4 GB, SSD 15 GB; chưa benchmark |
| [Chính sách cập nhật](docs/progress-policy.md) | Khi nào nói, hiện card hoặc giữ im lặng |
| [Quyền và bảo mật](docs/security-permissions.md) | App/folder grants, approval, isolation, prompt injection |
| [Lộ trình và release gates](docs/production-roadmap.md) | Đường từ reference core đến beta và production |
| [Nguồn và phương pháp](docs/research/methodology.md) | Phạm vi nghiên cứu, bằng chứng và hạn chế |
| [Source ledger](docs/research/sources.md) | Nguồn chính thức gắn với từng kết luận |
| [Ground truths](ground-truths.json) | Facts, observations, decisions và câu hỏi tách riêng |
| [Ghi chú Obsidian](obsidian/NORA%20-%20Home.md) | Bộ note Markdown có wikilinks |

## Chạy reference core

Chỉ cần Python 3.11+; lõi dùng standard library, không có lệnh mạng hay quyền điều khiển máy tính.

```powershell
$env:PYTHONPATH = 'src'
python -m unittest discover -s tests -v
python -m nora_policy replay examples/progress-events.jsonl
```

CLI chỉ mô phỏng quyết định thông báo dưới dạng JSON; không phát giọng nói và không thực thi task. Reference core kiểm tra router, lọc event, quiet mode và chống báo lặp qua notification ledger. Nó không triển khai toàn bộ durable goal engine hoặc sandbox.

Validation ngày 31/10/2026: **21 tests passed** và replay 9 events. Xem [phạm vi và giới hạn kiểm tra](docs/validation.md).

## Hướng triển khai đã chọn

Windows-first: Tauri 2 UI/tray → Rust permission broker → Python worker có persistent LangGraph checkpoints → typed app adapters. Audio theo STT → agent → TTS trước; push-to-talk/hotkey trước, wake word tự huấn luyện và audit giấy phép sau. Một orchestrator mặc định; worker song song chỉ xuất hiện khi các nhánh độc lập đáng chi phí.

Đây là quyết định thiết kế cần benchmark, không phải bằng chứng Tauri, LangGraph hay một model giọng nói luôn vượt các lựa chọn khác. Không cần cloud hosting để truy cập app/folder local.

Hướng khác biệt đang kiểm chứng: **NORA Giữ Ý**, giữ mục tiêu/tham số và tiếp tục phần hợp lệ khi nội dung bên ngoài có xung đột hoặc chỉ dẫn độc hại. Đây là giả thuyết sản phẩm/workflow, chưa có algorithm novelty hoặc kết quả vượt baseline. Alpha chỉ gồm voice/text, scoped file/note, research và synthetic merchant workflow; OA + nguồn order là pilot riêng, calendar/email/browser có release gates riêng. Nghiên cứu mới chỉ bổ sung Markdown/source ledgers, chưa triển khai các khả năng này.

## Công khai và dữ liệu cuộc thi

Repo public này chỉ có tài liệu gốc và policy reference độc lập. Không commit dataset, audio cá nhân, credentials, private-test outputs hoặc trọng số có quyền phân phối chưa rõ. Chưa tải hay sử dụng dữ liệu cuộc thi để viết reference core này.

[Thể lệ Zalo AI Challenge 2026](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf), mục 4 và 8, đặt điều kiện riêng cho dữ liệu và chia sẻ mã thi. Public GitHub repo tự nó không chứng minh đã đáp ứng yêu cầu công khai mã thi trên diễn đàn chính thức. Cần đọc quy định từng track trước khi thêm competition solution; xem [ghi chú LifeMate](docs/lifemate-2026.md).

## Giấy phép

Mã và tài liệu gốc trong repo: [MIT](LICENSE). Giấy phép này không cấp quyền với tài liệu tham khảo, dữ liệu cuộc thi, model weights hay thương hiệu của bên khác. NORA là codename; chưa kiểm tra khả năng đăng ký nhãn hiệu. Cảm hứng JARVIS/FRIDAY chỉ thuộc phong cách tương tác; không phân phối tài sản Iron Man/Marvel.
