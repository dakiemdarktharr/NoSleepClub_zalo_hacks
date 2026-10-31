# NORA nói khi có thay đổi đáng biết

Mục tiêu: người dùng nắm được công việc mà không nghe một luồng log. Policy nhận **domain events đã xác thực**, chọn surface và thời điểm; renderer diễn đạt ngắn sau đó. Lời model tự báo không đủ làm event verified.

Bản tích hợp Rust hiện đã có hàng đợi bounded, TTS drain acknowledgement, quiet mode và câu hỏi/xác nhận có thể đọc thành tiếng. Thông báo đã đọc xong được giải phóng cả khi cửa sổ đang ẩn; không ghi giả rằng người dùng đã thấy card. Câu hỏi được trả lời hoặc preview đã đóng sẽ hủy phần audio tương ứng còn chờ. Chỉ kết quả cuối tạo completion narration; lời chào ngắn không sinh thêm hai câu tiến độ. Các mục nói về Python reference bên dưới là phạm vi lịch sử, không phải toàn bộ runtime hiện tại.

## Event contract

Các trường chính: `event_id`, `goal_id`, `kind`, `occurred_at`, `milestone_key`, `summary`, `evidence_refs`, `verified`, source registry refs và sensitivity. Production bổ sung producer identity, sequence, TTL, policy version và trạng thái goal liên quan. Schema đề xuất ở [event.schema.json](../schemas/event.schema.json).

Planner không tự chứng nhận completion. Verifier xuất event khi acceptance checks đạt. Worker-start event cần actual runtime receipt; source milestone cần registry đã đọc/loại trùng. Nguồn external và tool output không được tự phát notification hoặc đổi policy.

## Ma trận mặc định

| Event | Timeline | Audio | Điều kiện |
|---|---|---|---|
| Goal accepted | Goal card | Mặc định im lặng | Milestone chỉ nói khi có ích; không bịa ETA |
| Worker started | Gộp milestone | Có chọn lọc | Worker thực sự chạy và việc phân công đáng biết |
| Sources milestone | Số unique sources + refs | Theo milestone | Nguồn đã đọc, liên quan và dedup; search hits không tính |
| Material finding | Có | Ngắn | Đổi quyết định/kết quả/phạm vi |
| Permission/input required | Action card | Một câu hỏi rõ | Chỉ hỏi điều còn thiếu, không bắt nói lại goal |
| Material blocker | Có, có lựa chọn | Ưu tiên | Ảnh hưởng công việc/need user/estimate |
| Completed | Artifact/state checks | Một câu | Verifier đã xác nhận |
| Epoch/batch/token/skill init | Diagnostic log | Im lặng | User hỏi chi tiết thì dùng status path |
| Retry được tự xử lý | Diagnostic log | Im lặng | Escalate khi có tác động/không phục hồi được |
| Heartbeat không đổi | Không | Im lặng | Không “vẫn đang làm” định kỳ |
| User hỏi “xong chưa?” | State hiện tại | Trả lời | Không cần chờ milestone mới |

Số nguồn nên thể hiện cả chất lượng và phạm vi. Runtime hiện đếm URL HTTPS duy nhất do provider trả trong source/citation metadata; đó không phải bằng chứng đã đọc toàn văn hoặc các nghiên cứu độc lập. Không gọi search hits là tài liệu đã đối chiếu. Source count là dữ kiện phụ, kết luận hữu ích mới là mục tiêu.

## Quy tắc thời điểm

1. Người dùng đang nói giữ quyền phát biểu. TTS hiện tại bị ngắt/flush phù hợp; thông báo mới chờ.
2. Quiet mode/focus/quiet hours có thể giữ timeline và action card, không tự nói. Câu hỏi status chủ động của người dùng có thể được trả lời.
3. Routine events coalesce trong cửa sổ; cooldown/quota theo preference. Không phát nhiều worker-start messages liên tục.
4. Permission, blocker, completion vượt routine cooldown; vẫn tôn trọng mic/user speech và quiet mode.
5. Dedupe theo goal + kind + milestone/event key; lưu acknowledgement tách UI và audio. Restart không làm phát lại toàn bộ history.
6. Trước khi phát một event đã chờ, kiểm tra nó còn đúng không. Blocker đã giải quyết hoặc source milestone cũ nên được bỏ/gộp; không đọc backlog nối đuôi.
7. Không làm tròn completion, ETA hoặc phần trăm mà không có mô hình ước lượng. Nếu không biết thời gian, nói đúng điều chưa biết.

## Reference core trong repo

[notifications.py](../src/nora_policy/notifications.py) minh họa rule gate và SQLite acknowledgement ledger. Cooldown routine **45 giây**, source milestone tối thiểu **3 nguồn** là cấu hình ví dụ. CLI `replay` chỉ mô phỏng acknowledgement và in JSON, không có TTS hoặc background dispatcher.

Lõi đã có: lọc technical/heartbeat, gate bằng evidence cho worker/finding/sources/completion, dedup source IDs, quiet mode, defer khi user nói, gate từng channel, lưu delivery markers qua restart.

Lõi **chưa có**: signed/authenticated producers, coalescing queue/outbox, event expiry, kiểm tra goal state khi dequeue, global cross-goal audio arbitration, source-content canonicalization, sensitive-text redaction hoặc actual TTS acknowledgement. Cooldown đang theo từng goal; dispatcher production phải áp thêm global policy. `verified` là trusted fixture input, không phải cơ chế chống giả mạo.

Nếu process chết sau khi speaker phát tiếng nhưng trước khi ack được ghi, lần resume vẫn có thể nói lại. SQLite không tạo exactly-once audio; cần thiết kế giảm lặp theo session/sequence và chấp nhận cửa sổ không xác định. Ưu tiên không che mất câu hỏi cấp quyền hoặc báo sai completion.

## Ví dụ trải nghiệm

Người dùng: “Nora, đọc folder tài liệu này, tìm nguồn mới và tạo báo cáo vào Obsidian.”

NORA: “Tôi sẽ đối chiếu tài liệu và lưu báo cáo vào vault đã chọn.” Sau đó chỉ cập nhật khi có milestone có ích: “Tôi đã đối chiếu tài liệu nền tảng; quyền model wake word đang làm thay đổi lựa chọn.” Nếu cần grant: “Tôi cần quyền ghi note trong folder NORA của vault này.” Khi verifier xác nhận: “Báo cáo đã lưu; tôi đã kiểm tra các file và liên kết.”

Không phát “khởi tạo skill”, “đang suy nghĩ bước 4”, log từng tool, hoặc từng epoch. Nếu người dùng nói “ngừng nói”, chỉ dừng audio. Nếu nói “hủy việc này”, runner nhận cancellation riêng và hiển thị phần đã hoàn tất.

## Đánh giá policy

Đo useful-update precision bằng người đánh giá, omission của blocker/action card, false-completion rate, số lần cắt ngang, duplicate rate sau crash, thông báo nhạy cảm đọc ở chỗ công cộng và status freshness. Chạy task dài có một trăm technical events nhưng ít material events, nhiều goal đồng thời và người dùng đổi preference giữa chừng. Tối ưu theo useful updates trên mỗi task, không theo số câu trợ lý phát ra.
