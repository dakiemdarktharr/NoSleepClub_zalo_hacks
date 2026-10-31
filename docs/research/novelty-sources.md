# Source audit: novelty, VNG và production planning

**Đối chiếu ngày 31/10/2026, Asia/Saigon.** Ba ledger nhánh có **70 records, tương ứng 66 URL duy nhất** sau khi chuẩn hóa dấu slash cuối URL (16 VNG, 31 landscape, 23 security). Một URL có thể được hai nhánh dùng. Các URL khác phiên bản/định dạng của cùng paper vẫn là URL khác; **66 URL không có nghĩa 66 nghiên cứu hay 66 xác nhận độc lập**.

## Cách đọc mức bằng chứng

- Tài liệu API/platform: mạnh cho contract được nhà cung cấp công bố; chưa chứng minh tích hợp của NORA chạy hoặc tài khoản cụ thể đã có quyền.
- Incident report: thông tin có nguồn gốc, nhưng scope/timeline thay đổi theo điều tra. Đọc bản HF tháng 7 cùng báo cáo bổ sung tháng 8 và METR/Redwood; nhiều báo cáo mô tả cùng một vụ.
- Paper/card: chỉ dùng phạm vi/phương pháp đã đọc trong bản trích xuất. Chưa tái lập tác giả, chưa audit toàn dữ liệu hoặc chứng minh license của mọi thành phần.
- Product docs/launch posts: bằng chứng tính năng từng được công bố, có hạn chế plan/platform/rollout; không mặc định đó là phiên bản hiện tại hay voice đã dùng mọi agent tools.
- Product hypothesis: merchant demand, support usefulness, incremental safe completion, client RAM/quality và business ROI cần thực nghiệm riêng. Không biến suy luận thành ground truth.

## Phạm vi và giới hạn tìm kiếm

Exa được dùng cho discovery và fetch; Firecrawl live fetch kiểm tra portal LifeMate; đọc tài liệu chính thức/nguồn gốc ưu tiên hơn tin dẫn lại. Nhóm từ khóa tiếng Việt/Anh bao gồm VNG/Zalo strategy/safety/OA/support, voice assistants/actions/security, Vietnamese tool benchmarks, goal alignment/IFC/safe recovery, PHP/Hugging Face/Korean financial breaches. Incidents trọng tâm là tháng 7–10/2026; prior art kỹ thuật gồm các công trình/tài liệu cũ hơn còn liên quan.

Đây là focused deep research có nhiều nhánh và vòng kiểm tra khoảng trống, **không phải systematic review đăng ký trước hay tìm hết Internet**. Không có quyền xem nội bộ VNG, submissions thí sinh, closed-source implementations hoặc toàn bộ vùng/plan của đối thủ. Không tìm thấy một tổ hợp trong bộ nguồn này không chứng minh nó chưa tồn tại.

Hai nhánh ghi đủ search counters: VNG yêu cầu 45 result slots qua 7 queries; landscape sau gap pass yêu cầu 75 qua 15 queries. Security evidence audit mở lại 23 URL, không thêm search queries. Không suy ra tổng lượt tìm toàn phiên từ các con số đó vì các lượt discovery/fetch khác không có cùng bộ đếm đầy đủ. **Requested result slots không được gọi là sources đã đọc.** Source count 66 ở trên chỉ đếm URL có bản trích xuất được ghi ledger, không đếm kết quả chưa mở.

Các bản fetch có thể chỉ là trích xuất theo giới hạn ký tự. Security ledger ghi 4.400–12.000 ký tự/URL; landscape ghi trường limitations cho partial snapshots và current/historical availability. Không mô tả đây là 66 full papers đã đọc toàn văn. Không sao chép exploit payload/code, restricted data, tokens hoặc customer content vào repo.

## Những nguồn cần đọc cùng nhau

| Chủ đề | Nguồn gốc và cách diễn giải |
|---|---|
| VNG problem fit | VNG01/VNG12 là corporate strategy; VNG04 là scam patterns; VNG08 là report guidance. Không xác nhận support backlog/cost |
| Zalo đã có gì | VNG02/16 Safety Center, VNG03 SIMO pilot, VNG09 OA Chatbot, VNG10/11 Kiki. Tránh pitch lại như tính năng đầu tiên |
| OA access | VNG05/06/15 là auth/messaging/error contract. Token exchange cần app secret; BFF là quyết định thiết kế để bảo vệ shared secret, không phải API đã được tích hợp |
| Closest prior art | SEC07/09/10/21 và CMP05/09/24/25/26. Các phần con Giữ Ý đã có tiền lệ; chỉ giữ hypothesis workflow/evaluation nếu có giá trị thêm |
| Việt ngữ | CMP27–31 đã có tool/function-call benchmarks. Không dùng claim first Vietnamese benchmark |
| HF | SEC02/16 là July reports; SEC22/23 là August follow-up/independent inquiry. Internal eval khác production API; scope và motive interpretations cần giữ nuance |
| Korea | SEC04: regulator14/10 vẫn điều tra vai trò AI/common attacker. Không khẳng định AI autonomous bank hack |
| PHP | SEC01: 21/10 fixes,23/10 follow-up,26/10 Windows rebuild. Không coi named versions là giải quyết mọi advisory |

Official competition và audio/platform nguồn đã có trong [source ledger ban đầu](sources.md), [LifeMate](../lifemate-2026.md) và [lightweight client](../lightweight-client.md); không cộng lại vào con số 66. Portal LifeMate và OA OAuth được parent đối chiếu lại trong lượt tổng hợp. Chưa xác minh exact scoring weights, runtime/API allowances hoặc novelty có điểm riêng.

## Ledgers có claims/caveats

- [VNG/OA — 16 records](novelty-vng-sources.json)
- [Competitors, papers và Vietnamese benchmarks — 31 records](novelty-landscape-sources.json)
- [Security/incident evidence — 23 records](novelty-security-sources.json)
- [Ground truths: facts, observations, decisions, hypotheses, unknowns](../../ground-truths.json)

## Chỉ mục URL đã đối chiếu

| IDs | Nguồn | Loại bằng chứng |
|---|---|---|
| VNG01 | [VNG Annual Report 2025 — Zalo & AI](https://ir.vng.com.vn/en/business-report/zalo-ai/) | official_corporate_report_self_reported_metrics |
| VNG02 | [Zalo và Google hợp tác ra mắt Trung tâm an toàn số bảo vệ người dùng Việt](https://www.zalo.me/vi/blog/zalo-va-google-hop-tac-ra-mat-trung-tam-an-toan-so-bao-ve-nguoi-dung-viet) | official_product_and_safety_announcement |
| VNG03 | [Zalopay thí điểm cảnh báo tài khoản nhận tiền nghi ngờ gian lận](https://zalopay.vn/zalopay-trien-khai-thi-diem-tinh-nang-canh-bao-tai-khoan-nhan-tien-nghi-ngo-gian-lan-lua-dao-7993) | official_payment_product_announcement |
| VNG04 | [Zalo Help — Đề phòng chiêu trò mạo danh hợp tác với Zalo để lừa đảo](https://help.zalo.me/huong-dan/chuyen-muc/bao-mat-va-rieng-tu/de-phong-chieu-tro-mao-danh-hop-tac-voi-zalo-de-lua-dao/) | official_user_safety_guidance |
| VNG05 | [Zalo OA — Xác thực và ủy quyền cho Ứng dụng](https://docs.zaloplatforms.com/docs/OA/bat-dau/xac-thuc-va-uy-quyen-cho-ung-dung-new) | official_API_authorization_documentation |
| VNG06 | [Zalo OA — Tin nhắn tổng quan](https://docs.zaloplatforms.com/docs/OA/tin-nhan/tong-quan) | official_API_messaging_policy |
| VNG07 | [Zalo Help — Tài khoản bị tạm thời vô hiệu hóa](https://help.zalo.me/huong-dan/chuyen-muc/quan-ly-tai-khoan-zalo/loi-thuong-gap/tai-khoan-zalo-bi-tam-thoi-vo-hieu-hoa/) | official_account_policy_guidance |
| VNG08 | [Zalo Help — Báo xấu tài khoản Zalo](https://help.zalo.me/huong-dan/chuyen-muc/bao-mat-va-rieng-tu/bao-xau-tai-khoan-zalo/) | official_user_support_process |
| VNG09 | [Zalo OA — Tự động hóa chăm sóc khách hàng với Zalo Chatbot](https://oa.zalo.me/home/resources/library/tu-dong-hoa-cham-soc-khach-hang-voi-zalo-chatbot_6352033339970702125) | official_existing_product_documentation |
| VNG10 | [Zalo — Kiki Info](https://zalo.me/en/post/kiki-info/) | official_existing_product_description |
| VNG11 | [Zalo — Kiki Auto](https://zalo.me/en/product/kiki-auto/) | official_existing_product_description |
| VNG12 | [VNG FY2025 audited results press release](https://vng.com.vn/news/press-release/vng-fy2025.html) | official_corporate_press_release |
| VNG13 | [Zalopay — nâng cấp bảo mật theo Thông tư 77/2025/TT-NHNN](https://zalopay.vn/thong-bao-zalopay-nang-cap-bao-mat-theo-thong-tu-77-8302) | official_product_security_notice |
| VNG14 | [Zalopay statement about CIC incident independence](https://zalopay.vn/he-thong-va-thong-tin-khach-hang-cua-zalopay-khong-bi-anh-huong-boi-su-co-tai-cic-7451) | official_company_incident_related_statement |
| VNG15 | [Zalo OA API error codes](https://docs.zaloplatforms.com/docs/OA/phu-luc/ma-loi) | official_API_operational_contract |
| VNG16 | [Zalo/Google Safety Center landing page](https://www.zalo.me/vi/safety) | official_live_safety_product_page |
| CMP01 | [Introducing ChatGPT agent](https://openai.com/index/introducing-chatgpt-agent/) | official_product_launch |
| CMP02 | [ChatGPT Agent System Card — security controls](https://deploymentsafety.openai.com/chatgpt-agent/security-controls) | official_system_card |
| CMP03 | [ChatGPT agent — current help overview](https://help.openai.com/en/articles/11752874-chatgpt-agent) | official_product_documentation |
| CMP04 | [Talk naturally with Gemini Live](https://support.google.com/gemini/answer/15274899?hl=en) | official_product_documentation |
| CMP05, SEC19 | [Architecting Security for Agentic Capabilities in Chrome](https://blog.google/security/architecting-security-for-agentic/) | official_vendor_security_architecture |
| CMP06 | [Gemini Computer Use](https://ai.google.dev/gemini-api/docs/computer-use) | official_API_documentation |
| CMP07 | [Voice features in Microsoft Copilot FAQ](https://support.microsoft.com/en-us/microsoft-365-copilot/frequently-asked-questions-about-voice-features-in-microsoft-365-copilot) | official_product_documentation |
| CMP08 | [Supported languages for Microsoft Copilot](https://support.microsoft.com/en-us/microsoft-365-copilot/supported-languages-for-microsoft-365-copilot) | official_product_documentation |
| CMP09, SEC20 | [Considerations for Safe Agentic Browsing](https://blogs.windows.com/msedgedev/2025/10/23/considerations-for-safe-agentic-browsing/) | official_vendor_security_architecture |
| CMP10 | [Browse with Copilot](https://support.microsoft.com/en-us/edge/copilot/browse-with-copilot) | official_product_documentation |
| CMP11 | [Introducing Alexa+](https://www.aboutamazon.com/news/devices/new-alexa-generative-artificial-intelligence) | official_product_launch |
| CMP12 | [Use Apple Intelligence in Shortcuts](https://support.apple.com/guide/shortcuts/use-apple-intelligence-in-shortcuts-tpg3vrvwmclv/ios) | official_product_documentation |
| CMP13 | [Apple Intelligence availability and requirements](https://support.apple.com/en-us/121115) | official_product_documentation |
| CMP14 | [Set up a fully local voice assistant — Home Assistant](https://www.home-assistant.io/voice_control/voice_remote_local_assistant) | official_project_documentation |
| CMP15 | [Best practices with Assist](https://www.home-assistant.io/voice_control/best_practices) | official_project_documentation |
| CMP16 | [OpenClaw tool and agent permissions](https://docs.openclaw.ai/gateway/security/tool-permissions) | official_project_documentation |
| CMP17 | [OpenClaw sandboxing](https://docs.openclaw.ai/sandboxing) | official_project_documentation |
| CMP18 | [OpenClaw prompt injection guidance](https://docs.openclaw.ai/gateway/security/prompt-injection) | official_project_documentation |
| CMP19 | [Mitigating Prompt Injection in Comet](https://www.perplexity.ai/hub/blog/mitigating-prompt-injection-in-comet) | official_vendor_security_architecture |
| CMP20 | [BrowseSafe — Perplexity](https://www.perplexity.ai/hub/blog/browsesafe) | official_vendor_research |
| CMP21 | [Managing Comet Assistant permissions](https://www.perplexity.ai/help-center/en/articles/13531023-managing-comet-assistant-permissions) | official_product_documentation |
| CMP22 | [Work with Browsers and Apps — Perplexity](https://www.perplexity.ai/help-center/en/articles/19800006-work-with-browsers-and-apps) | official_product_documentation |
| CMP23 | [ChatGPT Voice FAQ](https://help.openai.com/en/articles/8400625-voice-mode-faq) | official_product_documentation |
| CMP24, SEC07 | [CaMeL — Defeating Prompt Injections by Design](https://arxiv.org/html/2503.18813v2) | original_research_preprint |
| CMP25, SEC09 | [The Task Shield](https://aclanthology.org/2025.acl-long.1435/) | original_peer_reviewed_research_record |
| CMP26 | [RTBAS — Defending LLM Agents Against Prompt Injection and Privacy Leakage](https://arxiv.org/html/2502.08966v1) | original_research_preprint |
| CMP27 | [Multilingual-BFCL / Teaching Small Models When Not to Call Functions](https://huggingface.co/datasets/qualgo-technologies/Multilingual-BFCL) | original_author_dataset_card_and_research_artifact |
| CMP28 | [Vietnamese Function Calling Benchmark — phamhai](https://huggingface.co/datasets/phamhai/Vietnamese-Function-Calling-Test) | original_author_dataset_card |
| CMP29 | [V-Bench — Dataset and Submission/Scoring Specification](https://vbench.ai/dataset) | original_benchmark_website_documentation |
| CMP30 | [V-Bench — Vietnamese LLM Benchmark](https://vbench.ai/) | original_benchmark_website |
| CMP31 | [Teaching Small Models When Not to Call Functions: Structured Reasoning for Tool Refusal in Low-Resource Languages](https://doi.org/10.1145/3805712.3809979) | original_peer_reviewed_research_record_and_abstract |
| SEC01 | [PHP: News Archive — October 2026](https://www.php.net/archive/2026.php) | official runtime advisory |
| SEC02 | [Anatomy of a Frontier Lab Agent Intrusion: A Technical Timeline of the July 2026 Incident](https://huggingface.co/blog/agent-intrusion-technical-timeline) | first-party incident technical report |
| SEC03 | [The Hugging Face incident and other third-party impacts from misaligned models](https://openai.com/hugging-face-incident-and-misalignment/) | official living incident index |
| SEC04 | [Authorities Hold Meeting on Cybersecurity Management in Financial Sector to Bolster Anti-Hacking Measures](https://www.fsc.go.kr/eng/pr010101/87928) | official regulator release |
| SEC05 | [Space secrets leak disclosure](https://huggingface.co/blog/space-secrets-disclosure) | first-party incident disclosure |
| SEC06 | [Load/Mine: A Hugging Face Malware Campaign Returns After Takedown](https://pluto.security/blog/hugging-face-malware-campaign-load-mine/) | original security research report |
| SEC08 | [google-research/camel-prompt-injection](https://github.com/google-research/camel-prompt-injection) | author research repository |
| SEC10 | [Securing AI Agents with Information-Flow Control (Fides)](https://arxiv.org/html/2505.23643v2) | original research preprint |
| SEC11 | [microsoft/fides](https://github.com/microsoft/fides) | author research repository |
| SEC12 | [AgentDojo: A Dynamic Environment to Evaluate Prompt Injection Attacks and Defenses for LLM Agents](https://arxiv.org/html/2406.13352v3) | original research paper |
| SEC13 | [Technical Blog: Strengthening AI Agent Hijacking Evaluations](https://www.nist.gov/news-events/news/2025/01/technical-blog-strengthening-ai-agent-hijacking-evaluations) | government original technical evaluation |
| SEC14 | [Hijacking Large Audio-Language Models via Context-Agnostic and Imperceptible Auditory Prompt Injection (AudioHijack)](https://arxiv.org/html/2604.14604) | original research preprint |
| SEC15 | [MCP Security Best Practices](https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices) | official protocol security documentation |
| SEC16 | [OpenAI and Hugging Face partner to address security incident during model evaluation](https://openai.com/index/hugging-face-model-evaluation-security-incident/) | first-party incident report |
| SEC17 | [Security incident disclosure — July2026 (fixed source commit)](https://github.com/huggingface/blog/blob/1c5b998812cbbb3952e6b6d6a800dbebfa375b85/security-incident-july-2026.md) | first-party initial disclosure at fixed commit |
| SEC18 | [Prompt Injection Detection — OpenAI Guardrails Python](https://openai.github.io/openai-guardrails-python/ref/checks/prompt_injection_detection/) | official library documentation |
| SEC21 | [RTBAS: Defending LLM Agents Against Prompt Injection and Privacy Leakage](https://arxiv.org/abs/2502.08966) | original research preprint |
| SEC22 | [The Hugging Face incident and the road ahead](https://openai.com/index/hugging-face-incident-and-the-road-ahead/) | first-party expanded incident account |
| SEC23 | [Brief independent investigation of agents' behavior, reasoning and collaboration in the OpenAI / Hugging Face hacking incident](https://evals.alignment.org/hugging-face-incident-report-aug-2026.pdf) | independent third-party investigation |
