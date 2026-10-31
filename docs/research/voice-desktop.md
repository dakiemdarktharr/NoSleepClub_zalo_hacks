# Giọng nói và desktop runtime cho NORA

Ngày kiểm tra nguồn: **31-10-2026**. Phạm vi: trợ lý tiếng Việt chạy nền trên Windows, nhận một yêu cầu bằng giọng nói, thực thi trong ứng dụng hoặc thư mục đã được cho phép, và giao tiếp khi có điều đáng báo.

Đây là nghiên cứu kiến trúc. Chưa cài integration, tải model, đo benchmark hay xác nhận một bản phát hành đủ điều kiện production. Các con số mục tiêu phía dưới là đề xuất để kiểm nghiệm. Danh mục chứng cứ ở [voice-sources.json](voice-sources.json) gồm 20 URL đã mở: 19 trang có nội dung dùng được và một trang Zalo chỉ trả shell/cookie.

## Quyết định đề xuất

Chọn **Windows-first, ứng dụng tray chạy trong phiên đăng nhập của người dùng**, với Tauri 2 làm giao diện và Rust làm broker kiểm soát quyền. Đặt audio runtime trong process riêng. Bắt đầu bằng **STT → orchestrator → TTS**, giữ hotkey hoặc push-to-talk làm cách kích hoạt cơ bản. Wake word là tính năng opt-in, được phát hành sau khi kiểm tra tiếng Việt và quyền thương mại.

Đây là suy luận thiết kế từ nhu cầu quan sát transcript, kiểm soát hành động và quản lý quyền; không phải kết luận rằng một framework thắng mọi benchmark. Tài liệu OpenAI mô tả cả chained pipeline và speech-to-speech, đồng thời yêu cầu đánh giá kết quả tác vụ lẫn chất lượng hội thoại. [V14](https://developers.openai.com/api/docs/guides/voice-agents)

## Chọn desktop shell và cách chạy nền

| Phương án | Phù hợp | Đánh đổi |
| --- | --- | --- |
| Tauri 2 + Rust broker | Desktop có công cụ truy cập máy, UI tách khỏi quyền hệ thống | Đội cần Rust; sidecar và custom command phải tự kiểm tra quyền |
| Electron | Đội mạnh TypeScript, cần hệ sinh thái desktop quen thuộc | Phải cập nhật Chromium/Node, khoá renderer, kiểm tra IPC và remote content |
| Web app thuần | Thử nghiệm hội thoại, dashboard hoặc client phụ | Không thay thế được desktop broker cho folder và app local |

Tauri có system tray, plugin autostart và đóng gói external binary. Tuy nhiên, capability của Tauri chủ yếu giới hạn lời gọi từ WebView vào core/plugin. Tài liệu nêu rõ cơ chế này không bảo vệ trước Rust code độc hại, scope quá rộng hoặc kiểm tra sai trong custom command. Capabilities của cùng một window có thể được hợp nhất, nên thêm một file grant rộng có thể mở rộng quyền ngoài dự kiến. [V01](https://v2.tauri.app/security/capabilities/), [V02](https://v2.tauri.app/develop/sidecar/)

Electron vẫn là lựa chọn hợp lệ nếu giảm rủi ro triển khai cho đội. Cần bật context isolation và process sandbox, kiểm tra sender của IPC, hạn chế navigation, không expose Electron API cho nội dung không tin cậy, không cho remote content dùng Node integration. Chọn Tauri không làm các kiểm tra tương đương trở nên không cần thiết. [V03](https://www.electronjs.org/docs/latest/tutorial/security)

Autostart phải do người dùng bật và có thể tắt trong settings. Tray luôn có trạng thái microphone, nút mute, pause, quit và đường dẫn tới lịch sử hành động. Không dùng interactive Windows Service làm UI chính: Microsoft nói services không trực tiếp tương tác với người dùng từ Windows Vista và chạy trong Session 0. Một service đặc quyền, nếu sau này thật sự cần, phải tách khỏi UI và có IPC với ACL chặt. [V04](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services)

## Pipeline giọng nói

Luồng đề xuất là capture → xử lý echo/noise → VAD → endpointing → STT → command có cấu trúc → workflow → update policy → TTS. Giữ audio, hội thoại và thực thi tác vụ thành ba vòng đời riêng. Người dùng vẫn có thể nói khi một goal dài đang chạy.

VAD chỉ phát hiện có tiếng nói; endpointing quyết định lượt nói đã hết. Khoảng lặng ngắn trong câu tiếng Việt, tên tiếng Anh hoặc suy nghĩ giữa câu không được tự động xem là kết thúc. Silero VAD có runtime ONNX, hỗ trợ 8/16 kHz và license MIT theo repository. Khả năng đó là cơ sở chọn ứng viên, không chứng minh chất lượng trên mọi microphone. [V06](https://github.com/snakers4/silero-vad)

Chained pipeline giúp sửa tên riêng, kiểm tra số tiền, hiện transcript và đổi từng thành phần. Speech-to-speech có thể cải thiện tương tác tự nhiên nhưng vẫn phải gọi broker trước hành động. Không dùng transcript của một audio model làm bằng chứng duy nhất rằng model đã hiểu chính xác câu lệnh. Hai phương án cần cùng bộ đánh giá tiếng Việt và quyền truy cập.

### ASR tiếng Việt và model local

| Ứng viên | Dùng để đánh giá | Điểm cần kiểm chứng |
| --- | --- | --- |
| sherpa-onnx Vietnamese Zipformer 30M int8 | Laptop CPU, yêu cầu tiếng Việt, footprint nhỏ | WER theo giọng, code-switch, provenance và license weights |
| faster-whisper | Baseline multilingual, CPU quantization hoặc GPU | Tốc độ trên máy đích, RAM/VRAM, endpointing và chunking |
| STT cloud | Khi cần giảm tải thiết bị hoặc chất lượng local chưa đạt | Consent upload, mạng, quota, giá, vùng xử lý và retention |

Tài liệu sherpa có model Vietnamese 30M int8 ngày 2026-02-09 trong nhóm **offline transducer**. Demo microphone dùng VAD và executable có tên simulated-streaming. Vì vậy, “nhận từ microphone” không đồng nghĩa model acoustic native streaming; cần đo thời gian từ hết câu tới transcript ổn định. Model card gốc công bố tập dữ liệu và kết quả của tác giả, chưa phải kiểm chứng độc lập cho sản phẩm này. [V10](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/zipformer-transducer-models.html), [V11](https://huggingface.co/hynt/Zipformer-30M-RNNT-6000h)

faster-whisper hỗ trợ CPU/GPU và quantization. Những bảng “up to 4×” trong README có cấu hình, model và điều kiện cụ thể; không mang nguyên số đó sang laptop người dùng hoặc suy ra độ chính xác tiếng Việt. ASR engine cũng không tự giải quyết turn-taking, echo và điều phối tác vụ. [V09](https://github.com/SYSTRAN/faster-whisper)

Chưa có cơ sở chọn một TTS tiếng Việt thắng mọi phương án. Cần nghe thử độ rõ của tên app, số, ngày và tên người; kiểm tra voice license, quyền dùng thương mại, streaming và khả năng huỷ audio. URL tài liệu Zalo TTS tồn tại nhưng lần kiểm tra chỉ trả shell/cookie. API availability, quota, giá và điều kiện đăng ký vẫn chưa được xác minh. [V15](https://developers.zalo.me/docs/api/ai-text-to-speech)

## Wake word và quyền riêng tư

“Nora ơi” dễ gọi đối với người đọc nhưng chưa chứng minh được độ chính xác âm học. Wake word phải được kiểm tra với giọng Bắc/Trung/Nam, khoảng cách, tiếng TV, câu gần giống và giọng từ loa. Cần đo false accept theo giờ audio âm tính và false reject theo số lần cố gọi.

Một bẫy license đáng chú ý: openWakeWord có **code Apache 2.0 nhưng các pretrained models được cấp CC BY-NC-SA 4.0**. README cũng nói hiện hỗ trợ English. Không được ship pretrained “hey jarvis” vào sản phẩm thương mại chỉ vì nhìn thấy license của source code. Custom model cần kiểm tra riêng license của dữ liệu, giọng tổng hợp và công cụ tạo dữ liệu. [V07](https://github.com/dscripka/openWakeWord)

Porcupine hỗ trợ Windows, local inference và custom wake word, nhưng cần AccessKey cùng điều khoản tài khoản. Danh sách ngôn ngữ công bố không có tiếng Việt; tài liệu nói ngôn ngữ bổ sung cần trao đổi với vendor. Không thể xem đây là giải pháp tiếng Việt sẵn sàng chỉ bằng cách nhập một tên mới. [V08](https://picovoice.ai/platform/porcupine/)

Mặc định không lưu raw audio xuống disk; buffer chỉ giữ đủ cho xử lý đang diễn ra. Wake/VAD nên chạy local. Nếu gửi audio hoặc transcript lên cloud, UI cần cho biết nội dung nào rời máy và cho phép thu hồi lựa chọn. Audio dùng cho đánh giá phải opt-in, có thời hạn lưu và thao tác xoá. Giọng nói hoặc speaker recognition không thay thế xác thực cho giao dịch, cấp quyền hoặc thao tác nhạy cảm.

## Ngắt lời và cập nhật có chọn lọc

Phân biệt ba ý định: **dừng nói**, **tạm dừng goal**, **huỷ goal**. Khi người dùng ngắt câu trả lời, clear TTS/output queue và dừng generation phù hợp; không mặc nhiên huỷ một workflow đang làm việc. Một side effect đã hoàn tất cũng không thể rollback chỉ bằng cancel audio.

Pipecat mô tả interruption với cancellation, flush audio chưa phát và context chỉ chứa phần thực sự đã nói. LiveKit phân biệt VAD, semantic endpointing, STT endpointing và realtime-model turn detection. Đây là cơ chế tham khảo; language coverage và cấu hình phải được kiểm tra theo phiên bản đã pin. [V12](https://docs.livekit.io/agents/logic/turns/), [V13](https://docs.pipecat.ai/pipecat/fundamentals/interruptions)

Với một desktop Python runtime, Pipecat là ứng viên hợp lý để đánh giá trước. LiveKit đáng cân nhắc khi cần browser/mobile, room transport hoặc nhiều thiết bị. Không cần triển khai cả hai chỉ để có một pipeline local.

Update policy nên nhận event có bằng chứng: bắt đầu nhánh công việc quan trọng, có kết quả mới, phát hiện mâu thuẫn, gặp blocker, cần quyền mới hoặc hoàn tất. Log epoch, retry bình thường và khởi tạo thành phần nằm trong màn hình chi tiết. TTS phải ưu tiên thông tin cần hành động và không chen vào lúc người dùng đang nói. Các quyết định này là chính sách sản phẩm cần thử với người dùng, không phải output suy luận nội bộ của model.

## Truy cập ứng dụng và folder

Ưu tiên API chính thức hoặc connector, rồi đến browser automation và cuối cùng Windows UI Automation. Một grant nên chỉ rõ resource, action, thời hạn và khả năng thu hồi. Broker kiểm tra lại grant tại thời điểm thực thi, không chỉ khi model lập kế hoạch.

Filesystem adapter phải xử lý canonical path, junction/symlink, traversal và race giữa kiểm tra với mở file. Đóng gói sidecar không tự hạn chế quyền filesystem của process đó; muốn có ranh giới chống process bị compromise cần thêm cơ chế OS/process isolation thích hợp.

Windows có integrity levels và giới hạn đối với UI được elevate. UIAccess không phải shortcut cho một trợ lý phổ thông: Microsoft dành cơ chế này cho assistive technologies, kèm yêu cầu ký, nơi cài đặt tin cậy và manifest. Sản phẩm phải báo giới hạn truy cập, không giả định quyền admin hoặc vượt secure desktop. [V05](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview)

## Kế hoạch kiểm chứng

1. Tuyển khoảng 60 người nói, cân bằng Bắc/Trung/Nam; tách speaker giữa dev và test. Mỗi người đọc và nói tự nhiên khoảng 20 câu chứa tên app, tên người, số tiền, ngày, folder và tiếng Anh xen tiếng Việt.
2. Kiểm tra mic laptop/headset, yên tĩnh, quạt/nhạc và hội thoại nền. Có câu sửa lời, nói ngập ngừng, ngắt assistant và yêu cầu dừng.
3. Báo WER/CER với quy tắc chuẩn hoá công khai; thêm intent accuracy, critical-slot accuracy, task success và unauthorized action rate. Sai một chữ số quan trọng hơn nhiều lỗi từ không ảnh hưởng ý định.
4. Đo end-of-utterance → acknowledgement, end-of-utterance → câu trả lời hữu ích và thời gian hoàn thành tác vụ riêng. Một câu “đang kiểm tra” không được tính như đáp án hữu ích.
5. Thử mất mạng, CPU busy, đứt stream, sleep/resume, lock screen và hết quota. Xác nhận recovery không lặp side effect.
6. Dùng nghe thử của người Việt để chấm phát âm, tính dễ hiểu, overlap và mức phiền của updates; điểm tự động không thay thế bước này.

Có thể bắt đầu với mục tiêu p95 dừng audio khi ngắt lời ≤250 ms và p95 đáp án hữu ích cho tác vụ đơn giản ≤2,5 s trên cấu hình/mạng công bố. Đây là **ngưỡng đề xuất chưa được đo**. Công bố median, p95, số mẫu và tỷ lệ thất bại; nếu không đạt thì giảm scope hoặc tối ưu stage gây trễ.

## Khoảng trống và điều kiện phát hành

Chưa kiểm chứng model tốt nhất cho giọng Việt, license toàn bộ dataset của model gốc, TTS production, wake word mới hoặc quota dịch vụ. Cần chốt và pin phiên bản SDK/model, ký installer và update, thử rollback, kiểm thử permission broker và đánh giá end-to-end trước khi dùng nhãn production-ready.

Phần nghiên cứu này ưu tiên tài liệu chính thức và repository/model card của tác giả. Nguồn vendor đáng tin cho API họ công bố nhưng chưa là bằng chứng độc lập về chất lượng. Không có số benchmark nào trong tài liệu này được tạo bởi một lần chạy thực nghiệm của dự án.

