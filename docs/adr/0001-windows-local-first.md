# ADR-0001: Windows-first với broker và worker local

Status: **Accepted as research direction, implementation pending**. Date: 2026-10-31.

## Context

Sản phẩm cần microphone, chạy nền trong user session, quyền folder/app cụ thể và khả năng hoàn tất goal. Greenfield, đội nhỏ, chưa benchmark voice hoặc xác minh devkit. LifeMate chấm simulator actions; desktop thật cần OS permissions khác.

## Decision

Chọn hướng Tauri 2 UI/tray, Rust broker và Python worker. Audio cascade, hotkey trước wake word; một orchestrator, worker khi độc lập. Prototype LangGraph durability; local persistent goal/event store đánh giá SQLite, chưa coi development saver là production. Broker enforce tất cả actions; API-first, UI Automation fallback.

## Alternatives

Electron thuận tiện nếu đội mạnh TS và chấp nhận cập nhật Chromium/Node. Pure web không đủ lifecycle/quyền local. Windows Service không làm UI/mic chính. Temporal thêm vận hành chưa cần cho một thiết bị; đánh giá lại khi backend nhiều worker/ngày. Agent loop tự xây hẹp hợp demo nhưng tăng gánh nặng recovery.

## Consequences

Cần duy trì Rust/Python/TypeScript và packaging sidecar. Tauri capabilities không sandbox core; phải thiết kế containment/IPC riêng. Chưa có bằng chứng framework này nhẹ/nhanh hơn trên cấu hình mục tiêu. Quyết định có thể đổi sau vertical slice, không buộc giữ stack nếu deployment hoặc recovery không đạt gate.

Sources: [Tauri](https://v2.tauri.app/security/capabilities/), [Windows](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services), [LangGraph](https://docs.langchain.com/oss/python/langgraph/checkpointers).
