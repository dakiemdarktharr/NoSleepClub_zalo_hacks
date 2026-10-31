---
title: NORA - Ground Truths
updated: 2026-10-31
tags: [nora, ground-truths, evidence]
status: provenance-aware
---

# Ground truths, decisions và unknowns

Ngày kiểm tra **31/10/2026**, Asia/Saigon. Nhãn từng record quyết định cách đọc: facts có nguồn khác với design decisions, observations, hypotheses và open questions. Trạng thái có thể đổi; kiểm tra lại theo cột recheck trong JSON gốc.

Liên quan: [[NORA - Home]] · [[NORA - Decisions]] · [[NORA - Open Questions]].

[Machine-readable record](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/ground-truths.json) · [Source ledger đầy đủ](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/sources.md)

| ID / loại | Kết luận | Nguồn / artifact |
|---|---|---|
| GT001 — `verified_external_fact` | LifeMate 2026 nhận audio tiếng Việt, context, conversation history và available app tools có JSON schemas. | [Z02](https://challenge.zalo.ai/portal/lifemate) |
| GT002 — `verified_external_fact` | LifeMate output có ordered tool_calls và state_hash lấy từ env.get_state_hash(); agent không được tự bịa state hash. | [Z02](https://challenge.zalo.ai/portal/lifemate) |
| GT003 — `verified_external_fact` | Portal mô tả sys.ask_missing_info khi thiếu thông tin, và [] khi không installed app nào xử lý được request. | [Z02](https://challenge.zalo.ai/portal/lifemate) |
| GT004 — `verified_external_fact` | Lịch công bố tại ngày kiểm tra ghi nộp solution 16/11/2026 23:59 GMT+7 và private results 23/11/2026 23:59 GMT+7. | [Z01](https://challenge.zalo.ai/#challenges), [Z04](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf) |
| GT005 — `verified_external_fact` | Thể lệ mục 4 giới hạn mục đích sử dụng/phân phối dữ liệu cuộc thi; quyền tham dự không tự cấp quyền dùng dữ liệu thương mại. | [Z04](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf) |
| GT006 — `verified_external_fact` | Thể lệ mục 8.2 có điều kiện về công khai mã thi trên diễn đàn chính thức trong thời gian cuộc thi, trừ ngoại lệ được cho phép rõ. | [Z04](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf) |
| GT007 — `verified_external_fact` | openWakeWord công bố code Apache 2.0 nhưng included pretrained models CC BY-NC-SA 4.0. | [V07](https://github.com/dscripka/openWakeWord) |
| GT008 — `verified_external_fact` | openWakeWord README nói hiện chỉ hỗ trợ English; Porcupine supported-language list đã đọc không liệt kê Vietnamese. | [V07](https://github.com/dscripka/openWakeWord), [V08](https://picovoice.ai/platform/porcupine/) |
| GT009 — `verified_external_fact` | MCP roots là informational; protocol không enforce server chỉ truy cập trong các roots. | [A13](https://modelcontextprotocol.io/specification/2026-07-28/client/roots) |
| GT010 — `verified_external_fact` | Tauri capabilities không tự sandbox Rust core hoặc sidecars. | [V01](https://v2.tauri.app/security/capabilities/), [V02](https://v2.tauri.app/develop/sidecar/) |
| GT011 — `verified_external_fact` | Checkpoint/replay không bảo đảm exactly-once external effects; action có thể bị lặp khi completion receipt thất lạc. | [A02](https://docs.langchain.com/oss/python/langgraph/interrupts), [A04](https://docs.temporal.io/activity-definition) |
| GT012 — `verified_external_fact` | Windows services không trực tiếp tương tác user từ Vista; UI trợ lý nên chạy trong user session. | [V04](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services) |
| GT013 — `verified_external_fact` | Windows UI Automation chịu integrity-level/security boundaries; UIAccess không phải shortcut chung để vượt UAC. | [V05](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview) |
| GT014 — `observed_local_state` | Lần đọc trang Zalo TTS trong nghiên cứu chỉ trả shell/cookie; chưa xác minh được API production/quota/giá. | [V15](https://developers.zalo.me/docs/api/ai-text-to-speech) |
| GT015 — `verified_external_fact` | Model Vietnamese Zipformer trong catalog được xếp offline; microphone/simulated streaming không xác nhận native streaming. | [V10](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/zipformer-transducer-models.html), [V16](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/) |
| GT016 — `design_decision` | Tên codename: NORA, Natural-language Orchestration, Reasoning & Action; đọc Nô-ra. | [artifact](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/report-vi.md) |
| GT017 — `design_decision` | Windows-first Tauri/Rust broker/Python worker; cascade voice; hotkey baseline; một orchestrator mặc định. | [artifact](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0001-windows-local-first.md) |
| GT018 — `design_decision` | Chỉ thông báo từ trusted events/evidence; giữ im lặng với technical logs/heartbeat không đổi. | [artifact](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/adr/0002-evidence-gated-updates.md) |
| GT019 — `observed_local_state` | Reference core hiện là router/notification simulation dùng Python standard library, chưa có microphone, LLM integration hoặc desktop automation. | [artifact](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/tree/main/src/nora_policy) |
| GT020 — `observed_local_state` | Nghiên cứu chưa chạy Vietnamese audio benchmark, official LifeMate evaluator hoặc production security audit. | [artifact](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/methodology.md) |
| GT021 — `hypothesis` | Tên Nora ơi có thể phù hợp để gọi bằng tiếng Việt, nhưng FAR/FRR và hiệu quả theo giọng chưa được kiểm nghiệm. | [artifact](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/voice-desktop.md) |
| GT022 — `open_question` | LifeMate có cho phép hosted APIs/Internet/external models nào và có runtime/hardware limit cụ thể gì? | [Z02](https://challenge.zalo.ai/portal/lifemate), [Z04](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf) |
| GT023 — `open_question` | Chọn persistent LangGraph saver/storage nào đủ phù hợp cho Windows production? SQLite saver được tài liệu mô tả cho development không tự chứng minh production support. | [A01](https://docs.langchain.com/oss/python/langgraph/durable-execution), [A16](https://docs.langchain.com/oss/python/langgraph/checkpointers) |
| GT024 — `verified_external_fact` | AppContainer có file/network/process/window isolation; Job Objects quản lý nhóm tiến trình và resource limits, cần security limits riêng cho process. | [P01](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation), [P02](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) |

Không biến model summary hoặc một demo thành sự thật đã được kiểm chứng. Các mục tiêu p95 latency, cỡ mẫu và model recommendations hiện là đề xuất thử nghiệm. Code/license của model, weights và training-data rights phải được audit riêng.
