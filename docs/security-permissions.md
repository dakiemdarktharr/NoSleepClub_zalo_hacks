# Quyền truy cập phải được thực thi

Tài liệu thiết kế, chưa phải security audit hoặc sandbox đang chạy. Ranh giới quan trọng nhất là giữa **model đề xuất hành động** và **broker được phép thực hiện hành động**.

## Threat model

Tài liệu/webpage/tool description có prompt injection; connector bị cấu hình sai; worker retry sau crash; frontend bị chèn nội dung; app hoặc plugin ngoài không đáng tin; voice input bị nghe nhầm hoặc phát lại từ loa; token/log vô tình tiết lộ; grant bị thu hồi khi việc đang chạy.

Mô hình, retrieved content và memory không có quyền cấp permission. Mã first-party, broker, OS và configured identity là trust base; dependency compromise vẫn là rủi ro cần cập nhật và containment. Hệ thống phải hạn chế effects ngay cả khi model tin một chỉ dẫn độc hại. [MCP security](https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices)

## Grant record

```json
{
  "grant_id": "grant-report-folder",
  "subject": "current-user",
  "connector": "filesystem",
  "resource": "canonical-folder-identity",
  "operations": ["read", "create"],
  "egress": "local_only",
  "scope": {"goal_id": "goal-example"},
  "expires_at": "explicit-expiry",
  "revision": 1,
  "revoked": false
}
```

Resource phải là identifier được broker resolve, không chỉ chuỗi do model tự cung cấp. App grant phân biệt read/search/create/update/send/publish/delete. Folder grant phân biệt read/create/overwrite/delete. Network egress là quyền riêng, có destination/data classes. Scope có thể một effect, một goal, session hoặc persistent preference do user chọn; mặc định không làm grant rộng thêm.

Hỏi đúng lúc: “Để lưu báo cáo, cho phép tạo note trong folder NORA của vault này?” Card hiện resource, hành động, data egress, thời hạn và lựa chọn từ chối. Grant đang hợp lệ được dùng lại, không bắt xác nhận lặp. Từ chối không làm mất goal; nhánh cần quyền chờ, nhánh độc lập có thể tiếp tục.

## Trước mỗi action

1. Xác thực process/user/goal và schema version.
2. Resolve connector/action/resource; tool name hoặc arguments lạ bị từ chối.
3. Kiểm tra grant còn hiệu lực, revision và current user identity; không chỉ kiểm tra lúc planning.
4. Kiểm tra output/input data classes và cloud egress.
5. Đối với effect cần review, đối chiếu exact preview hash và approval còn hiệu lực.
6. Kiểm tra budget/cancel state; persist effect intent trước thực thi.
7. Thực thi typed adapter với tối thiểu quyền; ghi receipt đã redaction, verify actual state.

Approval gắn với tool, target, canonical arguments/content hash, expiry, grant/policy revision. Nội dung/đích nhận thay đổi thì approval cũ không còn hợp lệ. Production phải dùng trusted approval store/nonce hoặc token được xác thực, không tin JSON `approved=true` do model gửi. Voice speaker match/wake word không phải xác thực cho cấp grant hoặc giao dịch nhạy cảm.

## Filesystem

Không dùng string `startsWith` để kiểm tra folder: `Reports2` không nằm trong `Reports`; dấu phân cách, case, UNC paths, symlink/junction/reparse points, alternate data streams và TOCTOU đều cần xử lý trên Windows.

Thiết kế handle-based resolution để kiểm tra final target nằm trong allowed root; không resolve rồi thả lock và mở path khác sau đó. Test thay junction giữa check/use, traversal, path alias/case, network share và mục tiêu bị thay đổi. Không ghi đè note của user theo mặc định; tạo artifact mới hoặc preview diff. Atomic rename giúp publishing artifact local nhưng không giải quyết mọi ACL/TOCTOU case.

Broker không được trở thành generic filesystem/shell proxy. Khi cần code execution thật, dựng isolated workspace/runtime với quyền riêng; không chạy nội dung từ model trong interpreter có quyền toàn bộ user profile.

## Isolation và Windows automation

MCP roots là thông tin định hướng, protocol không enforce server chỉ ở trong roots. Stdio server vẫn là process với quyền của môi trường; Tauri capabilities cũng không sandbox core/sidecar. [MCP roots](https://modelcontextprotocol.io/specification/2026-07-28/client/roots), [Tauri boundary](https://v2.tauri.app/security/capabilities/)

Vì vậy production cần quyết định containment rõ: broker code first-party được ký/review; untrusted tools chạy trong môi trường có OS-enforced limits. AppContainer/restricted token là ứng viên spike cho worker phù hợp, không phải cấu hình đã có. AppContainer giới hạn window interaction nên adapter điều khiển desktop cần được tách và gate chặt. Job Objects dùng cho resource/process lifecycle, không thay thế sandbox. [AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation), [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)

UI Automation bị giới hạn bởi integrity levels, app accessibility và secure desktop. Không tự elevate admin, tắt UAC, hoặc dùng UIAccess để “mở mọi app”. API có typed permissions là ưu tiên; UI fallback phải verify trước/sau, phát hiện focus đổi và fail closed nếu target không chắc chắn. [Microsoft UI Automation security](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview)

## Prompt injection và egress

Đặt untrusted content trong data boundary; nó không được đổi system policy, đòi token, mở app mới hoặc biến task đọc thành task gửi. Retrieved tool instructions không cấp quyền. Connector descriptors có version, allowlist và provenance; update permission đáng kể cần review.

Credential riêng từng connector, scope tối thiểu, lưu trong OS secret store. Không đặt API key trong frontend, prompt, repo hay URL log. Remote MCP cần đúng authorization audience/resource và chống confused deputy; dùng revision tương thích được pin thay vì trộn thông số spec lịch sử/draft. [MCP security practices](https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices)

Không coi “ignore prompt injection” trong system prompt là phòng thủ hoàn chỉnh. Cần adversarial tests qua file, web, tool descriptions, memory và transcripts; deny unauthorized effects khi model bị lừa. Chưa có bằng chứng NORA chống được mọi injection.

## Dữ liệu và quyền riêng tư

Mic indicator liên tục; mute/quit/lock screen rõ. Raw audio không lưu mặc định; opt-in eval recordings có retention, export/delete. Transcript/log có thể chứa tên, đường dẫn, tài chính hoặc nội dung riêng tư nên cần minimization/redaction. Cloud processing chỉ nhận phạm vi dữ liệu user đã chấp thuận; local read permission không cho phép cloud upload.

Tracing có thể export model/tool spans, vì vậy cấu hình tắt hoặc redaction/sampling theo dữ liệu, không chỉ bật mặc định. [Agents SDK tracing](https://openai.github.io/openai-agents-python/tracing/)

Thiết kế data inventory và privacy review theo thị trường triển khai; nghiên cứu này không kết luận đã tuân thủ mọi luật. Với dữ liệu cuộc thi, áp dụng điều kiện sử dụng và phân phối của BTC; không đưa audio/dataset vào public repo hoặc provider cloud khi chưa xác minh quyền. [Thể lệ, mục 4](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf)

## Failure behavior

Grant revoked → chặn effect mới và recheck khi resume. App/API không hoạt động → báo blocker có thể hành động. Timeout với side effect không rõ → `needs_reconciliation`; không retry send/publish/payment mù. Cancel → dừng bước mới, ghi effects đã xong, không tự tuyên bố rollback.

Proof of completion đến từ file/content hash, app/API readback hoặc simulator receipt; không lấy screenshot hoặc lời model làm bằng chứng duy nhất nếu có state API chính xác hơn. Không có verifier đủ mạnh thì thông báo giới hạn thay vì “đã xong”.
