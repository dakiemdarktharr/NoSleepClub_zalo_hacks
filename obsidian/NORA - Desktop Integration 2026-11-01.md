---
title: NORA - Desktop Integration 2026-11-01
updated: 2026-11-01
tags: [nora, rust, validation, production, hardware]
status: unsigned-validation-candidate
---

# NORA desktop integration

[[NORA - Home]] · [[NORA - Ground Truths]] · [[NORA - Resource Budget]]

Bản chạy Rust 11 crates đã có native egui/tray/PTT, goal/checkpoint/result store, scoped broker và local audio AppContainer workers. LLM/worker roles gọi đúng `gpt-6-luna`, `xhigh`; không có Python runtime, browser thường trú hoặc local LLM.

Luồng: PTT hoặc WAV được chọn → Silero/resample → Whisper large-v3-turbo Q5 CPU → durable goal → Responses API → typed tools/exact approvals → receipt/readback → text/update policy → VieNeu v3 Turbo FP32 CPU → speaker. Một native model child tại một thời điểm. Startup không mở mic/gọi mạng.

**188 Rust tests**, format, Clippy `-D warnings`, release build qua. API thật qua calculator402, approved exact note/readback, missing-input cancellation, approved public research→note, restart result recovery và spoken follow-up giữ cùng một goal. GUI synthetic journey qua, chỉ có một completion speech acknowledgement. Epoch/technical logs im lặng; câu hỏi/xác nhận có thể đọc; stale questions/previews hủy phần audio chưa phát. Kênh UI/audio được ghi riêng, không fake ack.

Lượt GUI release cuối: working set peak **1.288.851.456 bytes**, private commit peak đồng thời **1.689.206.784**, kernel Job peak **1.702.887.424**. GPU dedicated sampled **5.120.000**, shared **33.841.152 bytes**. Package local **1.202.320.731 bytes**, 16 verified assets **1.177.282.839 bytes**. RAM sampler20ms/0 errors; GPU khoảng1.154ms/sample/0 errors. Models chạy CPU. Máy đo16GB RAM, RTX5050Laptop8.151MiB; chưa test GPU vật lý4GB. Ngân sách là riêng NORA, không phải tổng Windows RAM.

Fixture synthetic1,68s “Xin chào, tôi là Nora.”: admission sau14,3s từ launch, text16,2s, playback drain25,0s. Đây là một lượt OS cache không flush; chưa đạt chứng nhận realtime conversation hoặc WER/voice quality. Không mở mic, không đo human audibility. Native leases/scratch sau thoát0.

Root Job cap2.000.000.000 bytes commit, native child1.700.000.000. AppContainer thật qua allowed scratch/outside read-write denial/environment filtering/network negative controls với parent positive controls; profile/ACL cleanup qua.64MB job từ chối96MB allocation. Không phải proof mọi exploit bị chặn hoặc filesystem quota.

Budget API mẫu/local:96.000 input tokens,12.000 output tokens,24 calls/goal; có thể tính phí. Lượt48k ban đầu dừng đúng budget trước final response dù note receipt đã có; không tính là pass. Không có automatic retry/budget increase trong app. Key chỉ ở file local, không commit hoặc sao chép vào package; portable config có thể tham chiếu file riêng.

Cargo audit snapshot30/10:0 vulnerability advisories,4 unmaintained warnings (`paste`, `zbus_names`, `zvariant`, `zvariant_derive`). Không chứng nhận native/models. Chưa đủ full redistribution notices/voice-data rights review.

Còn gate: live Vietnamese corpus/mic, AEC/far-field, physical4GB GPU/target-device matrix, clean-machine, real third-party app launch journey, soak/crash matrix, signing/secure updates và exact-build license bundle. Hiện hỗ trợ scoped read/search/create-new notes, calculator/time/reminders, public research và approved registered EXE launch; chưa arbitrary GUI automation, OA/OMS hoặc contest adapter. Gọi đây là **unsigned validation candidate**, chưa gắn nhãn production-ready.

[Validation](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/validation.md) · [Machine-readable receipt](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/benchmarks/2026-11-01-integrated.json) · [Cách build/chạy](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/production-integration.md)
