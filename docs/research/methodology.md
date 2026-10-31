# Phạm vi và cách đọc bằng chứng

Ngày kiểm tra: **31/10/2026**, múi giờ Asia/Saigon. Đây là technical scoping review để ra quyết định sản phẩm, không phải systematic review đầy đủ mọi công trình hay benchmark so sánh độc lập.

Hai nhánh nghiên cứu dùng Exa: voice/desktop và durable agents/security. Tổng cộng yêu cầu 71 kết quả tìm kiếm; đó là số kết quả yêu cầu, không phải số nguồn đã đọc hay số bằng chứng độc lập. Các nhánh mở trang gốc để kiểm tra kết luận. Phần đề thi được đối chiếu trực tiếp bằng Firecrawl live fetch và trình duyệt, gồm PDF thể lệ 14 trang.

Source ledger gồm **42 URL**: 20 về voice/desktop, 16 về workflow/security, 4 URL đề thi và 2 tài liệu Microsoft về containment/process management được kiểm tra bổ sung. Có trang chỉ trả shell, hướng dẫn draft và tài liệu lịch sử; trạng thái này được ghi rõ trong JSON. Các URL khác nhau của cùng nhà cung cấp hoặc cùng nội dung không được coi là nhiều xác nhận độc lập.

## Câu hỏi nghiên cứu

1. Làm sao chuyển một yêu cầu tiếng Việt thành hành động có thể kiểm chứng?
2. Khi nào cần workflow có trạng thái, khi nào đủ một tool call?
3. Desktop chạy nền và quyền app/folder cần được thực thi ở đâu?
4. Làm sao nói đúng lúc mà không đọc raw reasoning/log?
5. Cần bằng chứng nào trước khi tuyên bố production-ready và phù hợp LifeMate?

## Tiêu chí chọn nguồn

Ưu tiên đề thi/thể lệ chính thức, OS/framework documentation, protocol specifications, model cards và repository gốc. Vendor engineering reports giúp chọn phương pháp nhưng không chứng minh sản phẩm của họ vượt trội. Benchmark tác giả tự báo được đánh dấu riêng. Không dựa vào bài SEO, danh sách “best tools” hay bài chép lại nguồn khác để ra quyết định kỹ thuật.

Các cụm truy vấn bao phủ voice-agent architectures, Vietnamese local ASR, wake-word licensing, interruptions, desktop permissions, LangGraph/Temporal durability, MCP security, notification policy và agent outcome evaluation. Không giới hạn năm khi tìm nền tảng còn hiệu lực; các hành vi/API dễ thay đổi được kiểm tra theo trang đang đọc. Phiên bản lịch sử/draft không được trình bày như quy tắc stable hiện tại.

## Mức bằng chứng

| Loại | Có thể kết luận | Chưa thể kết luận |
|---|---|---|
| Official specification/documentation | Hành vi/giới hạn được tác giả công bố | Tích hợp NORA đã chạy đúng |
| Original repository/model card | Mã, model, license được công bố | Toàn bộ data rights hoặc benchmark đã được xác minh độc lập |
| Vendor engineering experience | Các failure mode và phương pháp đáng thử | Mọi sản phẩm áp dụng đều đạt cùng kết quả |
| Local observation/test | Hành vi được quan sát trên fixture/cấu hình cụ thể | Tổng quát cho toàn bộ Windows/app/người dùng |
| Design decision/hypothesis | Hướng xây và giả thuyết có thể kiểm tra | Ground truth về thế giới bên ngoài |

## Những gì chưa làm

Chưa đăng ký thi, chấp thuận điều khoản, tải developer kit/dataset, chạy official evaluator, xác nhận hardware/network/time limits, benchmark microphone tiếng Việt, ký installer, audit OS isolation hoặc tích hợp provider production. Chưa xác minh đầy đủ API Zalo speech vì trang developer chỉ trả shell trong lần đọc này. Chưa kiểm tra nhãn hiệu NORA. Chưa pin/cài LangGraph/Tauri hoặc các model audio.

Test trong repo chỉ xác minh reference router/notification policies. Các con số SLO, budget và cỡ mẫu trong báo cáo là **đề xuất để kiểm nghiệm**, không phải thành tích sản phẩm.

## Khả năng kiểm toán

[voice-sources.json](voice-sources.json), [workflow-sources.json](workflow-sources.json), [contest-sources.json](contest-sources.json) và [platform-sources.json](platform-sources.json) giữ URL, loại bằng chứng, claim và giới hạn. [sources.md](sources.md) là bảng tra cứu hợp nhất. [ground-truths.json](../../ground-truths.json) dùng nhãn rõ cho facts, observations, decisions và unknowns.

Các profile Research Synthesist và Software Architect trong [agency-agents](https://github.com/msitarzewski/agency-agents) được dùng để tổ chức phương pháp, tradeoffs và ADR. Hướng dẫn editorial từ [Turnitout-Humanizer](https://github.com/AhmadHassan-BTed/Turnitout-Humanizer) được dùng để biên tập câu chữ; không chạy proprietary engine hoặc AI detector. Các repository này là nguồn phương pháp do người dùng chỉ định, không được tính vào 42 URL bằng chứng kỹ thuật.
