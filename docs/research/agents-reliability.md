# Agent workflow, độ tin cậy và quyền truy cập

Ngày đối chiếu: **2026-10-31**, múi giờ Asia/Saigon. Phạm vi: trợ lý giọng nói chạy nền trên Windows, thực hiện công việc trong app và folder được cấp quyền. Đây là nghiên cứu kiến trúc chung cho sản phẩm; không sử dụng dữ liệu cuộc thi hoặc tạo nội dung nộp thi. Nguồn và giới hạn bằng chứng nằm trong [workflow-sources.json](workflow-sources.json).

## Kết luận kiến trúc

Chọn một orchestrator có trạng thái bền vững, permission broker và bộ xác minh kết quả. Mặc định chỉ chạy một agent. Tạo worker khi có những nhánh độc lập đủ giá trị để bù chi phí điều phối. Anthropic phân biệt workflow do code kiểm soát với agent tự chọn bước tiếp theo, đồng thời khuyên bắt đầu bằng cấu trúc đơn giản. Đây là kinh nghiệm nhà cung cấp, không phải bằng chứng multi-agent luôn tốt hơn. [A11](https://www.anthropic.com/engineering/building-effective-agents)

Hướng triển khai đầu tiên: desktop shell → local supervisor → Python worker/LangGraph → permission broker → adapter ứng dụng. Lưu goal, checkpoint, event và effect ledger bằng transaction. Đánh giá SQLite cho một thiết bị; khi có backend nhiều người dùng, xem xét PostgreSQL hoặc Temporal. Không đặt cả LangGraph lẫn Temporal vào critical path ngay từ đầu. Lựa chọn này là đề xuất thiết kế, chưa được benchmark trên máy người dùng.

## Router: độ phức tạp khác với quyền

| Tuyến | Dấu hiệu | Thực thi |
|---|---|---|
| Direct | Đủ ngữ cảnh, không tác động môi trường | Xử lý xác định hoặc một lượt model |
| Bounded tool | Ít bước, thao tác và đích đã biết | Tool có schema, kiểm tra quyền và đầu ra |
| Durable goal | Nhiều phụ thuộc, cần tìm hiểu hoặc resume | Goal, graph, checkpoint, ledger, verifier |
| Clarify/approve | Thiếu thông tin quyết định hoặc vượt quyền | Lưu trạng thái chờ, hỏi đúng phần thiếu |

Không phân loại bằng độ dài prompt. Cần xét phụ thuộc, môi trường chưa biết, tác động ngoài, dữ liệu nhạy cảm và khả năng chờ. “Xóa thư mục” ngắn nhưng có rủi ro; “giải thích tài liệu dài” có thể không cần agent. Một câu nói tạo một goal; thiếu quyền dùng app chỉ làm nhánh liên quan chờ, không yêu cầu người dùng nói lại toàn bộ.

## Runtime và giới hạn

| Phương án | Phù hợp | Trách nhiệm còn lại |
|---|---|---|
| LangGraph | Graph, checkpoints, interrupts, resume | Supervisor, scheduler, quyền, ledger, retention |
| Temporal | Job dài, timers, nhiều worker, chờ duyệt | Service vận hành, deterministic workflow, activity idempotency |
| OpenAI Agents SDK | Agent loop, tools, sessions, tracing, HITL | Goal store, durable scheduling, reconciliation |

LangGraph checkpoint ở biên super-step. Khi resume, node có thể chạy lại từ đầu, gồm thao tác trước `interrupt()`. Dùng persistent checkpointer; `InMemorySaver` mất dữ liệu khi restart. Chế độ `sync` ghi checkpoint trước bước tiếp theo, đổi lại thêm độ trễ. Checkpointer không tự khởi động lại worker. [A02](https://docs.langchain.com/oss/python/langgraph/interrupts), [A16](https://docs.langchain.com/oss/python/langgraph/checkpointers)

Temporal dựng lại trạng thái bằng replay event history; LLM và I/O phải nằm trong Activities. Activity đã ghi kết quả không chạy lại khi replay, nhưng tác động có thể đã xảy ra trước khi worker mất kết nối và bị retry. Không gọi đây là bảo đảm exactly-once cho hành động bên ngoài. [A03](https://docs.temporal.io/workflows), [A04](https://docs.temporal.io/activity-definition)

Agents SDK có approval interruptions và serialize/resume `RunState`; sessions lưu lịch sử hội thoại. Những khả năng này không thay thế durable scheduler. Tracing bật mặc định nên phải chủ động kiểm soát dữ liệu gửi đi. Tài liệu được truy cập ở ngày nghiên cứu; chưa pin hoặc cài phiên bản SDK để thử API. [A05](https://openai.github.io/openai-agents-python/human_in_the_loop/), [A06](https://openai.github.io/openai-agents-python/tracing/), [A09](https://openai.github.io/openai-agents-python/sessions/)

## Goal và bằng chứng hoàn thành

Goal cần lưu `goal_id`, chủ sở hữu, transcript gốc, objective, acceptance checks, phạm vi quyền, plan version, budget, checkpoint, approvals, effect receipts, evidence refs và runtime/policy version. Budget gồm số bước, chi phí, deadline và số worker tối đa; các ngưỡng phải được đo và cấu hình, không xem một con số minh họa là mặc định production.

Trạng thái nên có `queued`, `running`, `waiting_permission`, `waiting_input`, `verifying`, `completed`, `failed`, `cancelled`, `needs_reconciliation`. Vòng chạy: hiểu yêu cầu → xác định điều kiện đạt → lập plan → kiểm tra quyền → thực thi → đọc receipt và trạng thái thật → kiểm tra budget/cancel → xác minh hoặc tiếp tục. Replan phải nằm trong mục tiêu gốc, không tự mở rộng quyền.

Chỉ chuyển `completed` khi acceptance checks đạt. Tạo báo cáo cần kiểm tra file tồn tại, đọc được và có nội dung yêu cầu. Thao tác app cần kiểm tra trạng thái thực tế; lời model nói “đã xong” không đủ. Evals phải phân biệt transcript với outcome. [A12](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents)

## Effect ledger và phục hồi

Trước tác động, ghi `effect_id`, goal, loại hành động, target chuẩn hóa, hash arguments, approval reference và trạng thái. Adapter truyền idempotency key nếu API hỗ trợ. Sau tác động, lưu receipt rồi đối chiếu kết quả. Với file mới, cân nhắc ghi tạm và atomic rename; với hệ thống ngoài, lưu identifier để truy vấn lại.

Nếu process chết giữa tác động và receipt, resume phải reconciliation trước khi retry. Không retry mù với publish, send, delete hoặc payment. Không xác minh được thì dùng `needs_reconciliation` và báo phần chưa chắc chắn. Ledger giúp phát hiện và xử lý tình huống này; bản thân ledger không tạo transaction xuyên mọi ứng dụng. Cả LangGraph và Temporal vẫn cần side effect idempotent. [A02](https://docs.langchain.com/oss/python/langgraph/interrupts), [A04](https://docs.temporal.io/activity-definition)

Cancel phải ngăn bước mới, hủy request và child process khi cơ chế hỗ trợ. Hủy không đảo ngược tác động đã xảy ra; báo receipt của phần hoàn tất. Trước mỗi bước mới, kiểm tra lại budget, quyền bị thu hồi và yêu cầu hủy.

## Cập nhật chọn lọc

Worker phát event có cấu trúc; policy xác định chọn `suppress`, `timeline`, `toast`, `speak` hoặc `ask`. Không stream raw reasoning hoặc toàn bộ log ra audio. Có thể dùng model diễn đạt sau khi event qua gate, nhưng câu nói phải dựa trên bằng chứng.

| Event | Hành vi mặc định |
|---|---|
| Goal accepted | Card mục tiêu; xác nhận ngắn nếu việc dài |
| Worker started | Gộp milestone nếu phân công có ích cho người dùng |
| Sources collected | Chỉ đếm nguồn duy nhất đã đọc, có liên quan |
| Material finding | Báo phát hiện làm đổi quyết định hoặc kết quả |
| Permission/blocker | Nêu điều cần giải quyết và hành động cụ thể |
| Epoch, token, skill init | Log kỹ thuật; ẩn khỏi cập nhật thường |
| Completed | Kết quả, artifact và kiểm tra đã đạt |
| Heartbeat không đổi | Giữ yên lặng |

Đây là thiết kế đề xuất, chưa có bằng chứng người dùng cho các ngưỡng tối ưu. Dedupe theo event/milestone và persist trạng thái đã thông báo. Gộp các worker starts gần nhau; quota tùy chế độ yên lặng, bình thường, chi tiết. Không cắt ngang người dùng; TTS có queue, priority và barge-in. Quiet hours có thể chỉ ghi timeline.

Khi người dùng hỏi tiến độ, trả lời từ state hiện tại. Timeout chỉ tạo update khi có ý nghĩa: vượt estimate, tool treo hoặc mất kết nối. Epoch bình thường giữ trong log; train thất bại hay không thể đạt điều kiện đã hứa là thay đổi đáng báo. Không bịa ETA hoặc phần trăm cho vòng agent mở.

“Đã đọc 12 nguồn” đòi hỏi read/extraction thực tế; kết quả search không tương đương đã đọc. Nhiều URL dẫn về cùng một nghiên cứu không phải nhiều bằng chứng độc lập. “Đã tạo agent” chỉ nói sau khi worker thực sự được khởi tạo và ghi nhận.

## Quyền phải được thực thi ngoài model

MCP roots là thông tin định hướng, không cưỡng chế server chỉ truy cập folder đó. Local stdio server là process với quyền môi trường được cấp; stdio không phải sandbox. Vì vậy cần broker quyền và isolation thực tế. [A13](https://modelcontextprotocol.io/specification/2026-07-28/client/roots), [A15, hướng dẫn draft](https://modelcontextprotocol.io/docs/draft/tutorials/security/local-server-security)

Grant gồm chủ thể, tool/app, resource, operations, quyền đưa dữ liệu ra ngoài, scope goal/session, expiry và revoke version. Đọc folder khác ghi folder; mở app khác chỉnh mọi tài liệu; đọc file khác tải lên model cloud. Quyền dùng app không tự cho phép gửi, xuất bản hoặc xóa.

Broker kiểm tra ngay trước effect. Chuẩn hóa path, chống traversal, xử lý symlink/junction và tránh TOCTOU; không chỉ kiểm tra chuỗi prefix. IPC local phải xác thực và giới hạn quyền, không để trang web điều khiển endpoint localhost. Không cung cấp generic shell mặc định.

Approval ràng buộc với action arguments, target và content hash. Nội dung hoặc đích thay đổi thì duyệt cũ hết hiệu lực. Thu hồi quyền phải chặn bước mới. Wake word hay speaker match không phải xác thực cho hành động nhạy cảm.

Web page, file, tool description và memory retrieval đều có thể chứa prompt injection. Nội dung là dữ liệu, không được sửa policy hoặc cấp quyền. Remote MCP cần consent đúng client, resource/audience validation và kiểm soát token passthrough/confused deputy. Credential riêng từng connector, lưu trong OS secret store. Những biện pháp này giảm rủi ro, không chứng minh đã loại bỏ prompt injection. [A14](https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices), [A08, bản protocol lịch sử](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization), [A10](https://genai.owasp.org/resource/agentic-ai-threats-and-mitigations/)

## Memory có nguồn

Tách transcript, checkpoint, evidence, preferences và long-term facts. Mỗi fact có nguồn, thời điểm lấy, tình trạng xác minh, scope user/project và TTL nếu dễ đổi. Tóm tắt do model sinh không tự trở thành ground truth. Retrieval phải kiểm tra quyền hiện tại, kể cả dữ liệu từng được đọc hợp lệ.

Note Obsidian phân biệt fact ngoài đã xác minh, trạng thái quan sát tại máy, quyết định thiết kế, giả thuyết và câu hỏi mở. Cho phép xem/sửa/xóa memory; retention bao gồm index và dữ liệu dẫn xuất. Checkpointer giữ trạng thái một thread, store giữ dữ liệu dài hạn; hai vai trò không đồng nhất. [A01](https://docs.langchain.com/oss/python/langgraph/durable-execution)

## Điều kiện phát hành

Đây là release gates đề xuất, chưa phải kết quả đạt:

- Bộ task tiếng Việt gồm direct/tool/goal, thiếu ngữ cảnh, tên app/file pha tiếng Anh, sửa lời và lỗi nghe; nhiều trial cho từng nhóm.
- Đo thành công bằng file, API hoặc app state thật; kiểm tra riêng lỗi báo hoàn thành sai.
- Crash matrix trước effect, sau effect trước receipt, lúc approval, mất mạng và resume checkpoint.
- Không có effect vượt scope trong suite; approval stale/revoked bị từ chối; retry không gây duplicate ngoài dự kiến. Suite hữu hạn không chứng minh không còn lỗi.
- Red-team injection qua page/file/tool/memory; kiểm tra credential leakage và cross-goal access.
- Đánh giá update hữu ích, cắt ngang, bỏ sót blocker và lặp sau restart bằng human review tiếng Việt.
- Kiểm tra budget, cancel, retention/delete/export, redaction, signed updates, rollback và kill switch cho tool.

Không phát hành chỉ dựa trên demo thành công. Cần lock dependency, xem license/changelog, chạy compatibility/security tests và quan sát canary. Tài liệu live có thể thay đổi; chỉ tuyên bố tính năng đã xác minh trên phiên bản thực tế. [A12](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents)
