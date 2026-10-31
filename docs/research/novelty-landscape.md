# NORA: đối thủ, prior art và phần khác biệt còn có thể bảo vệ

Ngày đối chiếu: **31/10/2026, Asia/Saigon**. Chỉ nghiên cứu và đề xuất; không triển khai, đăng nhập dịch vụ, chạy benchmark hay xem bài thi của người khác. Hai lượt nghiên cứu đã yêu cầu tổng cộng 75 kết quả trong 15 truy vấn Exa và mở 31 URL duy nhất. Đây không phải 75 nguồn đã đọc hoặc 31 xác nhận độc lập. Source ledger ghi phạm vi đọc ở [novelty-landscape-sources.json](novelty-landscape-sources.json).

## Kết luận cho lựa chọn novelty

**Voice + LLM + nhiều agent + tools + permissions không còn là novelty. Một security critic, classifier chống prompt injection, provenance hoặc hỏi ít lần hơn cũng đã có prior art rõ.** NORA có thể dùng các thành phần này như nền tảng, nhưng không nên trình bày chúng như phát minh đầu tiên.

Hướng còn đáng chứng minh là một **workflow sản phẩm cho người Việt/Zalo OA merchant**: giữ nguyên mục tiêu hợp lệ khi nội dung bên ngoài cố đổi người nhận, đích dữ liệu hoặc hành động; hoàn thành phần độc lập còn an toàn; hỏi đúng thay đổi trọng yếu; tạo biên nhận tối thiểu, được người dùng duyệt, để chuyển tiếp support. Đây là giả thuyết khác biệt về tích hợp, ngữ cảnh và kết quả đo được, chưa phải bằng chứng độc nhất toàn cầu.

Không thể biết các đội khác sẽ nộp gì từ tìm kiếm công khai. “Dễ trùng” dưới đây là **nhận định của chúng ta** dựa trên độ phổ biến, độ dễ tích hợp và prior art; không có phần trăm hoặc thông tin nội bộ về thí sinh. Loại một hướng khỏi *claim novelty* không có nghĩa bỏ khả năng cơ bản đó khỏi NORA.

## Tám nhóm đối thủ cần coi là baseline

| Nhóm | Khả năng có bằng chứng | Security/local-cloud có bằng chứng | Điều không được suy diễn |
|---|---|---|---|
| **Google Gemini Live + Computer Use** | Live đối thoại/ngắt lời và dùng một số Connected Apps; Computer Use là building block cho browser/mobile/desktop actions | Chrome công bố User Alignment Critic, origin gating, confirmations và injection checks | Developer API khác consumer Live. Account/platform/rollout hạn chế; chưa đo giọng Việt hoặc mọi app trong voice. [CMP04](https://support.google.com/gemini/answer/15274899?hl=en), [CMP05](https://blog.google/security/architecting-security-for-agentic/), [CMP06](https://ai.google.dev/gemini-api/docs/computer-use) |
| **ChatGPT Voice + dòng agent/Work** | Voice hội thoại; agent historical có browser, terminal, connectors và nghiên cứu nhiều bước | System card có confirmations, Watch Mode, lớp phòng vệ injection | Launch post agent đã ghi outdated; help hiện nói agent không còn khả dụng và chuyển hướng Work. Live Voice ban đầu chưa hỗ trợ connected apps/plugins. Không cộng mọi feature thành một voice flow hiện hành. [CMP01](https://openai.com/index/introducing-chatgpt-agent/), [CMP03](https://help.openai.com/en/articles/11752874-chatgpt-agent), [CMP23](https://help.openai.com/en/articles/8400625-voice-mode-faq) |
| **Microsoft Copilot Voice/Vision/Actions** | Voice Windows, wake activation opt-in; Voice có tiếng Việt; Actions/Browse thực hiện web tasks | Site controls, sensitive-action supervision, relevance checks; Task Tracker được mô tả đang thử nghiệm | Vision hướng dẫn không đồng nghĩa tự thao tác. FAQ nói một số text features và voice chat với agents chưa được hỗ trợ. “Browser chạy local” không có nghĩa LLM local. [CMP07](https://support.microsoft.com/en-us/microsoft-365-copilot/frequently-asked-questions-about-voice-features-in-microsoft-365-copilot), [CMP08](https://support.microsoft.com/en-us/microsoft-365-copilot/supported-languages-for-microsoft-365-copilot), [CMP09](https://blogs.windows.com/msedgedev/2025/10/23/considerations-for-safe-agentic-browsing/) |
| **Amazon Alexa+** | Voice tự nhiên, smart home, mua sắm/đặt chỗ và nhóm “experts” điều phối services | Privacy controls/dashboard theo công bố; xử lý dựa trên model/service cloud | Launch/marketing có phần rollout/future. Chưa xác minh tiếng Việt, desktop arbitrary-app control hoặc superiority. [CMP11](https://www.aboutamazon.com/news/devices/new-alexa-generative-artificial-intelligence) |
| **Apple Siri/Shortcuts/Apple Intelligence** | Kết hợp app actions với model on-device, Private Cloud Compute hoặc ChatGPT trong Shortcuts | On-device/cloud phân tầng và OS/app permissions | Apple Intelligence có Vietnamese không đồng nghĩa mọi Siri AI beta feature có Vietnamese. Initial Siri AI language list trong tài liệu đã đọc không có Vietnamese; ecosystem/device constraints rõ. [CMP12](https://support.apple.com/guide/shortcuts/use-apple-intelligence-in-shortcuts-tpg3vrvwmclv/ios), [CMP13](https://support.apple.com/en-us/121115) |
| **Home Assistant Assist** | Local STT–intent–TTS, Whisper/Speech-to-Phrase/Piper, home entities và LLM agent tùy chọn | Mọi entity opt-in; voice request không tự có user identity, cần acts-as-user mapping | Closed-ended Speech-to-Phrase không thay mọi task mở; không phải desktop assistant đầy đủ. Language quality thay đổi. [CMP14](https://www.home-assistant.io/voice_control/voice_remote_local_assistant), [CMP15](https://www.home-assistant.io/voice_control/best_practices) |
| **OpenClaw** | Agent/tools, multi-agent profiles, local/remote tool execution theo cấu hình | Per-agent sandbox, tool policy, session visibility, approvals; sandbox off mặc định theo docs | Không xem sandbox là hoàn hảo, persona là isolation, hoặc model resistance là immunity. Voice quality/Windows budget không được kiểm tra trong nghiên cứu này. [CMP16](https://docs.openclaw.ai/gateway/security/tool-permissions), [CMP17](https://docs.openclaw.ai/sandboxing), [CMP18](https://docs.openclaw.ai/gateway/security/prompt-injection) |
| **Perplexity/Comet** | Browser research/actions; voice entry Windows/Mac trong desktop app | Classifiers, guardrails, confirmations, thông báo security; BrowseSafe; enterprise domain/permission controls | Trang app-control đã đọc ghi installed-app control chỉ có Mac. Voice entry không chứng minh toàn bộ actions chạy trong mọi voice session. [CMP19](https://www.perplexity.ai/hub/blog/mitigating-prompt-injection-in-comet), [CMP20](https://www.perplexity.ai/hub/blog/browsesafe), [CMP21](https://www.perplexity.ai/help-center/en/articles/13531023-managing-comet-assistant-permissions), [CMP22](https://www.perplexity.ai/help-center/en/articles/19800006-work-with-browsers-and-apps) |

Nguồn vendor đáng tin để xác nhận tính năng/giới hạn mà họ công bố, không phải kiểm chứng độc lập về chất lượng hoặc security tuyệt đối. Availability dễ thay đổi; các trang live không phải bản archive chứng minh rollout tại mọi quốc gia. Không có phép so sánh thực nghiệm giữa NORA và các sản phẩm trên.

## Những hướng dễ trùng cần loại khỏi thông điệp novelty

| Hướng | Nhận định trùng lặp — suy luận của nhóm | Xử lý trong NORA |
|---|---|---|
| Voice điều khiển lịch, đèn, nhạc, nhắc việc | Rất dễ trùng vì đã là core của nhiều hệ thống | Giữ như baseline bắt buộc, không làm headline |
| STT → LLM → TTS, wake word và interruption | Pipeline và UX phổ biến, có local reference projects | Tối ưu tiếng Việt và đo latency; không gọi kiến trúc này là mới |
| RAG/tìm tài liệu rồi tóm tắt | Retrieval/context và report generation đã phổ biến | Giữ citations/provenance, đo đúng tài liệu/quyền truy cập |
| Nhiều agent nghiên cứu/phân tích/kiểm tra | Alexa experts, agent orchestration và các SDK đã có | Chỉ dùng khi task decomposition đáng chi phí |
| MCP hoặc connector layer | Chuẩn kết nối không tự tạo điểm khác biệt sản phẩm | Dùng làm phương tiện, broker vẫn enforce quyền |
| Thêm confirmation trước thao tác nhạy cảm | OpenAI, Google, Microsoft, Comet đều có | Bắt buộc làm tốt nhưng không dùng làm claim mới |
| Một model “phát hiện prompt injection/lừa đảo” | Classifier + alerts đã có nhiều tiền lệ | Dùng như một tín hiệu; tránh block-all và security theater |
| Privacy/local-first hoặc nhẹ RAM | Home Assistant và on-device products đã có | Có thể là lợi thế triển khai nếu benchmark đạt, chưa phải novelty đã chứng minh |

Không loại bỏ baseline chỉ để khác biệt. Nếu NORA chỉ demo security nhưng không làm đúng calendar/file/email/smart-home tasks được hỗ trợ, novelty không bù được mất utility hoặc điểm LifeMate.

## Prior art đánh trực tiếp vào ý tưởng security

| Thành phần dự kiến của NORA | Prior art gần nhất đã mở | Kết luận trung thực |
|---|---|---|
| Đối chiếu action với ý định người dùng | Chrome **User Alignment Critic**, Edge **Task Tracker/relevance checks**, **Task Shield** | Không phải ý tưởng mới chỉ vì thêm một verifier |
| Tách planner khỏi dữ liệu không tin cậy | Chrome dual-LLM/CaMeL-inspired design, **CaMeL** | Cần nêu rõ reuse/extension và boundary, không đổi tên thành invention |
| Theo dõi nguồn dữ liệu, allowed recipient/egress | **CaMeL** gắn capabilities/provenance/allowed readers vào giá trị | Goal contract + data provenance tự nó đã có nền tảng nghiên cứu |
| Tự chạy calls an toàn, chỉ hỏi khi cần | **RTBAS** dùng integrity/confidentiality checks, hỏi khi không bảo đảm | “Ít confirmation hơn” là mục tiêu đã có; phải chứng minh mức utility/friction thực tế |
| Classifier, cảnh báo và user controls | **Comet/BrowseSafe**, Google, Microsoft | Không gọi “AI firewall” là novelty nếu chỉ bọc classifier |
| Tiếp tục phần hợp lệ sau một nhánh bị chặn | Task Shield có feedback/refinement; RTBAS/CaMeL giữ task utility | Safe partial progress có thể là engineering contribution trong domain cụ thể; chưa đủ chứng minh first-ever |

[CaMeL](https://arxiv.org/html/2503.18813v2) trích xuất control/data flow từ trusted query và enforce capabilities khi gọi tools. Bài này có giả định/interpreter và phạm vi đánh giá cụ thể; không chuyển lời “provable” thành bảo đảm an toàn cho mọi app GUI.

[Task Shield](https://aclanthology.org/2025.acl-long.1435/) kiểm tra mỗi instruction/tool call có đóng góp cho user goal; các số attack success/utility trong paper là kết quả trên model/benchmark của tác giả, không phải số NORA.

[RTBAS](https://arxiv.org/html/2502.08966v1) nêu đúng bài toán approval fatigue: tự cho chạy calls giữ integrity/confidentiality và hỏi khi các điều kiện đó không được bảo đảm. Vì vậy nguồn này phải có mặt khi trình bày novelty, kể cả khi lựa chọn implementation khác.

## Đối chiếu thêm benchmark tiếng Việt

Không được gọi NORA là benchmark gọi công cụ tiếng Việt đầu tiên. Phần này chỉ đọc dataset cards, trang gốc và abstract; không tải dữ liệu, chạy evaluator hay xác minh độc lập các điểm số của tác giả.

| Prior art gốc | Phạm vi đã xác minh | Overlap với NORA | Điều chưa được nguồn này chứng minh |
|---|---|---|---|
| **Multilingual-BFCL / BFCL-VN**, Qualgo/UNEY | Card công bố 3.641 mẫu mỗi ngôn ngữ Việt/Thái trong phạm vi BFCL v3 single-turn của paper; đánh giá tool use, irrelevance và tool-call hallucination. Paper SIGIR '26 có DOI, xuất bản 10/07/2026. Card có thêm file kiểu BFCL v4 nhưng paper không báo kết quả multi-turn. [CMP27](https://huggingface.co/datasets/qualgo-technologies/Multilingual-BFCL), [CMP31](https://doi.org/10.1145/3805712.3809979) | Quyết định gọi, không gọi hoặc thiếu tool; structured reasoning; cân bằng capability/refusal và hiệu quả token đều đã có | Paper này không đánh giá audio, khôi phục workflow nhiều lượt dưới injection, hoặc toàn cây process trong 2 GB RAM. Không suy từ mô hình 1B sang minimum hardware cho NORA |
| **Vietnamese-Function-Calling-Test**, phamhai | Card ghi 2.899 mẫu single-turn, 159 functions, nhiều domain như banking, insurance, travel, shopping và work. Chấm đúng tên hàm và exact arguments. [CMP28](https://huggingface.co/datasets/phamhai/Vietnamese-Function-Calling-Test) | Tiếng Việt + domain thực tế + chọn tool/arguments không mới | Card đã đọc không mô tả audio, stateful multi-turn recovery hoặc paired injection. Chưa audit nguồn tạo dữ liệu/giấy phép, không tự xem các số model là kết quả có thể so sánh với NORA |
| **V-Bench**, CAIR/VinUniversity | Website gốc có track Agentic; trang dataset công bố release v2026.03.28 ngày 27/03/2026 và định dạng function-call JSONL. Track Safety được ghi đang inactive trong public scoring. [CMP29](https://vbench.ai/dataset), [CMP30](https://vbench.ai/) | Đánh giá agentic tiếng Việt, schema/required/enum và tool correctness đã có | Trang public không chứng minh benchmark audio + attack/recovery nhiều lượt. “Safety” ở đây mô tả tri thức/nội dung và không tự đồng nghĩa indirect prompt injection/data-flow safety. Chưa kiểm chứng toàn bộ private/professional test |

Phần có thể đề xuất là **một protocol đánh giá hẹp hơn**: cùng mục tiêu được giao bằng audio tiếng Việt, cặp dữ liệu sạch/bị thao túng/đổi thông tin hợp lệ, critical entities dễ nghe nhầm, tác vụ nhiều lượt đi qua các app được phép, đo cả hoàn thành đầy đủ an toàn, tiến độ từng phần, câu hỏi trọng yếu, interruption/recovery và tài nguyên client. Đây là tổ hợp cần kiểm chứng thêm, không phải tuyên bố chưa từng tồn tại. Những card trên không bao phủ nó trong phạm vi đã công bố/đã đọc; điều đó không loại trừ mọi benchmark hoặc sản phẩm khác.

**Phép bác bỏ:** giữ cùng audio, model, tools, grants và ngân sách; nếu thêm contract/provenance/safe continuation không cải thiện verified safe completion hoặc chỉ thắng nhờ từ chối nhiều hơn, không giữ claim novelty. Nếu tìm được protocol đã bao phủ cùng trục đánh giá, trình bày NORA như adaptation/engineering validation cho workflow được chọn. Không dùng dữ liệu đánh giá làm training mà vẫn báo kết quả held-out.

Không tải hoặc tái phát hành các bản ZaloLMBench/dữ liệu cuộc thi do bên thứ ba đưa lên Hugging Face khi quyền chia sẻ chưa rõ. Những nguồn đó không được coi là tài sản nghiên cứu có thể dùng mặc định; [portal LifeMate chính thức](https://challenge.zalo.ai/portal/lifemate) và thể lệ mới là căn cứ cho competition path. Việc một dataset card tồn tại công khai cũng chưa cấp quyền thương mại cho mọi dữ liệu bên trong.

## Phần khác biệt còn đáng làm cho Zalo

Đề xuất giữ feature thành một **vòng xử lý hoàn chỉnh**, không chỉ thêm lớp chặn:

1. **Chốt ý định bằng tiếng Việt thành contract có cấu trúc.** Lưu mục tiêu, đối tượng, loại effect, data scope và acceptance checks. Đích nhận/chi tiết quan trọng lấy từ ngữ cảnh hoặc registry được phép; nội dung khách hàng gửi chỉ là một claim có nguồn.
2. **Phát hiện thay đổi trọng yếu giữa lời giao và evidence mới.** Ví dụ yêu cầu hỗ trợ đơn hàng ban đầu không cho phép một tin nhắn đổi tài khoản nhận tiền hoặc yêu cầu gửi OTP/log đầy đủ. Đổi số điện thoại/tài khoản hợp lệ vẫn phải có lối xác nhận; không gán nhãn mọi thay đổi là lừa đảo.
3. **Giữ tiến độ phần an toàn.** Có thể kiểm tra đơn, chuẩn bị bản tóm tắt, tìm kênh support đúng trong khi chờ xác minh một đích nhận. Không thực thi action phụ thuộc vào dữ liệu chưa tin cậy. “Maximal safe subgraph” cần định nghĩa theo dependency/policy rõ, không chỉ LLM nói phần đó an toàn.
4. **Hỏi một câu có ích.** Nêu chính xác field/đích dữ liệu đang mâu thuẫn và bằng chứng; không đọc nguyên reasoning hoặc xin duyệt từng tool. Approval phải bind nội dung/action hiện tại, không coi voice “ừ” phát lại là quyền vô hạn.
5. **Biên nhận chuyển tiếp support do user kiểm soát.** Tạo bản đã redaction gồm goal, bước đã hoàn thành, bằng chứng cần thiết và blocker. User xem trước và chọn gửi; không tự upload toàn bộ chat, PII hoặc credentials cho Zalo/VNG. Receipt ghi được điều đã kiểm chứng, không chứng minh danh tính hoặc ý định của bên gửi ngoài phạm vi dữ liệu có thật.

Lợi thế tiềm năng là **giữ khả năng hoàn thành việc hợp lệ dưới tình huống người Việt gặp thật**, qua OA/merchant workflows được quyền truy cập, với workload phù hợp client nhẹ. Không cần tự nhận đã có API private của Zalo, quyền đọc personal chats hoặc chứng thực ngân hàng. Khả năng triển khai kênh OA và pain business cần nhánh nghiên cứu VNG xác nhận riêng.

Đây là product hypothesis. Những thành phần con đều có overlap; chưa tìm thấy trong nhóm nguồn đã đọc một sản phẩm chứng minh đúng tổ hợp/bộ đánh giá domain này. **Không tìm thấy không đồng nghĩa chưa ai làm.** Có thể gọi là khác biệt triển khai/đánh giá, không nên gọi là thuật toán security mới nếu không có đóng góp và kết quả tương ứng.

## Làm sao chứng minh “tốt hơn” và bác bỏ novelty nếu không đạt

Thiết kế bộ task theo cặp cùng user goal: phiên bản sạch, phiên bản có injection/scam claim, và phiên bản đổi thông tin hợp lệ. Có Bắc/Trung/Nam, noise, code-switching, names/numbers và multi-turn corrections. Scenario lấy từ public domain patterns hoặc synthetic consented fixtures, không đưa dữ liệu khách hàng/private competition data vào public repo.

Baselines cần cùng model/API, tool catalog, grants, audio và token/tool budgets:

- B0: agent function-calling cơ bản với confirmations cần thiết.
- B1: B0 + classifier/critic/guardrails tiêu chuẩn.
- B2: B1 + contract/provenance/egress enforcement.
- B3: B2 + safe partial progress/material-delta clarification.
- B4: B3 + redacted support receipt và evidence readback.

So sánh trực tiếp consumer assistants chỉ ở tasks/integrations họ thực sự hỗ trợ và khi có quyền dùng; không chấm một app không có connector bằng 0 để tạo lợi thế giả. SDK baselines giúp tách tác dụng kiến trúc khỏi việc NORA chọn model mạnh hơn. Không dùng tên “CaMeL baseline” nếu chưa tái hiện đúng interpreter/threat model; có thể gọi là baseline lấy cảm hứng và nêu phần thiếu.

Metrics chính: **verified legitimate-goal completion under attack** và unauthorized effects/leaks. Metrics kèm: benign-task success, false blocks, completion sau partial progress, số câu hỏi user, time-to-useful-result, API cost và memory toàn cây process. Support receipt đánh giá đúng evidence, redaction leakage và thời gian người support hiểu case, với người đánh giá thật khi có design partners.

Ablation bỏ từng module contract/provenance/partial progress/receipt để kiểm tra phần nào tạo giá trị. Đo nhiều trials, tách task/attack templates giữa dev/test, có adaptive attacker/red-team trong môi trường được phép. Báo uncertainty/sample size; benchmark tĩnh không chứng minh immunity ngoài đời.

**Go/no-go:** nếu B3/B4 không cải thiện completion/friction hoặc support handoff so với B1/B2 trong budget đã chốt, bỏ phần “novelty” không tạo giá trị hoặc đổi hypothesis. Nếu giảm attack success chỉ nhờ từ chối mọi task, không coi là thành công. Nếu tăng benign utility nhưng cho leak/effect nghiêm trọng, dừng release và sửa boundary. Không đặt số mục tiêu thành thành tích đã có.

Core capabilities cần giữ: hội thoại tiếng Việt/ngắt lời, app/file search, notes/calendar/reminders, research/report, tool sequences, missing-info/unsupported handling, cancel/resume và goal verification. Kết luận “hơn” chỉ nên áp dụng vào các task/điều kiện đã đo, không phải “vượt mọi voice assistant” hoặc “an toàn tuyệt đối”.

## Khoảng trống còn lại

Chưa có tài khoản/test cùng cấu hình cho từng competitor, chưa thực nghiệm tiếng Việt, chưa đánh giá support agents của VNG, chưa có submissions của thí sinh. Những nguồn về brand/features là live docs hoặc launch disclosures; không mặc nhiên phản ánh tất cả vùng/plan. Không audit source code đóng hoặc reproduce các paper. Sự khác biệt đủ thuyết phục cần evidence từ prototype, paired evaluation và design-partner validation ở bước sau.
