# ADR-0002: Cập nhật từ events và bằng chứng

Status: **Accepted as policy direction; reference subset implemented**. Date: 2026-10-31.

## Context

User muốn biết agent nào đã chạy, nguồn và blocker, nhưng không muốn log epoch/skill init. Stream raw reasoning không bảo đảm thông tin hữu ích hoặc đúng trạng thái.

## Decision

Trusted runtime/verifier phát events; deterministic gate chọn timeline/action card/audio. LLM có thể diễn đạt sau gate, không tự quyết định claim đã verified. Dedupe và per-channel delivery acknowledgement phải persist. User speech và quiet preferences điều khiển timing; status question có đường riêng.

## Consequences

Cần outbox/dispatcher và stale-event reconciliation, không chỉ prompt. Lõi reference hiện chưa có production queue, global audio arbitration hay producer authentication. Delivery ack có crash window; không hứa exactly-once audio. Xem [policy](../progress-policy.md) và [reference tests](../../tests/test_policies.py).
