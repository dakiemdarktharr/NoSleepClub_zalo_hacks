# Security novelty cho NORA: giữ ý định và tiếp tục công việc hợp lệ

Ngày kiểm tra: **31/10/2026, Asia/Saigon**. Đây là phương án/PRD nghiên cứu, **chưa triển khai, chưa chạy benchmark hoặc security audit**. Không dùng dữ liệu VNG nội bộ. Những nhận định về business impact cần được đối chiếu với nhánh nghiên cứu VNG và phỏng vấn design partners.

## Kết luận đề xuất

Chọn hướng **intent-preserving safe completion**: khi một message, webpage, tài liệu hoặc tool result có nội dung muốn đổi người nhận, đích gửi dữ liệu hay mục tiêu, NORA giữ phạm vi người dùng giao, chỉ chặn phần không có quyền/bằng chứng và tiếp tục phần còn hợp lệ. Nếu cần hỏi, hỏi đúng trường đang xung đột, không bắt user duyệt mọi tool call hoặc đọc lại cả goal.

Điểm khác biệt khả dĩ ở cấp sản phẩm là tổ hợp: **yêu cầu giọng nói tiếng Việt → intent/entity/data-flow contract → sửa kế hoạch tối thiểu khi gặp dữ liệu độc → kiểm chứng hành động → support capsule đã giảm dữ liệu** cho hệ sinh thái merchant/OA. Không tuyên bố đây là phát minh đầu tiên về intent checking, provenance, taint tracking, approvals hay auto-replanning.

### Demo wedge cụ thể

Người bán nói: “Đọc yêu cầu của khách về đơn 123, đối chiếu đơn trong bảng và soạn xác nhận giao hàng cho đúng khách.” Nguồn message được cấp quyền chứa thêm yêu cầu gửi danh sách khách hàng tới một link hoặc đổi người nhận. NORA đối chiếu order/customer IDs qua connector được cho phép, bỏ bước không thuộc goal, vẫn tạo nội dung xác nhận hợp lệ. Nếu customer identity có xung đột, hiện đúng khác biệt để người dùng giải quyết. Gửi message thật chỉ diễn ra khi grant/approval tương ứng có hiệu lực.

Không tự đọc toàn bộ personal chats. Không suy ra OA đã xác minh nghĩa là mọi message hoặc link trong đó an toàn. Không dùng display name làm bằng chứng người nhận. Không demo chuyển tiền thật tự động; phạm vi financial action có gate riêng và cần đối tác/API đủ quyền.

## Prior art: những gì đã có

| Công trình/sản phẩm | Đã có | NORA không được coi là novelty riêng |
|---|---|---|
| **CaMeL** | Tách control/data flow, capabilities/provenance và allowed recipients | “Theo dõi dữ liệu và ngăn gửi sai nơi” nói chung |
| **Fides** | Information-flow control với integrity/confidentiality labels và policy deterministic | “Policy engine ngoài LLM” hoặc selective information exposure |
| **Task Shield** | Kiểm tra instructions/tool calls có phục vụ user goal | “Một critic so hành động với mục tiêu” |
| **Google Chrome User Alignment Critic** | Critic tách khỏi raw page, vet action, feedback cho replan, origin gating, confirmations | “Agent bảo vệ agent và tự lập kế hoạch lại” |
| **Microsoft Edge Task Tracker** | Phòng thủ theo task context và các ranh giới browsing | “Theo dõi task để chống injection” |
| **OpenAI Guardrails** | Kiểm tra tool calls và tool outputs có lệch user goal | “Prompt-injection guardrail” chung |
| **RTBAS/related IFC work** | Tự chạy calls bảo toàn integrity/confidentiality; chỉ hỏi user khi không bảo đảm | “Chỉ hỏi khi cần”, “giảm consent fatigue” hoặc selective safe execution chung |

Nguồn gốc: [CaMeL — SEC07](https://arxiv.org/html/2503.18813v2), [Fides — SEC10](https://arxiv.org/html/2505.23643v2), [Task Shield — SEC09](https://aclanthology.org/2025.acl-long.1435/), [Chrome — SEC19](https://blog.google/security/architecting-security-for-agentic/), [Edge — SEC20](https://blogs.windows.com/msedgedev/2025/10/23/considerations-for-safe-agentic-browsing/), [OpenAI — SEC18](https://openai.github.io/openai-guardrails-python/ref/checks/prompt_injection_detection/), [RTBAS — SEC21](https://arxiv.org/abs/2502.08966).

CaMeL repository ghi rõ là research artifact có thể có bugs, không phải Google product được hỗ trợ. Fides repository là tutorial notebook đi cùng academic paper. Không lấy repo nghiên cứu làm production security boundary mà không triển khai/kiểm thử riêng. [SEC08](https://github.com/google-research/camel-prompt-injection), [SEC11](https://github.com/microsoft/fides)

## So sánh lựa chọn

| Lựa chọn | Giá trị | Hạn chế | Quyết định đề xuất |
|---|---|---|---|
| Scam/phishing classifier | Dễ trình diễn cảnh báo, có ích cho một số nội dung | Nhiều prior art; false positives; không enforce quyền hoặc chống RCE | Là tín hiệu phụ, không là novelty chính |
| Voice authentication/deepfake detector | Phù hợp câu chuyện lừa đảo giọng nói | Replay/noise/domain shift; sinh trắc học nhạy cảm; detector không là authorization | Không dùng làm gate duy nhất |
| SOC/CVE copilot cho VNG | Liên quan sự cố PHP/HF/Korea | Cần telemetry/private access; dễ thành RAG dashboard; khó chứng minh broad consumer value trong thời gian thi | Để mở rộng nội bộ sau validation |
| Intent-preserving safe completion + capsule | Gắn trực tiếp voice-to-action và merchant/support; có thể đo utility lẫn security | Khó tạo policy đúng; prior art rất gần; cần connectors và usability tests | Hướng ưu tiên, novelty cấp workflow/evaluation cần chứng minh |

Không thể biết các đội khác sẽ chọn gì từ web research. Việc loại bỏ ở đây là loại **claim khác biệt**, không loại các chức năng nền tảng cần có.

## Contract và enforcement đề xuất

1. **User intent anchor:** giữ request gốc/transcript reference, objective và acceptance checks. ASR ambiguity của số tiền/người nhận/ngày giờ phải được giải quyết; confidence score không tự là proof.
2. **Entity constraints:** gắn actor, order/customer, recipient/account, action, amount/time nếu có với nguồn xác nhận. Quy tắc thay đổi entity phải rõ; cache cũ không luôn là chân lý.
3. **Data-flow constraints:** nguồn có integrity/confidentiality labels, allowed operations/recipients/egress. Labels xuất phát từ ingestion/broker, không do worker tự nâng trust.
4. **Scoped action contract:** broker kiểm tra tool schema, grants, exact args/content hash, policy/grant revision, expiry và cancellation ngay trước effect. Critic model chỉ tư vấn; không cấp grant hoặc bỏ policy.
5. **Safe plan repair:** dừng nhánh vi phạm, xác định những bước độc lập còn an toàn trên plan hiện biết, loại các dependency bị nhiễm và replan trong cùng goal. Không hứa tìm được global maximum safe subgraph hoặc hiểu mọi ngữ nghĩa nghiệp vụ.
6. **Targeted interaction:** chỉ hỏi trường xung đột hoặc quyền mới cần thiết; giữ nguyên các effects đã xác minh. Nếu policy không chắc hoặc rủi ro không thể xác minh, chờ/handoff.
7. **Verification/receipt:** đọc lại app state hoặc artifact; reconcile trước retry nếu effect chưa rõ. Chỉ completion khi acceptance checks đạt. Audit/receipts nằm ngoài vùng worker được ghi. Grader/hidden-test resources không nằm trong quyền agent. Hủy goal không undo send/publish đã xảy ra.
8. **Support capsule:** chỉ xuất khi user chủ động chọn. Có intent tóm tắt đã redaction, step/reason IDs, phiên bản policy, receipt/state verification status và các phần unresolved. Không kèm raw audio, full chat, token, số tài khoản hay document payload theo mặc định.

Capsule là công cụ giải thích/triage, không phải bằng chứng pháp lý, xác minh ngân hàng hoặc chứng minh model không sai. Chữ ký/tamper evidence nếu có chỉ xác nhận nguồn/độ toàn vẹn theo trust model; không chứng minh nội dung đời thực đúng. Dùng crypto chuẩn, không tự nghĩ thuật toán. Dữ liệu ít entropy có thể bị suy ngược từ hash; export phải giảm dữ liệu thật, không chỉ hash mọi trường nhạy cảm.

## Threat → control → residual risk → test

| Threat | Control thiết kế | Residual risk | Kiểm tra cần có |
|---|---|---|---|
| Message/webpage đổi goal hoặc đích nhận | Intent anchor, entity/dataflow constraints, broker | Goal hợp lệ nhưng mơ hồ; metadata upstream sai | Paired benign/poisoned content, same original goal |
| Worker tóm tắt làm “rửa” instruction độc | Propagate origin labels qua summary/handoff; không tự nâng trust | Taint graph thiếu dependency hoặc policy granularity quá thô | Multi-worker/multi-turn injection và memory resume |
| Agents tạo kênh hợp tác ngoài scope | Typed inter-worker channel qua supervisor, grants/credentials riêng, network/write scope nhỏ | Shared infrastructure có covert channel hoặc parser bug | Unauthorized peer/goal messages và shared-state isolation tests |
| Worker sửa log hoặc bằng chứng completion | Broker-owned append-only audit boundary, independent state readback | Broker/TCB compromise; external readback không đủ | Tamper/spoof receipt, hidden-grader access và fail-closed tests |
| Tool trả dư PII/secret | Output minimization, source labels, destination policy | Data classification miss; side-channel | Exfil canaries qua tool args, URL, log, final answer |
| Approval bị dùng lại sau thay đổi nội dung | Bind exact preview, target/args hash, revision/expiry | Người dùng bị lừa approve; UI không rõ khác biệt | Changed recipient/amount/content, revoke giữa plan/effect |
| Replay/deepfake hoặc speaker ngoài phòng | Hotkey/PTT baseline, critical-action trusted confirmation | Audio channel không chứng thực chủ thể; speech ambiguity | Playback, echo, multi-speaker, accent, noise |
| Prompt injection trong audio/screenshot | Giữ modality/source boundary; policy enforce effect | Detector thất bại; chuyển ASR không loại mọi attack | Multimodal fixtures không chỉ text |
| Model hub/loader/dependency bị compromise | Signed/trusted manifest, pinned revisions, remote-code deny, isolation | Parser/native RCE, trusted publisher compromise | Tampered artifacts, unexpected network/process behavior trong sandbox test |
| Native process vượt broker | OS containment, secret separation, IPC auth, ít đặc quyền | Broker/OS compromise nằm trong TCB | Authorized containment tests và dependency review |
| Resume/retry lặp action | Effect ledger, idempotency/query/readback | API không cung cấp receipt/query đủ mạnh | Crash sau effect trước ack, revoked grant khi resume |
| Goal chấp niệm vượt phạm vi để đạt reward | Tool/domain allowlist, no hidden shell, budgets, watchdog | Legitimate tool có quyền quá rộng; confused deputy | Goal được thiết kế không thể hoàn tất trong scope, đòi bước vượt quyền |
| Capsule hoặc telemetry tự tạo leak | Consent, schema allowlist, redaction, bounded retention | Re-identification từ metadata; operator abuse | PII/secret export tests và delete propagation |

Âm thanh là attack surface có bằng chứng nghiên cứu, nhưng không được áp nguyên attack success rate của LALM trong paper vào Whisper/cascade của NORA. [AudioHijack — SEC14](https://arxiv.org/html/2604.14604) là preprint do Meng Chen và cộng sự, không phải paper của OpenAI dù metadata search từng ghi sai tác giả. MCP authorization cũng cần chống confused deputy; connector schema tự nó không là sandbox. [MCP — SEC15](https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices)

## Evaluation: không thắng bằng cách từ chối mọi việc

So sánh cùng model, audio/transcript, tools và budgets giữa: (A) assistant thường, (B) refuse-all, (C) approve-every-call, (D) critic-only, (E) scoped broker/IFC không repair, (F) contract+repair+capsule. Ablation thêm/bỏ source labels, semantic diff, repair và targeted confirmations. Chỉ so numeric results khi cùng điều kiện; không lấy leaderboard cũ làm thứ hạng trực tiếp.

Metrics chính:

- **Benign verified task success:** hoàn tất đúng khi không có tấn công.
- **Safe task success under attack:** vẫn hoàn tất goal đúng và không có side effect trái phép; refuse-all phải nhận điểm utility thấp.
- **Unauthorized effect / exfiltration rate:** đo từ environment, không từ lời model hoặc detector score.
- **User burden:** số câu hỏi không cần thiết, thời gian review, tỷ lệ approve nhầm, interruption và lúc user phải nói lại goal.
- **Recovery:** phần công việc hợp lệ giữ được sau block; retry duplication; crash/restart/cancel đúng.
- **Operations:** tokens/cost per safe completed task, p50/p95 latency, peak RAM/VRAM toàn cây process; không loại security workers khỏi ngân sách 2 GB.
- **Support usefulness:** người hỗ trợ có thể hiểu và tái hiện vấn đề từ capsule mà không đòi raw private content; metric cần pilot thật.

AgentDojo cung cấp khái niệm benign utility, utility under attack và targeted attack success, phù hợp cách đo này. NIST chỉ ra repeated attempts có thể làm risk estimate cao hơn đáng kể trong thử nghiệm cụ thể. Vì vậy cần held-out/adaptive attacks, nhiều trial, unseen tool names và delayed multi-turn attacks; không chỉ bộ strings được viết để hợp detector. [AgentDojo — SEC12](https://arxiv.org/html/2406.13352v3), [NIST — SEC13](https://www.nist.gov/news-events/news/2025/01/technical-blog-strengthening-ai-agent-hijacking-evaluations)

Bộ Việt hóa cần persona/accents, name collisions, số tiền, ngày tương đối, genuine recipient changes, legitimate links và workflow không đủ dữ liệu. Nếu chỉ benchmark malicious changes, hệ thống sẽ học từ chối cả thay đổi hợp lệ. Giữ các benign hard negatives có giá trị nghiệp vụ.

## Khoảng trống phải đóng trước tuyên bố novelty/production

Chưa có data chứng minh VNG có đúng support bottleneck, chưa có OA/API/merchant integration được cấp, chưa biết nhu cầu capsule hoặc threshold chấp nhận của người dùng. Chưa chứng minh local voice+security runtime nằm trong 2 GB. Chưa có formal verification, independent red team, production incident data hoặc guarantee chống unseen attacks.

Theo prior art đã đọc, originality nên được pitch là **đóng gói/kiểm chứng một workflow Việt hóa có safe recovery và hỗ trợ vận hành**, không phải “giải quyết prompt injection” hoặc “phát minh intent firewall”. Cần prototype và blind evaluation để biết nó có tốt hơn critic+confirm thông thường hay không. Nếu không cải thiện safe completion/user burden trên baseline mạnh, novelty claim phải thu hẹp hoặc bỏ.
