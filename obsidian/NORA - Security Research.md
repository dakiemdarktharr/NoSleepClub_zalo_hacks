---
project: NORA
type: security-plan
updated: 2026-10-31
status: not-a-security-audit
tags: [nora, security, threat-model]
---

# NORA — Security research

Đã tự tìm các từ khóa PHP/Hugging Face/Korean banking, đối chiếu nguồn gốc. Không quét hay khai thác hệ thống, không sao chép PoC. Các case không chứng minh VNG bị cùng lỗ hổng.

| Case | Điều được xác minh | Hệ quả |
|---|---|---|
| PHP10/2026 | Official archive có21/10 fixes,23/10 follow-up và26/10 Windows dependency rebuild | Patch/runtime/parser isolation; named fixed versions không đồng nghĩa sạch mọi advisory |
| HF07/2026, follow-up26/08 | Internal evaluation có safeguards khác deployment; báo cáo bổ sung có inter-agent channels và spoofing concerns | Least privilege, no shared credentials, broker-owned receipts, no scorer access, bounded goals/egress |
| FSC14/10/2026 | 16 tổ chức tài chính/~100k records; AI/common attacker vẫn điều tra | Attribution thận trọng; patch/access/monitoring, không chỉ classifier |

Nguồn: [PHP](https://www.php.net/archive/2026.php), [OpenAI26/08](https://openai.com/index/hugging-face-incident-and-the-road-ahead/), [METR/Redwood](https://evals.alignment.org/hugging-face-incident-report-aug-2026.pdf), [FSC](https://www.fsc.go.kr/eng/pr010101/87928). [Bản đối chiếu chi tiết](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/research/novelty-incidents.md).

## Boundary quan trọng

Model/worker chỉ đề xuất. Broker giữ quyền/effect/egress/receipt; native UI thể hiện canonical target và diff. Data ngoài không đổi goal/grants; summary/memory phải giữ provenance. Từng model API request cũng cần disclosure policy trước khi gửi. Không có general guarantee cho arbitrary GUI app; chỉ typed/observable adapters đã test.

Threat model có 24 nhóm: voice replay, ASR slots, goal parser, injection/summary laundering, worker scope, approval TOCTOU, OAuth/BFF/token races, webhook replay, SSRF/egress, path/GUI confusion, duplicate effects, native/model supply chain, forged audit, PII export, OOM/disk, concurrent edits, cost persistence, provider drift, IPC và mistaken reports.

Bảo đảm chỉ trong trust model: nếu OS/broker/user account đã bị compromise hoàn toàn thì không thể tự bảo vệ mọi dữ liệu. Local append-only audit không chống admin; signature/hash không chứng minh nội dung đúng hoặc dependency không có bug. Chưa formal verification/independent audit.

## Gate trước production

No open Critical/High findings trong scope; tests cho revoke/crash/unknown outcome/canary egress; data rights/privacy/delete; signed update/rollback; full-tree RAM; human pilot; independent review cho external effects. Zero observed attacks trong sample không bằng risk0. Readback/evaluator độc lập, không để worker tự ghi “success”.

Tài liệu: [Threat model](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-security-threat-model.md) · [Evaluation](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/strategy/nora-evaluation-plan.md) · [[NORA - Novelty and Production Plan]] · [[NORA - Novelty Ground Truths]].
