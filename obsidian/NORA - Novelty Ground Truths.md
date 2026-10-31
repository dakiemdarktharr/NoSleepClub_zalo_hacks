---
project: NORA
type: grounded-research
updated: 2026-10-31
status: evidence-and-hypotheses-separated
tags: [nora, ground-truths, novelty]
---

# NORA — Ground truths bổ sung

Nguồn chuẩn máy đọc: [ground-truths.json — 43 records](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/ground-truths.json). Bảng dưới tóm tắt phần nghiên cứu novelty; không biến các decisions/hypotheses thành facts.

| Phân loại | Nội dung |
|---|---|
| Fact từ tài liệu nhà cung cấp | Zalo có Safety Center/OA Chatbot/Kiki, Zalopay có SIMO pilot. Không chứng minh efficacy hoặc NORA có API access |
| Fact API | OA admin cấp app/OA scopes; PKCE vẫn có app secret cho exchange/refresh. Không có quyền arbitrary personal inbox |
| Design consequence | Shared app secret nằm ở BFF; refresh single-flight/atomic rotation. Chưa có backend chạy |
| Research observation | CaMeL/Fides/Task Shield/RTBAS overlap với goal/provenance/selective approvals; không có algorithm novelty đã xác lập |
| Fact về published scope | Đã có Vietnamese tool-call benchmarks. Audio + multi-turn recovery + attack/benign controls + resource protocol chỉ là gap hypothesis sau đối chiếu |
| Fact từ regulator, có mốc thời gian | FSC14/10/2026 xác nhận các breach nhưng vẫn điều tra AI/common attacker; không được kể thành AI tự hack bank đã xác nhận |
| Fact từ incident reports | HF07/2026 trong internal eval; báo cáo08/2026 có unauthorized communication/spoofing concerns và scope limits |
| Decision | Ưu tiên Giữ Ý cho workflow hẹp; alpha file/note/research/synthetic, OA/order pilot sau |
| Hypothesis | Giữ Ý cải thiện safe completion/user burden so với critic+confirm cùng model/tools |
| Hypothesis | Support capsule tăng completeness/giảm effort; chưa có số liệu ROI/ticket reduction |
| Unknown | Merchant demand, VNG support bottleneck, OA/OMS access, competition API rules, 2 GB local voice fit, production evidence |
| Observed local state | Repo có policy reference và docs; chưa có voice desktop app/OA integration/security benchmark hoặc GA |

Source audit mới có **70 records / 66 URL trích xuất**, không phải 66 nghiên cứu độc lập hoặc 66 full papers. Source count không phải search result count. [Ledgers/giới hạn](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/novelty-sources.md).

Chỉ cập nhật confidence khi có evidence mới: API contract/version, actual readback, matched eval, hardware traces hoặc authorized pilot. Không dùng lời model “đã xong” như ground truth.

Liên kết: [[NORA - Ground Truths]] · [[NORA - Novelty and Production Plan]] · [[NORA - Open Questions]].
