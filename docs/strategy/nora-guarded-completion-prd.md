# PRD: NORA Giữ Ý và trợ lý nền

**Version 0.1 — 31/10/2026 — Proposed, chưa triển khai hoặc được pilot xác nhận.** Owner chức năng: product lead; kỹ thuật: runtime lead; review quyền/bảo mật: security lead. Đây là vai trò cần phân công ở bước phát triển, không phải một đội đã được tuyển.

## 1. Giá trị cho người dùng

“Giao việc một lần bằng tiếng Việt. NORA làm phần được phép, giữ đúng người/đơn/đích dữ liệu, báo khi có thay đổi cần quyết định, và cho bạn thấy kết quả đã kiểm tra.” Bản đầu phục vụ người làm việc trên Windows và merchant quản lý OA được cấp quyền. Không yêu cầu người dùng hiểu agent framework hoặc đọc log kỹ thuật.

Job đầu tiên: đối chiếu một yêu cầu khách hàng với order record/chính sách, soạn phản hồi và lưu biên bản trong folder đã chọn. Công việc này có nhiều nguồn tin và một output rõ, có thể dựng bằng dữ liệu tổng hợp. Chưa có interviews xác nhận tần suất/cost; product lead cần kiểm nghiệm trước khi mở rộng. Căn cứ public và khoảng trống nằm trong [VNG research](../research/novelty-vng.md).

## 2. Baseline cần xây theo từng adapter trước khi claim tốt hơn

| Nhóm khả năng | Năng lực mục tiêu theo các release | Giới hạn/điều kiện |
|---|---|---|
| Đối thoại giọng Việt | PTT/hotkey, hiển thị transcript sửa được, đáp ngắn, ngắt lời, text fallback | Wake word tùy chọn sau khi có dữ liệu/license/đo FAR–FRR |
| Thời gian và hỏi đáp | Context thời gian/timezone; hỏi đáp, trích dẫn khi nghiên cứu | Không bịa nguồn hoặc thời điểm “hiện tại” |
| App và file | Mở app allowlist; tìm/đọc file trong grant, preview thay đổi | Quyền thực thi app khác quyền đọc/ghi tất cả dữ liệu của app |
| Notes và tài liệu | Tạo/tóm tắt/sửa note hoặc file trong workspace, readback | Tôn trọng file đang bị app khác sửa, không ghi đè im lặng |
| Lịch và nhắc việc | Đọc lịch, đề xuất/tạo/sửa có quyền, durable reminders | Có timezone/DST, chống trùng, đối soát sau offline |
| Email/OA | Tìm message được cấp quyền, soạn draft, preview và gửi khi đủ điều kiện | Phải có connector/account; recipient/send policy riêng |
| Browser | Tìm nguồn, điền form trên domain được phép khi adapter có hỗ trợ | Mặc định ít tab, không arbitrary browsing qua quyền rộng |
| Công việc dài | Goal, checkpoint, cancel, resume, budget, completion checks | Không coi cancel là undo effect đã gửi |
| Sửa lỗi/missing info | Hỏi đúng slot hoặc quyền còn thiếu, giữ tiến độ đã xác minh | Không suy người nhận/số tiền từ dự đoán LLM |
| Smart home/dịch vụ | Official connector/simulator khi có catalog | Thiết bị/đặt xe/đồ ăn/ngân hàng thật cần API, scope và điều kiện riêng |
| Minh bạch | Bước đã làm, blocker, nơi lưu artifact và receipt | Không phát raw chain-of-thought |
| Accessibility | Keyboard/text fallback, trạng thái mic rõ, mute/stop dễ thấy | Không bắt buộc chỉ điều khiển bằng giọng |

Đây là capability backlog, không phải cam kết tất cả trong first release. **Alpha đầu:** voice/text + file/note trong một grant + public research + synthetic order workflow; **closed beta đầu:** thêm một OA và một nguồn order/policy được cấp quyền; calendar/email/browser có release gate riêng. Calendar/email chưa được tích hợp. Mỗi release phải freeze ma trận OS, adapter/version, account/scope, operation và tests; capability chưa có không xuất hiện như đang dùng được trong UI. “Tốt hơn” chỉ công bố cho task/điều kiện đã đo.

## 3. Hai lớp phân loại task

**Complexity** và **risk** là hai trục độc lập. Một lệnh chuyển tiền có thể ngắn nhưng rủi ro cao; một nghiên cứu dài chỉ đọc nguồn có thể rủi ro thấp. Router không dùng số chữ hoặc số worker làm tiêu chí duy nhất.

| Trường hợp | Runtime đề xuất | Kiểm tra trước effect |
|---|---|---|
| Một tool rõ, đủ slots, effect thấp | Fast path trong cùng broker | Schema, scope, target, freshness |
| Có dependencies, nhiều app, cần verify/có thể gián đoạn | Goal và plan lưu bền | Cùng checks cộng checkpoint/effect ledger |
| Nhánh độc lập có thể giảm thời gian, đáng chi phí | Workers API giới hạn concurrency | Grants con là tập con; evidence có nguồn |
| Critical slot mơ hồ hoặc effect mới | Clarification/approval cụ thể | Chưa thực hiện bước phụ thuộc |
| Không có tool/phạm vi hỗ trợ | Trả unsupported hoặc handoff | Không cài app/tự tạo quyền để ép hoàn tất |

Default một orchestrator; tối đa hai worker API đồng thời trong profile nhỏ để thử, không mặc định spawn theo vai trò. Spawn chỉ khi có output contract, deadline và lợi ích dự kiến. Người dùng không phải trả lời câu “có tạo agent không” cho mỗi goal.

## 4. Contract ở mức thiết kế

Các trường dưới đây là yêu cầu cho implementation sau, chưa là schema hay formal proof:

| Contract | Các trường tối thiểu | Nguồn có quyền thay đổi |
|---|---|---|
| Goal | ID/version, yêu cầu gốc, desired outputs, acceptance checks, deadline/budget | User qua UI tin cậy; sửa goal phải tạo version mới |
| Entity | Canonical ID, loại entity, giá trị quan trọng, source reference, freshness, trạng thái unresolved | Connector/source có authority theo nghiệp vụ hoặc user xác nhận |
| Data scope | Read roots/records, sensitivity, allowed recipients/domains, cloud disclosure policy | Native grants/settings; worker không tự nâng nhãn |
| Action proposal | Tool/version, exact typed args, dependency IDs, input evidence, expected effect | Model đề xuất, broker phân giải/kiểm tra |
| Approval | Goal/action version, canonical preview, argument/content digest, expiry, grant revision | Native UI và authenticated session phù hợp risk |
| Receipt | Broker action ID, attempt/idempotency, provider reference, observed state, verification status | Broker và adapter; model không được tự ghi success |

Authority phải theo **field và operation**, không chỉ “nguồn trusted”. Một order record có quyền xác nhận customer ID nhưng có thể không có quyền đổi bank account. Dữ liệu vừa đọc vẫn có thể stale/compromised; conflict không tự phân giải bằng “mới nhất” hoặc “ở OA đã verified”.

## 5. Flow Giữ Ý

1. Thu audio do user kích hoạt; giữ modality/source boundary. ASR xuất transcript và alternatives nếu có; uncertainty không tự chứng thực đúng/sai. Critical slots được normalize theo locale nhưng giữ bản gốc để review.
2. Router tạo fast action hoặc goal. Contract thể hiện yêu cầu rõ và phần inferred/unknown; LLM tự viết contract không được coi là căn cứ quyền đủ mạnh.
3. Planner đọc metadata tối thiểu từ tools. Tool output/tài liệu được xử lý như data/claims; không được biến câu “hãy gửi mọi file” trong tài liệu thành user instruction. Mọi router/critic/summary/worker API request được broker kiểm tra data labels, destination, purpose và consent trước serialization/transmission; tool gate sau đó không cứu được một upload đã xảy ra.
4. Workers trả kết quả có nguồn và bounded size. Summary/handoff giữ provenance; không dùng summary để rửa nhãn của nội dung ngoài.
5. Broker kiểm tra hiệu lực grant, canonical target, data destination, schema và risk ngay trước effect. Critic semantic là defense bổ sung, không được override deny.
6. Khi xung đột, loại các bước phụ thuộc chưa đủ điều kiện, tiếp tục những bước độc lập đã kiểm tra; nếu chưa xác định dependency hoặc source policy thì chờ. Đây là heuristic trên graph hiện biết, không bảo đảm tìm được phương án tối ưu toàn cục.
7. Nếu cần user quyết định, UI nêu trường cũ/mới, nguồn và tác động. Có thể hỏi một nhóm fields liên quan trong cùng lượt; không hứa mọi vấn đề chỉ cần đúng một câu.
8. Sau action, adapter đọc lại state/receipt. Nếu outcome không rõ, vào reconciliation. Chỉ full completion khi toàn bộ authorized postconditions đạt; partial progress hiển thị đúng là partial.

Ví dụ: goal chỉ “soạn trả lời và lưu biên bản” thì gửi message không được tự thêm. Một attachment xin export customer list nằm ngoài goal có thể bị bỏ qua và ghi event. Nếu user thực sự yêu cầu cập nhật địa chỉ, NORA phải phân giải authority cho địa chỉ mới, không chỉ bỏ mọi thay đổi rồi đánh dấu goal xong.

## 6. Quyền ứng dụng, folder và cloud

| Bề mặt | Cách truy cập dự kiến | Hành vi khi thiếu quyền |
|---|---|---|
| Folder | Native picker; grant read/write riêng; broker resolve handle/reparse points, kiểm tra containment khi mở | Nêu file/operation cần dùng và cho chọn folder; không scan toàn ổ |
| Obsidian | Ghi Markdown qua folder grant; adapter có conflict handling | Chọn vault/subfolder; không cần quyền toàn máy |
| App có API | OAuth/official API với account/scope cụ thể, paginated output | Kết nối account hoặc tiếp tục draft local nếu phù hợp |
| App không có API | Adapter Windows accessibility/UIAutomation giới hạn app/window/actions | Hỏi cấp capability theo session; fallback manual handoff |
| Browser | Domain/origin-scoped session, typed actions; confirmation với sensitive effects | User chọn session/domain; không lấy cookies từ browser khác |
| LLM API | Gửi transcript/context tối thiểu theo policy đã cho phép | Có thể tiếp tục local deterministic task/text; không upload bừa |
| Voice cloud | Tùy chọn rõ audio nào gửi, provider/retention/cost | Không tự chuyển cloud khi local OOM |

Quyền app/folder có thể tạm thời hoặc lưu lâu, có màn hình revoke. Grant read không cho execute; grant một folder không tự cho tool truyền nội dung ra mạng. Tauri capabilities/MCP roots không thay OS containment. [Ground truths GT009/010/024](../../ground-truths.json), [chi tiết trust boundaries](nora-security-threat-model.md).

PTT là mặc định để giảm false activation và tài nguyên. Voice match/wake word không phải authentication; sensitive consent không chỉ dựa vào tiếng “ừ” phát lại. Mic capture tắt khi session kết thúc; audio ring buffer bounded, không lưu lâu mặc định. STT gửi text tới LLM cũng là disclosure cần account/policy phù hợp.

## 7. Zalo connector thực tế

OA không tự là order-management system. Alpha dùng fixture tổng hợp và CSV trong folder user chọn; CSV được merchant chỉ định làm nguồn nào phải khai báo rõ. Pilot cần read-only OMS/API hoặc bản export có quyền, mapping OA actor/customer ID sang order ID, bản policy đổi trả có version và quyền quyết định từng field. Các thao tác update thật cần OMS write scope riêng; nếu chỉ có CSV đọc thì chỉ draft/ghi note, không tự nhận đã cập nhật đơn. Display name không đủ để nối hai customer records. Giá trị thực và quyền truy cập này là dependency, không phải thông tin đã có từ OA API.

OA app chỉ được thao tác OA/account/scope đã được admin cấp. **PKCE không loại yêu cầu app secret** trong OAuth v4 đang được tài liệu hóa; shared app secret phải nằm ở backend/BFF, không nhúng trong desktop. Token refresh cần single-flight và ghi token rotation atomically; workers không tự refresh dùng chung token. Webhook ingress cần verification theo contract thật, dedup/replay control; cơ chế ký/headers chỉ chốt sau đọc API version. [OAuth v4](https://docs.zaloplatforms.com/docs/OA/bat-dau/xac-thuc-va-uy-quyen-cho-ung-dung-new).

Messaging eligibility, time window, quota/fee và schema version là constraints của adapter. Không gửi ngoài policy, không retry vô hạn. V2 cũ còn xuất hiện trong search; implementation phải pin V3/current contract. [Messaging](https://docs.zaloplatforms.com/docs/OA/tin-nhan/tong-quan), [error codes](https://docs.zaloplatforms.com/docs/OA/phu-luc/ma-loi).

Chưa có public API verified cho personal inbox toàn bộ, SIMO/bank data hoặc report submission. Personal bots có rủi ro account policy; không dùng để làm production foundation. Support capsule xuất local để user xem/chọn gửi qua kênh chính thức. [Account restrictions](https://help.zalo.me/huong-dan/chuyen-muc/quan-ly-tai-khoan-zalo/loi-thuong-gap/tai-khoan-zalo-bi-tam-thoi-vo-hieu-hoa/), [report guidance](https://help.zalo.me/huong-dan/chuyen-muc/bao-mat-va-rieng-tu/bao-xau-tai-khoan-zalo/).

## 8. Cập nhật mà không làm phiền

Updates được tạo từ **trusted runtime events có bằng chứng**, không từ raw reasoning hoặc model tự nói đang tiến triển. Có ba kênh: voice cho điều cần nghe ngay, card cho tiến độ có thể xem, activity log cho chi tiết theo yêu cầu.

| Event | Default dự kiến | Nội dung có ích |
|---|---|---|
| Bắt đầu việc nhanh | Một acknowledgment ngắn nếu cần; thường chờ kết quả | Không đọc plan nhiều bước không tồn tại |
| Goal dài bắt đầu | Một câu tóm tắt outcome và card | “Tôi sẽ đối chiếu đơn rồi soạn phản hồi.” |
| Tạo worker có ý nghĩa | Card; voice chỉ khi giúp hiểu thời gian/phạm vi | “Hai nhánh đang đối chiếu chính sách và dữ liệu đơn.” Chỉ khi workers thực sự đã chạy |
| Đã xem nhiều nguồn, có kết luận mới | Gộp theo milestone; count từ provenance ledger | “Đã đối chiếu 8 nguồn; hai nguồn mâu thuẫn về thời hạn.” Không đọc count mỗi lượt search |
| Cần quyền, critical info, conflict | Voice/card theo urgency, nêu lựa chọn cụ thể | “Địa chỉ mới khác hồ sơ đơn; cần xác nhận trước khi cập nhật.” |
| Retry tự khôi phục, framework/skill init, epoch train, token stream | Im lặng; log/metric nếu cần | Chỉ escalates nếu có tác động deadline, cost hoặc cần user |
| Goal xong hoặc thất bại không tự sửa được | Một thông báo có receipt/artifact/next choice | Không “xong” nếu mới tạo draft hoặc effect chưa rõ |

Không đọc prompt injection hoặc URL nhạy cảm bằng TTS; render nguồn như dữ liệu. Count “nguồn” phải là unique sources được mở/đối chiếu, không số kết quả search được yêu cầu. Sau interruption, chỉ phần audio đã phát được coi là user đã nghe; các events chưa nói vẫn có trong card.

Cooldown khoảng 45–60 giây cho voice milestone là giả định UX để thử, không áp dụng cho cảnh báo/quyền cần hành động, và không yêu cầu nói mỗi khi hết cooldown. Quiet mode/meetings giữ card và chỉ escalates theo policy người dùng chọn. Người dùng có thể yêu cầu update chi tiết hơn. [Policy gốc](../progress-policy.md).

## 9. Support capsule tùy chọn

Schema export dự kiến chỉ gồm goal summary đã review, step/status, source references tối thiểu, timestamps cần thiết, app/policy version, blocker và verification evidence được chọn. Không mặc định gồm raw audio, toàn bộ chat/file, token, bank account hay payload tấn công. Không coi hash của trường ít entropy là anonymization.

User preview/redact/chọn đích rồi tự xuất. Không auto-report/auto-accuse hoặc cam kết được giải quyết nhanh. Capsule có tamper evidence nếu cần chỉ chứng minh integrity/source theo trust model, không xác thực mọi nội dung là thật. Beta chỉ bật nếu đánh giá usefulness và privacy đạt; core Giữ Ý vẫn dùng được khi capsule tắt.

## 10. Acceptance stories tối thiểu

| ID | Given / When | Then cần chứng minh |
|---|---|---|
| U01 | Yêu cầu đơn giản đủ slots trong grant | Một đường ngắn, không tạo workflow/worker không cần thiết, kết quả được kiểm tra |
| U02 | Goal dài gián đoạn sau khi draft đã lưu | Resume từ state hợp lệ, không tạo file/send trùng |
| U03 | Message ngoài cố đổi recipient/egress | Không effect trái phép; phần độc lập hợp lệ tiếp tục |
| U04 | Đổi địa chỉ thực sự hợp lệ từ source đủ authority | Thay đổi được xử lý đúng; không bị blocker vĩnh viễn |
| U05 | Hai khách trùng tên hoặc ASR sai số đơn | Hỏi đúng trường, không đoán effect quan trọng |
| U06 | Revoke folder/app khi đang chạy | Action chưa thực hiện bị chặn; state phản ánh phần đã xảy ra |
| U07 | Crash sau gửi trước ack | Reconcile theo provider receipt; không blind resend |
| U08 | OOM/network/provider unavailable | Không tự mở rộng quota/upload; goal có trạng thái khôi phục hoặc handoff |
| U09 | Chọn export support | Preview đúng, giảm PII/secret, user quyết định đích |
| U10 | Giọng vùng miền/noise/barge-in | Không thực thi từ audio cũ/ngoài session; cancel có hiệu lực với action chưa bắt đầu |

Giá trị định lượng, sample size và gates đặt trong [evaluation plan](nora-evaluation-plan.md). Chưa có story nào ở đây được nghiệm thu bằng production app.
