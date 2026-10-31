# NORA delivery roadmap và điều kiện production

**Kế hoạch ngày 31/10/2026, Asia/Saigon; chưa thực thi.** Các owners dưới đây là vai trò cần phân công. Lịch giả định một người phụ trách chính có trợ giúp AI và có quyền/thiết bị test đúng lúc; đây là dự toán scope, không cam kết hoàn thành mọi integration trước cuộc thi.

## 1. Đường đi và thứ tự ưu tiên

Có hai đầu ra riêng: **LifeMate candidate** tuân official environment/evaluator; **desktop product** có quyền máy thật, audio, privacy, recovery và release operations. Dùng chung intent/tool-policy concepts khi phù hợp, không ép simulator semantics lên production hoặc ngược lại. Novelty là nhánh thử nghiệm sau baseline, không được làm hỏng format/utility của bài thi.

Lịch công bố đã kiểm tra ghi solution deadline **16/11/2026 23:59 GMT+7**, private test 20/11 và private result deadline 23/11 23:59. Từ 31/10 đến 16/11 là 16 ngày lịch; kiểm tra lại trước nộp. [Portal](https://challenge.zalo.ai/portal/lifemate), [thể lệ chính thức](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf), [ghi chú track](../lifemate-2026.md).

## 2. Milestones đến candidate

| Mốc dự kiến | Owner | Việc và dependency | Exit gate / bằng chứng |
|---|---|---|---|
| M0 — 31/10 | Product/research | Chốt phương án, prior art, threat model, scope và test plan | Bộ tài liệu này; chưa có implementation |
| M1 — 01–02/11 | Competition lead + product | Official devkit/version/metrics, terms/data rights, Internet/API/model/hardware rules; adapter matrix và discovery protocol | Rule/access checklist có nguồn; phân loại mã thi/private assets; không suy hosted API được phép |
| M2 — 02–04/11 | Runtime + audio lead | Vertical slice PTT→ASR→1 safe tool→readback→TTS; baseline simulator→tool calls/state hash; đo cold RAM sớm | Reproducible benign baseline, spike trace, danh sách CPU/backend; chưa mở rộng nhiều agents |
| M3 — 04–07/11 | Runtime + security | Goal state/effect ledger, grants, typed broker, failure/revoke/retry; synthetic order source và authority policy | Crash/replay/unknown-outcome tests; no hidden side effects; B0/B1 có score |
| M4 — 07–10/11 | Security + product | Giữ Ý B2/B3, conflict trong workflow, positive control thay đổi hợp lệ, useful updates | Matched pilot/ablation; no-go nếu block-all hoặc không có incremental value |
| M5 — 10–13/11 | Evaluation + QA | Audio end-to-end, noise/accents, attack/recovery, resource/cost checks, official evaluator | Versioned report, failure analysis, không gọi pilot score thành production proof |
| M6 — 13–15/11 | Release + competition | Freeze model/dependency/config, reproduce clean environment, validate submission contract; inspect current deadline | Repro artifacts/checksums/data rights, no private assets in public repo |
| M7 — 16/11 | Competition owner | Nộp qua official channel khi user/đội đã đủ điều kiện và cung cấp quyền | Receipt từ hệ thống thật; không báo đã nộp chỉ vì tạo file |

Các mốc có thể overlap nhưng dependencies giữ thứ tự. Nếu M1/M2 chậm, cắt capsule, broad connectors và multi-worker trước; giữ baseline correctness, API/rules compliance và safe execution. Candidate thi có thể không phải desktop beta; không gắn nhãn GA vì đến deadline.

Chưa đăng ký/đồng ý điều khoản/tải dataset/submit ở bước này. Code sản xuất kết quả thi có điều kiện công khai riêng theo thể lệ mục 8.2; public GitHub một mình không đủ. Khi triển khai, phần hạn chế giữ private, generic docs/policy public; việc công bố trên diễn đàn cần đúng quy trình và quyền gửi thông tin, không tự đăng thay user trong phiên lập kế hoạch.

## 3. Các cổng quyết định phải xử lý sớm

| Decision | Kiểm tra cần làm | Hướng xử lý khi không đạt |
|---|---|---|
| Competition có cho hosted LLM/audio APIs? | Track rules/devkit/official confirmation nếu tài liệu thiếu | Dùng model/runtime theo môi trường BTC cho phép hoặc đổi architecture competition; không lén gửi data ra cloud |
| Local voice nằm trong 2 GB với chất lượng đủ tốt? | Full-tree cold/overlap bench Q8/Q5/TTS profiles | Tối ưu lifecycle, thử model/profile khác; cloud chỉ khi user chọn; không claim low-RAM nếu fail |
| Merchant thật có pain chưa được OA Chatbot giải quyết? | 5–10 discovery sessions dự kiến, không liên hệ tự động hiện tại | Thu hẹp/đổi wedge; giữ generic assistant nếu nhu cầu merchant không đủ |
| Có quyền OA và nguồn order/policy? | Admin authorization, app scopes, OMS/CSV rights và customer mapping | Synthetic demo ghi rõ; không gọi là production integration hoặc scrape personal chats |
| Giữ Ý hơn baseline mạnh? | B1/B3 matched comparisons và genuine-change controls | Bỏ novelty claim không tạo giá trị; giữ controls hữu ích cho product quality |
| TCB/containment dùng được với Windows/audio/UI? | Compatibility/security spike theo process | Thu hẹp supported operations; không coi Job Objects là sandbox thay thế |

Quota 2 GB là target client của sản phẩm. Không mặc định competition server cũng có đúng giới hạn đó; ngược lại, máy inference thi mạnh không chứng minh client laptop đạt target. Nếu cuộc thi bắt local LLM không phù hợp budget, tách competition compute profile và giải thích rõ.

## 4. Backlog triển khai theo dependency

| ID / ưu tiên | Outcome | Dependency | Definition of done |
|---|---|---|---|
| D01 P0 | Hợp đồng tool/provider/data được khóa | Official access/rules | Versioned schemas/terms, permissions và reproducibility plan |
| D02 P0 | Native audio profile nhỏ | CPU/driver/model rights | PTT/cancel, bounded capture, cold/warm traces, native/ONNX pack không cần end-user PyTorch toolchain |
| D03 P0 | Broker và local IPC | Threat model/OS spike | Schema/tool allowlist, peer/session auth, grants/revoke, no unauthenticated control port |
| D04 P0 | Durable goal/effect state | D03, store decision | Atomic transitions, locks/versioning, idempotency/reconciliation, crash/restart tests |
| D05 P0 | File/note adapter | D03/D04 | Scoped handles, path containment, conflict-safe writes/readback, no silent overwrite |
| D06 P0 | Competition baseline adapter | D01 | Official tool calls/order/hash/missing info/unsupported exact contract |
| D07 P0 | API LLM orchestration | D01/D03 | Structured proposals, bounded context/spend, one orchestrator, workers no independent grants/credentials |
| D08 P0 | Giữ Ý policy/evidence | D04/D05/D07 | Field authority/provenance, destination checks, safe continuation and UNKNOWN handling |
| D09 P1 | Trustworthy updates và TTS | D02/D04 | Runtime events→policy→voice/card, anti-duplicate, barge-in, no CoT/epoch/skill-init chatter |
| D10 P1 | Merchant synthetic/CSV workflow | D05/D08, declared authority | Same clean/attack/genuine-change demo, source/customer IDs traceable, partial status đúng |
| D11 P1 | OA connector/BFF pilot | OA/admin/app rights + D03/D04 | Server secret storage, account isolation, PKCE/current auth contract, single-flight token rotation, messaging eligibility |
| D12 P1 | OMS hoặc declared export source | Data owner/rights + D11 | Canonical actor/order mapping, read/write scopes tách, policies versioned; no update claim on read-only data |
| D13 P1 | Calendar/email adapter riêng | Account scopes/terms | Scoped operations, sends/recipients/duplicates/revoke có tests, updated support matrix |
| D14 P2 | Browser/UI automation | Process/OS/origin controls | Observable targets/effects, versioned supported actions, resource gate; manual handoff ngoài support |
| D15 P2 | Support capsule | D08 + usefulness/privacy study | Preview/redact/local export, no auto-report, reviewer evidence of value |
| D16a P0 — M2 | Minimum eval/resource harness | D01 và vertical slice D02/D03 | Clean initial-state reset, schema/state verifier, basic audio/resource/cost traces; B0/B1 đo được trước novelty |
| D16b P0 — M3–M5 | Mở rộng harness theo feature | D16a; lần lượt D04–D10 | Failure/attack/ablation suites đi cùng mỗi feature; matched baselines, CI/limitations report |
| D17 P0 trước beta | Packaging/update/privacy | D02–D09, signing plan | Signed installer/update, SBOM/notices, rollback/recovery/delete/grant management |
| D18 P0 trước GA | Operational readiness | D11–D17 theo scope | Human pilot, independent review, incident/credential/runbook drills, monitored staged rollout |

P0 của candidate và P0 của product beta không hoàn thành cùng lúc mặc định. D11–D15 không được chặn official baseline khi thiếu partner credentials. Public research repo không chứa data/API keys hay dependencies không được phân phối.

## 5. Kiến trúc cần chốt sau spike

Giữ logical architecture: Windows tray/UI → typed permission/effect broker → scoped audio/orchestrator/adapters → independent verification/event policy. Tauri/Rust là hướng đã chọn; Python/LangGraph một process là ứng viên orchestration, chưa chốt footprint. Native model workers chỉ thêm khi cần isolation/lifecycle, không một process cho mỗi agent role.

Mọi request tới LLM/router/critic/summarizer/worker đều là data egress: broker kiểm tra labels, destination, purpose và consent trước serialization/transmission. Workers không nhận key trong context hoặc tự tạo socket/tool ngoài scope. Nếu Python stack không qua memory/startup gate, giảm dependency hoặc chuyển phần orchestration gọn sang Rust; đo chi phí chuyển đổi thay vì mặc định framework nổi tiếng sẽ nhẹ.

Durable store đề xuất local SQLite cho app state, một writer/transactions và versioned migrations. Saver LangGraph cụ thể phải spike/check docs vì dev saver không tự thành production engine. Không hứa exactly-once từ checkpoint. BFF OA là dịch vụ nhỏ khi shared app cần giữ secret/webhook/token; không cần microservices/Kubernetes/vector DB cho desktop một người dùng. Secrets tách state/log; storage encryption theo threat model thực tế.

## 6. Release scope và khoảng thời gian sau candidate

| Release | Scope cụ thể | Evidence cần đủ | Thời gian dự toán sau khi bắt đầu đúng scope |
|---|---|---|---|
| Alpha nội bộ | Voice/text + grant file/note + research + synthetic merchant workflow | D02–D10, core crash/safety/resource traces, explicit unsupported list | Khoảng 1–2 tuần kỹ thuật; không cộng thành lời hứa thi |
| Closed beta hẹp | Windows support matrix, 1 OA + 1 order source với partner đã đồng ý | D11/D12, 5–10 discovery sessions và cohort pilot nhỏ; signed build/privacy/recovery | Khoảng 2–4 tuần sau alpha và quyền tích hợp |
| RC cho scope đã khóa | Freeze supported adapters, no Critical/High open, full eval và independent review | Resource/latency/cost/utility gates, real update rollback, deletion/incident drills | Khoảng 2–4 tuần sau beta ổn định; phụ thuộc review |
| GA có giới hạn | Chỉ hardware/adapters/operations đã được nghiệm thu | Pilot duration đủ, staged rollout, monitoring/on-call/kill switch, release notes | Theo evidence và external approval; không chốt ngày khi chưa có số đo |

Các khoảng là planning estimates chưa có velocity. Không cộng “4 tuần” rồi coi mọi máy/app đã production-ready. Calendar/email/browser ra sau nếu chưa qua gates; tính năng UI phải phản ánh đúng adapter hiện có.

## 7. Chi phí production: trả cho cái gì

| Hạng mục | Bắt buộc hay tùy chọn | Cách kiểm soát |
|---|---|---|
| LLM API | Cần cho hướng hybrid đã chấp nhận; provider/model chưa chọn | BYOK hoặc service key ở backend; per-goal request/token/spend caps; matched quality/cost eval |
| Nhiều workers/critic | Tùy goal; dùng thêm API không tự miễn phí | Một orchestrator default, bounded concurrency/context, không spawn nếu không có output độc lập |
| STT/TTS local | Không có phí API mỗi lượt nếu license/artifacts phù hợp | Disk/runtime/update/support vẫn có cost; license code/weights/data kiểm tra riêng |
| Speech cloud/search | Tùy chọn; phát sinh theo provider/plan | Consent, quota, bounded retries, không tự chuyển khi local fail |
| OA messaging/plan | Theo eligibility/plan/API hiện hành | Check trước effect và hiển thị cost khi cần quyết định; không mặc định free không giới hạn |
| BFF/token broker OA | Cần cho shared secret theo auth contract hiện tại | Service nhỏ, auth/account isolation, storage/monitoring/backup; chọn hosting sau sizing |
| Signing/distribution/operations | Cần cho quy trình phát hành đáng tin; hình thức/giá tùy kênh | Dự toán signing identity, bandwidth, logs, incident/support, update storage |

Không đưa bảng giá model/API chưa được chọn thành sự thật. Công thức lập ngân sách mỗi goal: input tokens × giá input + output tokens × giá output + tool/search/voice units + retry/worker overhead; cộng allocation backend/distribution/support ở cost sản phẩm. Kiểm tra bảng giá chính thức theo ngày trước khi chọn provider. User thấy estimated cap và actual usage phù hợp, không thấy các thuật ngữ hạ tầng không giúp họ quyết định.

Giới hạn 15 GB disk gồm runtime/models, cache/log, update staging và rollback. Một bản runtime đang chạy + bản update có thể cần gần gấp đôi phần binary; tính trước, không vượt cap rồi bỏ recovery. Máy phát triển có toolchains/test artifacts không đại diện cho bộ cài end user.

## 8. Definition of production-ready cho scope công bố

Một release cần đồng thời đáp ứng:

- **Utility:** supported tasks đạt mục tiêu eval với CI/failure disclosure; missing-info/unsupported/genuine-change xử lý đúng; không chỉ security demo.
- **Voice/UX:** audio intelligibility/critical slots, PTT/barge-in/cancel, trạng thái mic, quiet mode, accessibility và updates có evidence.
- **Safety/security:** broker boundaries được test, no known critical unauthorized effect ở gate suite, Critical/High findings đã đóng, account/credential/egress controls và independent review phù hợp scope.
- **Reliability:** crash/restart, duplicated events, provider unknown outcome, offline, sleep/resume, revoke, migration và concurrent edits có kết quả đúng.
- **Resource:** toàn cây process/peak/cold/overlap/update qua budget trên hardware matrix; không chỉ đo steady-state một PID.
- **Privacy/data rights:** consent/minimization/retention/delete/export, license/terms/notices, quyền dataset/model và provider data handling.
- **Delivery/ops:** signed artifacts/updates, tamper checks, SBOM/pinned versions, staged rollout/rollback/kill switch, runbooks và người chịu trách nhiệm vận hành.

Demo chạy, 21 policy unit tests hiện có hoặc một lần benchmark không thay được các gates này. [Evaluation protocol](nora-evaluation-plan.md), [threat model](nora-security-threat-model.md).

## 9. Manifest cho bước “oneshot” sau

Một prompt tiếp theo có thể yêu cầu thực thi backlog theo các gates, ghi evidence và xử lý phần độc lập liên tục. Đầu vào cần có hoặc phải kiểm tra ở M1: official devkit/terms; supported Windows/CPU/GPU; phạm vi folder/app; LLM account/quota/data policy; OA/OMS partner nếu đi pilot; signing/distribution identity; người dùng/audio đồng thuận cho validation.

Implementation được thực hiện theo vertical slices có kiểm chứng; quyền và thông tin chưa có thì chỉ chặn nhánh liên quan. Không thay credentials thật bằng mock rồi báo integration xong, không coi simulator là banking thật, không dựng chứng nhận GA. Khi external approval/human validation chưa sẵn, kết quả có thể là tested candidate cùng danh sách gates còn thiếu, không được đổi tên thành production-ready.

Ở phiên hiện tại, các mục trên chỉ được lập kế hoạch. Không tạo backend/service, tải model/data, thay đổi app/security settings, gửi message/report hay chạy workflow thật.
