---
title: NORA - Open Questions
updated: 2026-10-31
tags: [nora, research, validation]
status: unresolved
---

# Những điều còn phải kiểm chứng

Không coi danh sách này là đã hoàn tất. Kết quả mới cần cập nhật [[NORA - Ground Truths]] kèm evidence và ngày.

## Ưu tiên cho LifeMate

- Lấy devkit/evaluator hợp lệ sau bước đăng ký/điều khoản của người dùng. Xác định tool catalog, exact metric và submission contract đầy đủ.
- Xác minh hosted APIs, Internet, model/data bên ngoài, hardware/time/memory limits và cách freeze/reproduce.
- Kiểm tra điều kiện chia sẻ mã thi tại mục 8.2 trước thêm competition implementation vào repo public. Không phân phối dataset/raw audio/private outputs.
- Dựng audio → tool simulator → final-state verifier vertical slice rồi phân tích lỗi slots/dependencies/missing-info.

## Ưu tiên cho desktop alpha

- Benchmark ASR/TTS theo Bắc/Trung/Nam, noise, code-switching và critical slots trên thiết bị mục tiêu.
- Chốt model TTS có quyền thương mại, không suy API Zalo đã dùng được từ một URL shell.
- Spike Tauri/Python packaging, authenticated IPC, OS containment và folder/app grants thực.
- Chọn persistent LangGraph storage/saver phù hợp; test crash sau effect trước receipt, disk full/locking và restart.
- Hoàn thiện outbox/coalescing/stale-event checks/global audio arbitration; reference policy chưa có các phần này.

## Trước production

- Signed installers/updates, migration/rollback, secret rotation, retention/delete/export và privacy review.
- Security tests: prompt injection, cross-goal grants, path/junction swaps, revoked/stale approvals, cloud egress.
- Đo task success, useful latency, cost/task, unauthorized effects và độ hữu ích của updates; không chỉ WER hoặc demo đẹp.
- Wake word “Nora ơi”: data consent/provenance, license, FAR/FRR và phân phối theo giọng. Chưa có chứng minh.
- Kiểm tra nhãn hiệu NORA và quyền đối với tài sản/giọng nhân vật nếu branding thay đổi.

Mục tiêu SLO và cỡ mẫu hiện là đề xuất, chưa có phép đo. Xem [roadmap và gates](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/production-roadmap.md).
