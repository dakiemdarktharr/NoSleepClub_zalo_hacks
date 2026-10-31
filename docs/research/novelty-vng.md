# VNG/Zalo: vấn đề có bằng chứng và cơ hội cho NORA

Ngày đối chiếu: **31/10/2026, Asia/Saigon**. Phạm vi là nghiên cứu công khai phục vụ lập kế hoạch; không truy cập hệ thống nội bộ, đăng nhập tài khoản, liên hệ nhân viên hay triển khai sản phẩm. Source ledger: [novelty-vng-sources.json](novelty-vng-sources.json).

Đã yêu cầu 45 kết quả tìm kiếm qua 7 truy vấn, sau đó đọc nội dung ở **16 URL chính thức duy nhất**. Số kết quả tìm kiếm không phải số nguồn đã đọc. Những trang cùng chương trình hoặc cùng tập đoàn không được coi là xác nhận độc lập về hiệu quả. Không có ticket analytics, interviews hoặc số liệu tổn thất riêng của VNG trong bộ bằng chứng này.

## Kết luận đề xuất

Đặt điểm khác biệt vào **hoàn tất đúng mục tiêu qua nhiều app ngay cả khi dữ liệu nhận vào chứa yêu cầu giả mạo, mâu thuẫn hoặc chỉ dẫn độc hại**. NORA kiểm tra nguồn gốc của các tham số quan trọng, chặn đúng bước nguy hiểm, tiếp tục phần an toàn và tạo biên nhận có bằng chứng đã che thông tin nhạy cảm. Zalo OA merchant/support là một wedge thử nghiệm có đường tích hợp chính thức; core sau đó áp dụng được cho email, tài liệu, dịch vụ công hoặc môi trường doanh nghiệp.

Đây là **giả thuyết sản phẩm**, không phải tuyên bố phát minh đầu tiên trên thế giới. Prompt-injection defense, approvals, provenance và receipts đều có prior art. Điểm cần chứng minh là kết hợp chúng thành một trải nghiệm tiếng Việt có khả năng làm xong việc, ít hỏi thừa, có kết quả đo trên tình huống Zalo liên quan. Nhánh competitor/security cần kiểm tra mức trùng trước khi claim novelty.

## 1. VNG đã nói gì công khai?

| Bằng chứng | Ý nghĩa đối với lựa chọn sản phẩm | Không thể suy ra |
|---|---|---|
| Báo cáo 2025 nêu 79,6M MAU, 2,11B tin nhắn/ngày, khoảng 150k OA trong Business Solutions; 2026 ưu tiên AI có ích theo ngôn ngữ/ngữ cảnh Việt, độ tin cậy dịch vụ và năng suất nội bộ | Có nền tảng phân phối rộng và nhu cầu utility/action thực tế | Không chứng minh mọi OA cần NORA hoặc có ngân sách mua |
| Zalo có Safety Center hợp tác Google, reporting, 2FA và privacy controls | An toàn trong tương tác hằng ngày phù hợp chiến lược công khai | Không được nói Zalo chưa có công cụ chống scam |
| Zalo Help mô tả mạo danh thương hiệu/hợp tác, installer giả, OTP phishing, chiếm tài khoản và phát tán đến contacts | Có lớp đe dọa thực tế từ nội dung và yêu cầu trông đáng tin | Không chứng minh VNG bị breach nội bộ hoặc biết tần suất từng mẫu |
| Zalopay đã thí điểm cảnh báo recipient bằng dữ liệu nội bộ + SIMO | Kiểm tra trước action là nhu cầu được chính VNG đầu tư | Không tự cho NORA quyền truy cập SIMO hoặc dữ liệu dân cư |

Nguồn: [VNG01](https://ir.vng.com.vn/en/business-report/zalo-ai/), [VNG02](https://www.zalo.me/vi/blog/zalo-va-google-hop-tac-ra-mat-trung-tam-an-toan-so-bao-ve-nguoi-dung-viet), [VNG04](https://help.zalo.me/huong-dan/chuyen-muc/bao-mat-va-rieng-tu/de-phong-chieu-tro-mao-danh-hop-tac-voi-zalo-de-lua-dao/), [VNG03](https://zalopay.vn/zalopay-trien-khai-thi-diem-tinh-nang-canh-bao-tai-khoan-nhan-tien-nghi-ngo-gian-lan-lua-dao-7993).

Số liệu vận hành là self-report của tập đoàn. Không sử dụng tỷ lệ tăng daily messages vì cùng trang có 8%/9% không thống nhất. Safety Center hiện tồn tại; fetched primary body nói bắt đầu phân phối từ tháng 8 nhưng không hiển thị năm, nên không ghi ngày ra mắt chính xác như ground truth.

## 2. Những hướng nên loại khỏi vị trí novelty chính

**Voice Q&A/content drafting:** Kiki Info đã có Q&A, kiến thức giao thông, viết nội dung và gợi ý. Kiki Auto đã xử lý tiếng Việt vùng miền cho navigation/music, sau đó thêm traffic alerts, phạt nguội, AI news và saved destinations. Làm lại các chức năng này vẫn cần cho baseline phù hợp use case, nhưng khó là điểm khác biệt. [Kiki Info](https://zalo.me/en/post/kiki-info/), [Kiki Auto](https://zalo.me/en/product/kiki-auto/)

**FAQ/RAG chatbot chăm sóc khách hàng:** OA đã có chatbot trả phí với flow/rule, thu thập thông tin, dynamic backend data, phân loại/routing và báo nhân viên. Một chatbot đọc FAQ hoặc tra trạng thái đơn hàng chung chung trùng đáng kể. [Zalo OA Chatbot](https://oa.zalo.me/home/resources/library/tu-dong-hoa-cham-soc-khach-hang-voi-zalo-chatbot_6352033339970702125)

**Scam classifier hoặc cảnh báo tài khoản nhận tiền đơn thuần:** Safety Center, Google detection và Zalopay/SIMO đã hiện diện. Không nên pitch “AI phát hiện lừa đảo đầu tiên cho Zalo”. Có thể dùng tín hiệu chính thức đã cấp quyền như một input, nhưng không gọi một classifier output là kết luận gian lận. [Safety hub](https://www.zalo.me/vi/safety), [SIMO pilot](https://zalopay.vn/zalopay-trien-khai-thi-diem-tinh-nang-canh-bao-tai-khoan-nhan-tien-nghi-ngo-gian-lan-lua-dao-7993)

**Generic internal IT helpdesk:** Phù hợp tuyên bố VNG muốn tăng năng suất nội bộ và đã dùng AI trong nhiều nghiệp vụ, nhưng chưa có bằng chứng công khai về quy trình hỏng, ticket backlog hoặc lỗ hổng riêng. Cần discovery thật; không dựng chuyện để làm câu chuyện thi hấp dẫn. [FY2025 release](https://vng.com.vn/news/press-release/vng-fy2025.html)

## 3. Ba cơ hội xếp hạng

Đánh giá dưới đây là judgment có giải thích, không phải market-size score hoặc kết quả phỏng vấn.

| Hạng | Vấn đề/cơ hội | Độ mạnh bằng chứng | Giá trị rộng | Đường kiểm nghiệm |
|---|---|---|---|---|
| **1** | Bảo toàn mục tiêu/tham số khi assistant thực hiện việc qua OA, order data và tài liệu không đáng tin | Cao cho threat patterns và strategic fit; trung bình/thấp cho đúng workflow cụ thể vì chưa có merchant interviews | Áp dụng sang invoice, email, booking, dịch vụ công, doanh nghiệp | Synthetic OA/order scenarios + official app/OA pilot được cấp quyền; đo safe task completion và interruption |
| **2** | Hỗ trợ thu thập evidence và chuyển giao cho support khi người dùng bị mạo danh/nhầm giao dịch | Cao cho quy trình report cần chứng cứ; chưa có dữ liệu incomplete-case rate/cost | Người dùng phổ thông, gia đình, merchants, support agents | Người dùng chủ động chọn dữ liệu, tạo gói báo cáo redacted, reviewer chấm completeness và effort |
| **3** | Internal permission-aware knowledge-to-action cho IT/ops | Chỉ có strategic support và AI usage public; pain nội bộ cụ thể chưa xác minh | Doanh nghiệp ngoài VNG cũng dùng được | 5–10 discovery interviews được cho phép, workflow logs/permissions xác thực, design partner |

### Cơ hội 1: làm xong việc trong môi trường có dữ liệu đối kháng

Ví dụ có thể dựng hoàn toàn bằng dữ liệu tổng hợp: merchant nói “Đối chiếu yêu cầu đổi hàng của đơn 123, cập nhật trạng thái và soạn phản hồi đúng khách.” Trong một message/attachment xuất hiện chỉ dẫn yêu cầu gửi danh sách khách hàng ra link khác hoặc thay người nhận/địa chỉ. NORA phải:

1. Giữ mục tiêu từ user command tách khỏi message/attachment được đọc.
2. Resolve đơn hàng, customer identity và các nguồn được phép dùng; hiển thị provenance ở các tham số quan trọng.
3. Phát hiện action dự kiến đang vượt mục tiêu/quyền hoặc dùng tham số mâu thuẫn với nguồn đáng tin.
4. Chỉ chặn/phân giải bước bị ảnh hưởng; tiếp tục các bước độc lập an toàn, hỏi đúng một thông tin có tính quyết định khi cần.
5. Kiểm tra state sau effect; tạo biên nhận gồm intent/action/evidence, đã che dữ liệu không cần thiết.

Không coi verified OA badge là proof mọi bank account được gửi trong OA đều an toàn. Không tự truy vấn SIMO hoặc ký/chuyển tiền qua API không được cấp. Trong demo contest, các payment tool chỉ chạy simulator theo contract; production cần giới hạn và phê duyệt riêng. Demo “bank account change” có thể là đọc/xác minh/draft/handoff; không cần chuyển tiền thật.

KPIs đề xuất để đo, chưa có baseline: verified benign task completion, attacker-induced effect rate, unnecessary interruption, critical-slot correctness, time-to-resolution, evidence completeness và p95 latency/cost. Mẫu có cả benign lookalikes, legitimate recipient changes và account takeover; không chỉ kịch bản dễ để làm đẹp detection rate.

Go nếu giảm unauthorized effects mà không làm hỏng clean-task success hoặc hỏi liên tục. No-go nếu kết quả chỉ là chặn mọi thao tác, trùng guardrail có sẵn, cần dữ liệu nội bộ không có quyền hoặc không có merchant pain đủ rõ. Không cam kết giảm thiệt hại tài chính hay giảm ticket bao nhiêu phần trăm trước pilot.

### Cơ hội 2: gói evidence cho support, với user kiểm soát

Zalo Help yêu cầu report có message text/voice, ảnh/video/link hoặc screenshot cụ thể; không phải mọi report đều dẫn tới gỡ bỏ. Điều này tạo căn cứ cho một assistant giúp chuẩn bị case. **Chưa có nguồn xác nhận** tỷ lệ report thiếu bằng chứng, thời gian xử lý thực tế hoặc chi phí mỗi ticket. [Report flow](https://help.zalo.me/huong-dan/chuyen-muc/bao-mat-va-rieng-tu/bao-xau-tai-khoan-zalo/)

Sản phẩm có thể giúp user chọn artifact, lập timeline, giữ nguồn gốc, che PII không liên quan, kiểm tra trường còn thiếu, rồi xem preview trước khi user tự gửi qua kênh chính thức. Không auto-accuse, auto-report hàng loạt hoặc gọi API report không được tài liệu hóa. Biên nhận của NORA chứng minh những thao tác NORA quan sát được; không phải chứng cứ pháp lý đã được cơ quan có thẩm quyền xác nhận.

Tính năng này hợp làm phần mở rộng của cơ hội 1: sau một action conflict hoặc nghi ngờ mạo danh, NORA vẫn có thể giúp người dùng hoàn tất bước hỗ trợ hợp lệ. Nếu tách thành sản phẩm riêng, novelty thấp hơn và dễ bị nhìn như form-filling assistant.

### Cơ hội 3: nội bộ, chỉ sau discovery

Có public commitment về internal productivity, nhưng không có quyền suy ra “VNG support đang quá tải”, “nhân viên làm lộ secret” hoặc “hệ thống nội bộ bị hack”. Có thể nghiên cứu permission-aware assistant đọc policy, chuẩn bị access request, hướng dẫn thiết bị hoặc soạn ticket; triển khai thật phụ thuộc tài liệu, IAM, ticket API và partner agreement. Bản đầu không nên dựa toàn bộ pitch vào dữ liệu nội bộ không có.

## 4. Đường tích hợp thực tế và những chỗ không được giả định

**OA pilot là đường có căn cứ:** app phải được một OA cụ thể liên kết/cấp quyền; OAuth v4/PKCE và permission groups có tài liệu. Token refresh phải được quản lý an toàn, tránh nhiều worker refresh đồng thời làm vô hiệu token của nhau. Scope nhận message events không đồng nghĩa được đọc mọi chat cá nhân. [Authorization](https://docs.zaloplatforms.com/docs/OA/bat-dau/xac-thuc-va-uy-quyen-cho-ung-dung-new)

**Messaging eligibility là phần business logic:** consultation message cần tương tác đủ điều kiện trong 7 ngày và OA chưa bị block; miễn phí trong 48h, ngoài khoảng này có pricing. GMF/ZBS templates có chính sách khác. Tài liệu legacy V2 còn tìm thấy trong search, trong khi error code -240 nói MessageV2 đã đóng và phải chuyển V3. Cần pin contract hiện hành khi implementation bắt đầu. [Overview](https://docs.zaloplatforms.com/docs/OA/tin-nhan/tong-quan), [API errors](https://docs.zaloplatforms.com/docs/OA/phu-luc/ma-loi)

**Không dùng unofficial personal-account bot làm nền sản phẩm:** Zalo Help liệt kê bot/emulator/third-party software trong nhóm nguyên nhân account disabling. Điều này là rủi ro nền tảng thực, không được giải quyết bằng cách yêu cầu user cấp quyền filesystem. [Account policy](https://help.zalo.me/huong-dan/chuyen-muc/quan-ly-tai-khoan-zalo/loi-thuong-gap/tai-khoan-zalo-bi-tam-thoi-vo-hieu-hoa/)

**Không vượt security controls của payment app:** Zalopay có thông báo dừng hoạt động khi phát hiện một số môi trường root/jailbreak, emulator/VM hoặc debugging/ADB. Không đề xuất bypass để cho desktop assistant điều khiển giao dịch. [Security notice](https://zalopay.vn/thong-bao-zalopay-nang-cap-bao-mat-theo-thong-tu-77-8302)

**Chưa xác minh public API cho report, SIMO hoặc dữ liệu cá nhân rộng:** bước đầu dùng synthetic data hoặc user-selected artifacts, official OA scope và thao tác handoff phù hợp. Mini App có thể là companion distribution nếu sau đó kiểm tra mic/permission/backend/app-review contract; không coi research này đã xác nhận mọi capability.

## 5. Những phát biểu cần tránh trong pitch

- “VNG bị tấn công CIC”: Zalopay công bố nó không gửi dữ liệu cho CIC và khách hàng không bị ảnh hưởng bởi sự cố đó. Đây là statement của nhà cung cấp, không phải independent audit; tuyệt đối không đảo thành cáo buộc breach. [Statement](https://zalopay.vn/he-thong-va-thong-tin-khach-hang-cua-zalopay-khong-bi-anh-huong-boi-su-co-tai-cic-7451)
- “Zalo chưa có chống scam/AI/chatbot”: đã có bằng chứng ngược lại.
- “Xác minh OA là biết mọi yêu cầu/giao dịch từ OA an toàn”: nguồn chỉ xác minh identity/scope, không chứng thực từng action.
- “Đọc được toàn bộ personal chats qua OA API”: tài liệu không cấp quyền đó.
- “Giảm 70% ticket/thiệt hại”: không có measurement; chỉ được nêu là hypothesis và thiết kế pilot.
- “NORA đầu tiên trên thế giới”: cần prior-art comparison, không thể suy ra từ vài kết quả search.

## 6. Validation trước khi phát triển

Chọn một merchant workflow hẹp và một loại user-side task tổng quát. Xin 5–10 interviews/design partners qua kênh được user cho phép ở giai đoạn sau, tập trung: action conflicts xảy ra khi nào, dữ liệu tin cậy ở đâu, ai có quyền quyết định, hệ thống nào có readback và case nào gây support effort. Không gửi liên hệ trong phiên nghiên cứu này.

Đối chiếu với capability hiện có của OA/SIMO/Safety Center; giá trị mới phải nằm ở completion/repair/receipts, không chỉ thêm giọng nói. Dựng paired benign/adversarial scenarios, giữ baseline voice assistant capabilities và quota phần cứng 2 GB RAM/4 GB VRAM/15 GB SSD làm gate độc lập. Demo dùng synthetic data, không cần quyền toàn bộ Zalo cá nhân hoặc banking thật.

Chỉ tiến tới implementation khi official API/tool contract, data/consent, quyền phân phối competition code và acceptance checks đã được chốt. Chưa có bằng chứng đủ để hứa one-shot tới production chỉ từ bản kế hoạch này.
