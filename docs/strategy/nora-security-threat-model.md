# Threat model và thiết kế bảo mật NORA

**31/10/2026 — Design v0.1.** Đây là phân tích rủi ro dự kiến, không phải báo cáo tìm thấy lỗ hổng trong một bản NORA đã chạy. Chưa pentest, chưa audit độc lập. Mức Critical/High/Medium là ưu tiên xử lý theo impact và khả năng tiếp cận bề mặt, không phải CVSS được đo. Không có PoC, khai thác hay quét hệ thống bên ngoài trong công việc này.

## 1. Tài sản và phạm vi tin cậy

Tài sản cần bảo vệ: ý định/quyền của user; app/folder contents; âm thanh/transcript; danh tính người nhận và dữ liệu đơn hàng; API/OAuth credentials; external effects; audit/effect ledger; model/runtime artifacts; availability, thời gian và API spend.

Attacker có thể kiểm soát một message, attachment, website, tool result, tên file, screenshot hoặc audio user yêu cầu xử lý. Có thể có tài khoản khách hàng hợp lệ, gửi nhiều lượt và cố rửa chỉ dẫn qua worker/memory. Một dependency/model loader cũng có thể bị compromise; đây là lớp rủi ro khác prompt injection. Model/worker có thể sai, bị điều hướng hoặc tự đưa ra hành động vượt phạm vi dù không có attacker.

Trusted computing base dự kiến: broker và policy implementation, native approval UI, OS/session identity, secret store, adapter code, provider authentication, audit writer và release/update verifier. Nếu broker, OS hoặc tài khoản user đã bị chiếm hoàn toàn, NORA không thể tự bảo đảm confidentiality/integrity. Admin của máy có thể sửa local logs; “append-only” trong thiết kế không có nghĩa chống được local administrator.

## 2. Trust boundaries

| Boundary | Dữ liệu đi qua | Enforcement dự kiến | Giới hạn cần nghiệm thu |
|---|---|---|---|
| Phòng/mic → audio session | Tiếng user, TV, người khác, replay | PTT/session indicator, bounded buffer, stop/cancel, xác nhận critical fields | Voice/wake word không chứng minh ai đang nói |
| Audio process → goal parser | Transcript/alternatives, timestamps | Typed interface, length/time limits, modality/source labels | Confidence của ASR không phải xác suất an toàn |
| App/web/file → planner/worker | Nội dung có thể chứa instruction độc | Data/control separation, provenance và field authority | Không thể chỉ “sanitize text” để hết mọi injection |
| Worker/LLM API → broker | Action proposals | Tool allowlist, schema, grants, entity/destination policy, current goal/version | JSON hợp lệ không đồng nghĩa hành động hợp lệ |
| User UI → approval/grant | Quyết định có quyền | Trusted native rendering, canonical target/fields, exact action binding | User vẫn có thể approve nhầm hoặc bị social engineering |
| Broker → adapters/OS | File handle, account, typed operation | Least privilege, scoped credentials, capability checks trước effect | Một adapter có quyền quá rộng là confused deputy |
| Desktop → cloud LLM/BFF | Context/token/tool traffic | Broker kiểm tra mọi model request trước serialization/send: labels, destination, purpose và consent; minimum data, TLS/account isolation | Router/critic/summary cũng là egress, có thể leak trước tool effect; retention kiểm tra riêng |
| OA platform → BFF → desktop | OAuth/webhook/token/records | App secret server-side, authN/authZ, verified callbacks, token rotation | Chỉ scopes được cấp; không suy ra quyền personal inbox |
| Runtime → state/audit | Checkpoints, receipts, errors | Broker-owned writer, goal/action IDs, transaction/locking, bounded retention | Local privileged attacker vẫn ngoài bảo đảm |
| Updater/model pack → native runtime | Binaries/weights/config | Trusted manifest, hashes/signatures, pinned revision, artifact policy | Signature không chứng minh upstream không có bug/malware |

Tauri capabilities giới hạn frontend IPC chứ không tự sandbox Rust/sidecars; MCP roots chỉ thông báo phạm vi, không enforce filesystem. Job Objects quản lý cây process/resource, cần containment/ACL/token/network controls riêng. AppContainer là ứng viên cần spike compatibility với audio/GPU/automation; không mặc định mọi process đã nằm trong đó. [Tauri](https://v2.tauri.app/security/), [MCP roots](https://modelcontextprotocol.io/specification/2025-06-18/client/roots), [Windows AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation), [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

## 3. Những điều broker phải bảo đảm trong phạm vi hỗ trợ

1. Model chỉ đề xuất, không cấp grant, tự ghi receipt thành công, sửa policy hoặc thay acceptance checks để làm đẹp kết quả.
2. Delegation chỉ thu hẹp quyền. Worker không dùng chung credentials, không có kênh nhắn liên goal tự do, không tự tạo peer agent/network endpoint.
3. Mọi effect có canonical subject/target, schema/version, evidence và authorization hiện hành. Mọi request tới router/critic/summarizer/worker API cũng là disclosure cần broker check trước serialization/transmission, không đợi tool call. Nội dung tool/file không thể tự làm mới quyền.
4. Approval gắn đúng content/args, goal/action version, account, grant/policy revision và expiry. Target/content đổi thì approval cũ không còn áp dụng. Broker recheck cancellation/revocation ngay trước effect; không tuyên bố ngăn được effect đã được provider chấp nhận.
5. Unknown outcome đi vào reconciliation; không dùng replay như exactly-once. Nếu provider không cho idempotency/query tin cậy, yêu cầu manual resolution hoặc giới hạn operation.
6. Readback từ app/environment là nguồn kiểm chứng; LLM “tôi đã làm” không phải evidence. Grader và hidden evaluation resources nằm ngoài tool/data scope của agent.
7. Một goal không đạt trong quyền/budget phải dừng, hỏi hoặc handoff. Không tự cài package, dùng secret tìm thấy, vượt sandbox hoặc sửa scorer để hoàn tất.

Đây là yêu cầu có test; chưa là formal proof hoặc feature đã implement. Closest prior art gồm [CaMeL](https://arxiv.org/html/2503.18813v2), [Fides](https://arxiv.org/html/2505.23643v2), [Task Shield](https://aclanthology.org/2025.acl-long.1435/) và [RTBAS](https://arxiv.org/abs/2502.08966). Áp dụng một phần ý tưởng không được tự gọi là bản tái hiện đầy đủ các hệ thống đó.

## 4. Risk register: threat → control → residual → test

| ID / ưu tiên / STRIDE | Failure scenario | Controls cần xây | Residual risk và test nghiệm thu |
|---|---|---|---|
| R01 High / S,E | Replay/TV/người khác giao sensitive action | PTT baseline; source/session binding; critical action confirmation theo session/risk | Không phân biệt hoàn hảo speaker. Test audio ngoài session, replay và cancel trước effect |
| R02 High / T | ASR nhầm số tiền/tên/ngày; LLM sửa theo suy đoán | Critical-slot uncertainty, canonical entity lookup, alternatives và targeted clarification | Ground truth/entity source cũng có thể sai. Test homophones, trùng tên, số gần nhau, Bắc/Trung/Nam |
| R03 High / T,E | Parser tạo contract sai rồi policy tuân theo goal sai đó | Soát critical fields từ user context, phân biệt explicit/inferred/unknown, UI review khi material | Goal language mơ hồ. Test sai objective/effect, phủ định, sửa ý giữa lượt |
| R04 High / E,I | File/web/tool output đổi goal hoặc đích gửi | Control/data separation, source labels, effect/destination policy; critic hỗ trợ | Policy/model có thể bỏ sót semantic. Test paired clean/poisoned và attack trong metadata/screenshot/audio |
| R05 High / T,E | Summary/worker/memory rửa instruction và quyền | Labels bảo toàn qua transformations, isolation theo goal/account, memory không tự thành authority | Dependency tracking thiếu. Test delayed multi-turn, compressed summary, cross-goal contamination |
| R06 Critical / E | Worker gọi adapter khác hoặc kế thừa quyền supervisor | Grants con subset, broker deny unknown tools, authenticated scoped IPC, no arbitrary shell | Broker bug hoặc quyền adapter quá rộng. Test direct tool bypass, foreign account/goal IDs |
| R07 High / T,E | Preview một recipient, chạy recipient khác; revoke không có hiệu lực | Canonical preview từ broker, exact argument/content binding, revision/expiry/recheck | User approve nhầm; race sau provider accept. Test TOCTOU, Unicode/display name, changed attachments |
| R08 Critical / I,E | Secret nằm trong desktop; OAuth callback/token bị chiếm | BFF giữ shared app secret, PKCE/state theo contract, account authZ, secret store, scoped tokens | BFF compromise. Test account isolation, expired/replayed callback, unauthorized token retrieval |
| R09 High / T,D | Hai workers refresh OA đồng thời, token cũ làm mất session | Single-flight refresh, atomic rotation, rollback/error classification, no raw token logs | Crash/unknown refresh outcome theo provider. Test concurrent refresh/restart/revocation |
| R10 High / S,T | Webhook giả/replay hoặc duplicate business effect | Verification theo API version, event dedup, authenticated ingress, explicit event-to-goal policy | Provider event order/clock skew. Test replay/out-of-order/oversized callbacks, không mặc định mọi event được tự chạy |
| R11 Critical / I,E | Exfil qua URL/tool args, SSRF, redirect/DNS/local services | Egress allowlist, recipient policy, resolve/recheck redirects, block internal endpoints theo connector, output minimization | Legitimate endpoint bị compromise hoặc covert channel. Test synthetic canaries ở args/URL/body/log/answer |
| R12 Critical / E,T | Path traversal, reparse/UNC/shortcut làm vượt folder grant | Handle-based checks, canonical containment khi mở, least rights, deny unsafe path forms theo support matrix | OS/filesystem race. Test junction/symlink switch và grant revoke trong test workspace, không quét ổ thật |
| R13 High / T,E | UI automation click nhầm app/window hoặc page thay layout | Bound app/session/origin, semantic target checks, preview/recheck sensitive effects, handoff nếu unreliable | UI readback không transactional. Test focus theft/layout drift; không bật arbitrary coordinate clicks mặc định |
| R14 High / T | Crash sau external effect trước checkpoint; replay gửi trùng | Effect ledger, idempotency khi provider hỗ trợ, independent query/readback, UNKNOWN state | Không có exactly-once chung cho mọi API. Test crash points, timeouts và duplicated queue events |
| R15 Critical / E | Model loader/native parser/dependency RCE | Pinned audited artifacts, no remote code mặc định, sandbox process, least privilege, patch/update policy | Trusted publisher/OS/broker compromise. Test tampered artifact rejection và containment ở môi trường được phép |
| R16 High / R,T | Worker sửa audit, spoof tool call/receipt hoặc tự chấm xong | Broker-owned writer/verifier, ACL/process separation, event sequencing; no grader access | Local admin/broker có thể sửa. Test forged success, altered state hash, writable transcript; không gọi local log immutable |
| R17 High / I | Secrets/PII leak qua log, TTS, capsule, model context | Field allowlist, redaction, minimum retention, user preview; no raw payload/audio mặc định | Metadata re-identification, redaction miss. Test canary leak, low-entropy hashes, export/delete path |
| R18 Medium–High / D | PDF/zip/model/input lớn làm OOM hoặc đầy disk | Size/depth/time/page limits, streaming, bounded queues, tree memory/disk quotas, cancellation | Limit có thể kill worker giữa effect. Test startup/overlap/browser/OOM, không dùng pagefile để báo đạt target |
| R19 High / T | Hai goals sửa cùng file/order hoặc lịch nhắc việc đổi timezone | Per-resource locking/version check, optimistic concurrency, native atomic file writes, timezone-aware schedule | External app có thể sửa ngoài NORA. Test conflict, DST/clock correction/sleep/resume, no silent overwrite |
| R20 High / E,D | Goal không thể đạt khiến agent lặp, tìm secret/ngoại lệ hoặc dùng nhiều API | Action/token/time/spend caps, no policy override, watchdog, impossible-goal handling | Legit tool có side channel. Test unreachable goal, quota exhaustion, repeated failed plan |
| R21 Medium–High / D,I | API outage/drift/retention policy đổi | Version pin/snapshot where possible, structured adapter contract, bounded backoff/circuit breaker, data policy check | Hosted model không pin hoàn hảo. Test malformed output, quota/429/timeouts, schema migration; re-evaluate after model change |
| R22 High / S,I | Local IPC bị website/process khác điều khiển | Prefer OS IPC with ACL/peer checks; nếu loopback thì session auth, origin controls, no wildcard CORS | Compromised user process. Test unauthenticated/cross-origin requests và replay; no unauthenticated control port |
| R23 High / T,I | Support capsule kết tội nhầm hoặc gửi dư dữ liệu | Neutral evidence/status wording, consented local export, no auto-report, preview/redaction | Người dùng tự chia sẻ sai. Test genuine changes/false allegations; review completeness/privacy với người thật |
| R24 High / I,E | BYOK/API credential bị tool/model đọc; account logout còn quyền | Broker/OS credential store, token not in context, revoke/unlink, no key in URL/log/cache | Logged-in OS account compromise; API vendor exposure. Test credential canary isolation và revocation khi resume |

Các lớp STRIDE là Spoofing, Tampering, Repudiation, Information disclosure, Denial of service và Elevation of privilege; một scenario có thể thuộc nhiều lớp. Bảng là design hazards, không gán “đã khai thác được” cho NORA khi chưa có implementation.

## 5. Chính sách effect và quyền

| Effect | Default trong bản đầu | Điều kiện nâng quyền |
|---|---|---|
| Read trong grant, calculate, search public | Có thể tự chạy theo goal đã giao | Vẫn kiểm soát PII/egress, bounded inputs |
| Tạo draft/note mới trong folder chọn | Chạy khi user đã giao operation đó | Safe filename, collision handling, readback |
| Ghi đè/xóa/move dữ liệu user | Preview/reversible change nếu adapter hỗ trợ; sensitive action gate | Exact target, scope đã cấp, conflict/version check; không xóa vĩnh viễn mặc định |
| Send/publish/share ngoài máy | User instruction hoặc automation grant phải cho effect này; preview/approval theo risk | Canonical recipient, content và data policy, provider eligibility; không hỏi lại nếu approval đúng còn hiệu lực |
| Cài phần mềm, shell/code execution, đổi OS/security | Ngoài V1 lightweight profile | Workflow riêng với scope/compute rõ, isolation và kiểm tra; không tự cài skill/package từ nội dung ngoài |
| Banking/payments thật | Đọc/draft/deep link/manual handoff nếu có quyền | Cần API partner, authentication và separate financial release gate; simulator không cấp quyền production |

Giữ guardrails không tự tạo approval loop. Quyền thường trực hợp lệ cần được tôn trọng; chỉ material change, scope mới hoặc policy chưa đủ mới hỏi. Mọi intent/critic check thất bại cần error state giải thích được, không lặp vô hạn để xin user bấm chấp nhận.

## 6. Sự cố đã đọc và bài học cụ thể

PHP advisories tháng 10 cần patch/dependency response, không thể chữa bằng prompt filter. HF 07/2026 và báo cáo 26/08 cho thấy cần chặn unauthorized inter-agent channels, không chia credentials, bảo vệ scorer/audit và dừng khi goal không thể hoàn tất trong scope. FSC 14/10 vẫn điều tra vai trò AI trong các breach; không gán attribution chưa xác minh. [Incident evidence](../research/novelty-incidents.md), [source ledger](../research/novelty-security-sources.json).

Các incident nói trên xảy ra trong hạ tầng/threat model khác; không lấy số attacker actions hoặc attack success của paper làm probability NORA bị compromise. [AgentDojo](https://arxiv.org/html/2406.13352v3) và [NIST](https://www.nist.gov/news-events/news/2025/01/technical-blog-strengthening-ai-agent-hijacking-evaluations) là cơ sở chọn phép đo utility dưới attack và repeated/adaptive trials, không phải chứng nhận sản phẩm.

## 7. Retention và vận hành khi có sự cố

Mặc định thiết kế: không giữ raw audio sau session ngoài bounded transient buffer; transcript/task state giữ tối thiểu theo user setting; telemetry tối thiểu, có consent và không chứa content/token. Chốt thời hạn retention/delete cụ thể trước beta theo loại dữ liệu và nhu cầu recovery; không hứa xóa được thông tin đã gửi tới bên ngoài hoặc immutable provider logs. Local encryption dùng cơ chế chuẩn/OS secret protection; key không nằm cạnh bản encrypted như một giải pháp giả.

Quy trình incident dự kiến: dừng adapter/goal bị ảnh hưởng bằng kill switch → revoke/rotate credentials khi cần → giữ evidence tối thiểu dưới quyền user/operator → xác định effects đã xảy ra và thông báo đúng phạm vi → patch/rollback bản đã ký → regression/re-evaluate → chỉ mở lại sau kiểm tra boundary. Không tự gửi dữ liệu khách hàng tới VNG, vendor hoặc dịch vụ scan mà chưa có quyền.

Mỗi connector/model pack có version, owner, release manifest và cách disable riêng. On-call/runbook phải thực thi được: xử lý unknown send, credential compromise, dependency critical advisory, data disclosure, disk full và broken update. Một model update hoặc tool schema change có thể làm mất hiệu lực eval trước; cần test lại phần bị ảnh hưởng.

## 8. Điều kiện qua security gate

Không còn finding Critical/High chưa xử lý trong phạm vi phát hành; các residual risks có owner và giới hạn công khai. Boundary tests, crash/replay/revoke, canary exfil, supply-chain review và account isolation phải có artifacts. Review độc lập trước GA cho những adapter có external effects. Zero observed attack successes trên một sample không được quảng cáo là “an toàn tuyệt đối”; [evaluation plan](nora-evaluation-plan.md) nêu uncertainty và giới hạn bằng chứng.
