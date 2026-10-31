# Từ research foundation đến production

Ngày lập: **31/10/2026**. Milestones là kế hoạch đề xuất; chỉ đi tiếp khi acceptance gates đạt. Không có ước tính đáng tin để hứa production chỉ từ một phiên nghiên cứu.

## Định nghĩa phạm vi sản phẩm đầu

Windows per-user assistant, tiếng Việt, hotkey/push-to-talk; một folder do user chọn, Obsidian note creation và một app connector có API rõ. Có bounded task và durable research/report goal; có pause/resume/cancel và selective updates. Banking/payment/ride thật không thuộc beta đầu nếu chưa có integration, consent và kiểm chứng đủ mạnh.

| Mốc | Deliverable | Gate trước khi mở rộng |
|---|---|---|
| Research foundation — hiện tại | Report, sources, ground truths, schemas, reference policies | Tests/reference và links kiểm tra được; không gọi là desktop app |
| M1: contract spike | Devkit/evaluator LifeMate; chọn versions; voice bake-off plan | Đọc rules/API/limits, quyền model/data rõ; baseline reproduce |
| M2: vertical slice | Audio → intent → một simulator tool → verifier → text/TTS | Final state đúng; missing/unsupported xử lý đúng; latency đo thật |
| M3: local desktop alpha | Tauri tray, mic switch, Rust broker, một folder/Obsidian adapter | Không effect ngoài scope trên suite; IPC/path/grant tests; installer nội bộ |
| M4: durable alpha | Checkpoint, effect ledger, supervisor, approvals, outbox | Crash/restart matrix, revocation, reconciliation và cancel đạt |
| M5: closed beta | Nhiều người dùng có đồng thuận, telemetry tối thiểu, support | Chất lượng giọng Việt, update usefulness, cost và failure rates được báo |
| M6: production release | Signed installer/update, migration/rollback, data controls | Security/privacy/license review, canary và response procedure |

Một đội nhỏ nên hoàn thành vertical slice trong khoảng 1–2 tuần nếu đã có credentials/contract; đây là giả định lập kế hoạch, không cam kết. Closed beta và production cần thời gian đo thực tế, phụ thuộc mức hỗ trợ app/hardware và rủi ro. Đừng ghép đồng thời multi-agent, wake word, multi-platform và hàng chục connector vào M2.

## Sprint cuộc thi riêng

| Ngày dự kiến GMT+7 | Ưu tiên |
|---|---|
| 31/10–02/11 | Xác minh devkit/rules, dựng baseline reproduce, định nghĩa eval |
| 03–06/11 | ASR/slots, tool schema validation, dependency execution |
| 07–10/11 | Error analysis, missing-info/unsupported, local ablation |
| 11–13/11 | Cost/runtime/fault checks, freeze candidate |
| 14–16/11 | Reproduce, packaging và submission checks trước 23:59 ngày 16 |

Các khoảng trên là lịch làm việc đề xuất; mốc chính thức cần đọc lại ở [LifeMate](lifemate-2026.md). Không tự đăng ký, chấp thuận điều khoản, nộp bài hoặc công khai mã thi khi chưa hoàn tất bước tương ứng.

## Model và framework selection

Baseline ASR: faster-whisper với multilingual Whisper; đối chứng sherpa-onnx Vietnamese CPU model. Giữ fixed audio/test splits; không chỉ chọn theo WER từ model card. TTS đối chứng ít nhất hai phương án tiếng Việt có quyền thương mại rõ; đánh giá phát âm tên riêng/số/ngày và time-to-audio. Realtime speech-to-speech là phương án sau, qua cùng broker.

Pin versions sau spike, ghi lockfile và model hashes; không ghi “latest” như phiên bản reproduce. LangGraph checkpoints và SQLite local store cần kiểm tra compatibility/durability; development saver chưa đủ cho production. Nếu chưa đạt trên Windows, đánh giá explicit state machine hoặc backend runtime bằng ADR, thay vì che giới hạn bằng retry.

## Evaluation corpus

Đề xuất khoảng 60 người nói × 20 yêu cầu, cân bằng Bắc/Trung/Nam, phân chia speaker độc lập dev/test. Có mic laptop/headset; yên tĩnh, quạt/nhạc, cuộc nói chuyện nền; tên người/app/path, số tiền, lịch tương đối, code-switching và câu sửa lời. Đây là cỡ mẫu khởi đầu cần budget/consent, không phải dataset đã có.

| Tầng | Metrics/kiểm tra |
|---|---|
| Audio | VAD misses, endpoint premature/late, echo loop, wake FAR/FRR khi có |
| ASR | WER/CER với normalization công bố; critical-slot exact/semantic accuracy |
| Intent/router | Mode accuracy, under/over-escalation, missing-info recall |
| Actions | Verified task success, tool/argument correctness, unwanted side effects |
| Reliability | Duplicate effects, resume success, unresolved-effect rate |
| Notifications | Usefulness, omissions, false completion, interruption/duplicate rate |
| Operations | p50/p95 latency, cost/completed task, retry cost, idle RAM/CPU/battery |

Báo theo nhóm giọng, noise, task và thiết bị; nêu sample size/confidence intervals khi phù hợp. Với agent stochastic, nhiều trial và pass-rate theo task; không chọn một demo thành công làm bằng chứng tổng quát.

## SLO đề xuất để thử nghiệm

- p95 barge-in stop ≤250 ms từ lúc xác định người dùng ngắt lời tới khi audio output dừng.
- p95 phản hồi hữu ích cho simple task ≤2,5 giây từ end-of-utterance, trên cấu hình và mạng công bố. Acknowledgement “đang kiểm tra” không được tính là phản hồi hữu ích.
- Không observed unauthorized effects, stale approvals được chấp nhận hoặc blind repeated external effects trên release suite. “0 lỗi quan sát” chỉ là kết quả hữu hạn của suite, không bảo đảm tuyệt đối ngoài đời.
- Completion chỉ phát sau verifier; failure cần có state/receipt và đường xử lý, không loop vô hạn.

Chưa có benchmark đạt SLO. Ngưỡng task success, update usefulness và cost cần chốt sau baseline và xác định rủi ro từng connector; không bịa con số chính xác khi chưa có phân phối dữ liệu.

## Fault/security matrix bắt buộc

Crash trước effect; crash sau effect trước receipt; mất mạng khi waiting approval; app focus đổi; folder biến thành junction; token expired; grant revoked; locked/sleep/resume; disk full; SQLite locked/corrupt backup; TTS ngắt; child worker treo; budget vượt; cancellation giữa dependent steps.

Red-team injection qua webpage, file, tool description, memory, speech từ loa; thử đọc secret, viết ngoài root, upload file local lên cloud và gửi/publish bằng approval stale. Authenticated IPC chống caller không được phép. Signed build/update kiểm tra rollback và migration; kill switch dừng connector có vấn đề.

## Privacy/operations gate

Data inventory, raw-audio opt-in/retention/delete/export; redacted traces; connector token rotation/revocation; model/data/code license audit; user-visible active mic và grants. Thiết kế quy trình xử lý lỗi/incident, support và canary theo nhóm người dùng. Không tuyên bố đáp ứng pháp luật chỉ bằng một checklist.

## Mô hình chi phí

```text
cost/task = STT(audio minutes)
          + model(input/output tokens, retries)
          + TTS(generated audio/text)
          + tool/API charges
          + allocated compute/storage/observability
```

Đo cả failed tasks và phần verification/reconciliation. Local models giảm cloud token/audio cost nhưng dùng CPU/RAM/GPU/điện và tăng tải cập nhật. Cloud tiện vận hành model nhưng thêm network latency, quota và data egress. Không chốt provider hoặc provision dịch vụ trả phí trong repo nghiên cứu này.

## Tooling

Reference core chạy Python 3.11+ không external dependencies. Desktop build sau cần Rust/Tauri và toolchain Windows phù hợp, Python sidecar packaging và các model artifacts đã audit.

Nếu dùng Vercel cho web dashboard hoặc control plane, **rất nên cài Vercel CLI bằng `npm i -g vercel`** để dùng `vercel env pull`, `vercel deploy`, `vercel logs`. CLI chưa cài trong phiên này. Trước khi chạy npm/build, nâng Node hiện tại lên phiên bản đã vá: hook của môi trường báo `24.18.0` cần ít nhất `24.22.1`; đây là local security observation, chưa đối chiếu advisory trong nghiên cứu. Không cần Vercel để chạy tray app hoặc truy cập file local. Chọn cloud integration thật chỉ sau scope/rules/credentials được xác định.
