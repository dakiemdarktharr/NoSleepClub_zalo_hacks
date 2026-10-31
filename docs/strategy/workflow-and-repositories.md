# NORA: workflow đầy đủ và bản đồ repositories

**Thiết kế đối chiếu ngày 31/10/2026; implementation cập nhật 01/11/2026.** ADR-0005 đã chấp nhận hướng Rust cho foundation. Repo hiện có Cargo workspace 5 crates, Cargo.lock, audio/native hosts, VieNeu Rust port và state/policy primitives. [Bản đồ đã xây và chưa xây](../rust-foundation.md) là nguồn trạng thái hiện tại; các vòng workflow đầy đủ dưới đây vẫn cần tích hợp desktop/LLM/broker.

## 1. Cấu trúc tổng thể: ba vòng phối hợp

NORA cần ba vòng có vòng đời riêng: **voice interaction**, **durable goal execution** và **event-driven updates**. Một goal dài tiếp tục làm việc trong khi audio/UI phản hồi; tắt TTS không đồng nghĩa hủy goal. Tất cả model requests và external effects đều qua broker/policy tương ứng.

~~~mermaid
flowchart TD
    U[Người dùng] --> A[Mic/PTT, audio processing, VAD, endpointing]
    A --> S[ASR tiếng Việt và critical-slot checks]
    S --> R{Router: complexity và risk}
    R -->|Trả lời| Q[Answer proposal]
    R -->|Một action| B[Broker: quyền, schema, target, provenance]
    R -->|Nhiều bước| G[Goal, plan, checkpoints, budgets]
    G --> W[Workers logic, LLM API qua egress gate]
    W --> G
    G --> B
    B -->|Thiếu quyền hoặc thông tin| H[Hỏi đúng phần cần quyết định]
    H --> U
    B -->|Đủ điều kiện| T[Typed app/file/API adapters]
    T --> V[Readback, receipt, reconciliation]
    V --> G
    B --> E[Trusted runtime events]
    V --> E
    G --> E
    Q --> E
    E --> N[Update policy: voice, card hoặc im lặng]
    N --> O[TTS, playback và UI]
    O --> U
~~~

Q ở đây là nội dung trả lời, không phải lời model tự xác nhận tool success. Router không thể bỏ egress/grant checks cho fast path. Worker không gọi app hoặc mạng ngoài broker; LLM API được gọi qua lớp credentials/data policy dù diagram rút gọn không vẽ mọi HTTP hop.

## 2. Setup trước lần giao việc đầu tiên

1. Xác định CPU features/GPU/driver và model pack phù hợp. Chưa có benchmark chứng minh mọi laptop 15–25 triệu đạt profile. Không lấy giá máy thay support matrix.
2. Chọn microphone/speaker; cấp mic permission, PTT/hotkey và trạng thái mute rõ. Wake word “Nora ơi” là giai đoạn sau khi có model/license/dataset và FAR–FRR tests.
3. Chọn folder read/write và apps/accounts/capabilities được phép; quyền có scope, expiry nếu cần, màn hình revoke. Chọn folder không tự cấp quyền upload nội dung.
4. Cấu hình LLM provider/account/quota và data-egress policy. Local ASR/TTS vẫn có thể gửi transcript/context tối thiểu tới LLM API theo quyền đã cấp; không gọi toàn sản phẩm là offline.
5. Cài model packs từ manifest đã pin/hash/license-reviewed; chưa tải model hoặc tạo installer trong phiên lập kế hoạch. Không auto-run code từ model repository.
6. Nếu dùng OA pilot: admin cấp OA/app scopes và nguồn order/policy có quyền. Shared OA app secret ở backend/BFF; không nhúng vào desktop. Có thể tiếp tục dùng synthetic workflow khi chưa có partner, nhưng UI/demo phải ghi rõ.

Secrets được giữ ở broker/OS secret protection hoặc backend phù hợp; không vào frontend/model context. Autostart là lựa chọn của user, chạy user-session tray thay vì dựa Windows Service cho mic/UI.

## 3. Voice loop

| Bước | Công việc | Output và giới hạn |
|---|---|---|
| Capture | CPAL lấy audio từ thiết bị, buffer bounded | Audio callback không chạy LLM/inference blocking |
| Audio processing | Mono/resample theo model, echo/noise handling theo profile | CPAL chỉ là I/O; không coi nó là AEC. Speaker mode cần playback reference/AEC tests |
| VAD | Silero nhận diện đoạn có speech | Không xác thực người nói, không tự biết user đã nói hết ý |
| Endpointing | Xác định kết thúc lượt từ pauses/context/time limits | Không thực hiện effect từ partial transcript chưa chốt |
| ASR | Whisper large-v3-turbo qua native engine, quantized pack cần đo | Transcript/segments và các trường cần làm rõ, không sửa số/người nhận theo suy đoán |
| Intent | Normalize context, resolve entities, phân biệt explicit/inferred/unknown | Critical slot mơ hồ phải có lối hỏi đúng trường |
| Reply/TTS | Text đã qua policy → phonemizer/tokenizer → VieNeu graphs/streaming → CPAL playback | Hỗ trợ cancel/flush; chỉ phần đã phát được coi user đã nghe |

Full-duplex/barge-in qua loa phải được thử riêng: AEC candidate chưa chứng minh Windows compatibility/footprint. PTT/headset là profile đầu để thử, không quảng cáo rằng VAD một mình giải quyết echo. User ngắt lời thì dừng/flush playback và ưu tiên capture; goal chỉ pause/cancel khi ý định đó được xác định.

## 4. Router: chọn mức thực thi cần thiết

| Loại | Ví dụ | Đường thực hiện |
|---|---|---|
| Answer | Hỏi giải thích hoặc dữ kiện trong context | API khi cần, trả lời có nguồn/giới hạn phù hợp |
| Bounded action | “Tạo note cuộc họp trong folder này” | Resolve target → broker → adapter → verify |
| Durable goal | Nghiên cứu, đối chiếu nhiều nguồn, tạo báo cáo và lưu | Goal/plan/dependencies/checkpoints, bounded workers khi đáng dùng |
| Missing input/permission | Chưa rõ người nhận hoặc app/folder chưa được cấp | Chỉ chờ nhánh liên quan, hỏi fields/scope cần thiết |
| Unsupported | Không tool đã hỗ trợ nào xử lý được | Báo đúng giới hạn; không cài app/bịa tool để ép làm |

Complexity và risk độc lập. Lệnh tài chính ngắn vẫn có thể high risk. Một task read-only dài không tự cần approvals cho mỗi bước. User authorization còn hiệu lực được tôn trọng, không hỏi lại chỉ vì chuyển worker.

## 5. Goal và workers

Goal contract dự kiến lưu objective/version, entities, required outputs, postconditions, allowed data/effects/destinations, deadline, token/spend/action budget và trạng thái. Các acceptance checks không được worker tự sửa để dễ báo hoàn thành.

Default một orchestrator. Khi có nhánh độc lập đáng chi phí, tạo workers API logic với context tối thiểu, scope con, output schema/evidence và timeout. Ví dụ research worker tìm nguồn; data worker đối chiếu order; reviewer kiểm tra consistency. Không cần mỗi vai trò là một process/model local, không mặc định luôn tạo đủ mọi vai trò.

Mọi router/planner/worker/critic request cần egress check **trước** serialization/transmission. Worker output là proposals và evidence có provenance; không tự nâng trust qua summary. Model không thấy secrets, không ghi success receipts, không có generic shell để vượt adapter.

State dự kiến: queued → running → waiting_input / waiting_permission / waiting_approval → running → verifying → completed. Có failed, cancelled và needs_reconciliation; retry/replan có giới hạn. Checkpoints không tự bảo đảm exactly-once với API bên ngoài.

## 6. Broker và NORA Giữ Ý

Trước hành động, broker kiểm tra tool/schema version, current grants/cancellation, actor/account, canonical target, field authority/freshness, data destination và budget. Nếu cần approval thì native UI hiển thị exact content/target/diff; approval bind version/args/content/expiry. Worker đổi tham số không được dùng lại approval cũ.

Khi gặp injection/conflict, không coi message/file/tool result là quyền mới. Dừng các bước phụ thuộc chưa đủ điều kiện; tiếp tục phần độc lập đã xác minh; tìm nguồn có authority hoặc hỏi đúng thay đổi trọng yếu. Không coi mọi thay đổi là tấn công, cũng không mặc định record cũ luôn đúng. Cơ chế này là product hypothesis cần so baseline critic+confirm, không phải algorithm novelty đã chứng minh.

Ví dụ đổi hàng: attachment và order record mâu thuẫn địa chỉ/customer. NORA vẫn kiểm tra eligibility và tạo draft/note; cập nhật địa chỉ/tạo shipping label chờ xác nhận đúng actor theo policy. Thay đổi hợp lệ sau xác nhận phải chạy được. Goal chỉ “soạn” không tự biến thành “gửi”.

Các nền tảng thư viện không cung cấp thay NORA phần domain authority này. Grant, provenance, capability delegation, safe continuation và effect receipts phải được thiết kế/test trong mã dự án.

## 7. Adapters và verification

Ưu tiên official app APIs; file/note qua broker scoped filesystem; Windows APIs/UI Automation cho app/operation đã test; browser chỉ khi cần và có resource/domain scope. Không dùng personal Zalo bot SDK làm nền production.

Trước effect lưu intent/action ID; dùng idempotency nếu provider hỗ trợ. Sau effect đọc lại app state/receipt. Timeout hoặc mất ack không có nghĩa chưa gửi: vào reconciliation, query trước retry. Nếu không đủ query/receipt thì handoff/manual resolution. Full completion chỉ khi toàn bộ authorized postconditions đạt; partial progress hiển thị đúng là partial.

OA cung cấp conversation/actor trong scope, không tự là OMS. Nguồn order/policy cần fixture tổng hợp, CSV có quyền hoặc OMS connector với customer/order mapping. Send eligibility/fees/token rotation theo contract hiện hành; không giả định public SIMO/bank/report API. [PRD tích hợp](nora-guarded-completion-prd.md).

## 8. Update loop

| Event đã xác minh | Default dự kiến |
|---|---|
| Bắt đầu goal dài, milestone hữu ích, findings mâu thuẫn | Card, voice ngắn khi có ích |
| Worker thực sự chạy và giúp hiểu tiến độ | Có thể cập nhật, không đọc tên mọi role/log |
| Nguồn đã mở/đối chiếu đủ để có kết luận | Count unique inspected sources từ ledger, không đếm result slots |
| Thiếu quyền/critical info hoặc blocker cần user | Hỏi cụ thể qua voice/card |
| Epoch train, skill init, heartbeat không đổi, retries đã khôi phục | Im lặng; log theo nhu cầu |
| Completion/failure không tự sửa được | Kết quả, nơi lưu, verification status và phần còn thiếu |

Event policy chọn nói/card/im lặng, gộp trùng và tôn trọng quiet mode/barge-in. Không phát raw reasoning, secrets hoặc nguyên prompt injection. TTS không được tự bịa progress từ text model. [Policy hiện có](../progress-policy.md).

## 9. State và chạy nền

SQLite dự kiến giữ goal/checkpoint, effect ledger, notification outbox và facts có nguồn/TTL theo quyền. Conversations/audio có retention riêng; không lưu raw audio lâu mặc định. Secret store tách khỏi task state. Full-text/structured search trước, chưa cần vector DB service cho V1.

Đóng UI có thể giữ tray theo user setting; Quit tắt mic/process. Khóa màn hình mặc định mute capture/dừng UI actions; chỉ background tasks đúng quyền tiếp tục. Sleep/offline lưu state; resume recheck grants, tokens, stale data và unknown effects. Máy tắt thì local NORA không chạy. Scheduled reminders cần persisted due-time/timezone và recovery, không chỉ Tokio timer trong RAM.

Ngân sách 2 GB tính toàn cây process, WebView/browser/native hosts; GPU và disk/cache/update staging cũng phải đo. Heavy jobs cần scope/compute budget khác hoặc handoff. Không âm thầm chuyển audio cloud khi local OOM.

## 10. Repositories chính dự kiến dùng

Đây là **bản đồ upstream của thiết kế**, gồm cả phần đã dùng và phần chưa tích hợp. Cargo.lock hiện pin foundation thực tế; Tauri, Tokio/reqwest cho app integration và direct Windows adapter vẫn là phần sau. Dùng crates/bindings/model artifacts, không clone/ghép toàn bộ mọi repo. Model packs audit license riêng; danh sách không phải toàn bộ transitive dependency graph.

| Lớp | Repository | Vai trò trong phương án Rust |
|---|---|---|
| Dự án gốc | [NoSleepClub_zalo_hacks](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks) | Monorepo mã NORA, contracts, adapters, evaluation và docs |
| Desktop/tray | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | UI/tray on-demand; có WebView, native UI là arm so sánh |
| Plugins desktop | [tauri-apps/plugins-workspace](https://github.com/tauri-apps/plugins-workspace) | Chọn ít plugins như dialog/autostart/updater; không mở generic FS/shell cho frontend |
| Audio I/O | [RustAudio/cpal](https://github.com/RustAudio/cpal) | Capture/playback, Windows WASAPI; không thay AEC |
| Resampling | [HEnquist/rubato](https://github.com/HEnquist/rubato) | Chuyển sample rates/buffers theo audio models |
| VAD | [snakers4/silero-vad](https://github.com/snakers4/silero-vad) | Model VAD ONNX; không authN/endpointing đầy đủ |
| ASR engine | [ggml-org/whisper.cpp](https://github.com/ggml-org/whisper.cpp) | Whisper large-v3-turbo quantized và CPU/GPU backend cần đo |
| Rust ASR bindings | [whisper-rs trên Codeberg](https://codeberg.org/tazz4843/whisper-rs) | Rust host cho whisper.cpp; pin version tương thích |
| ONNX engine/bindings | [microsoft/onnxruntime](https://github.com/microsoft/onnxruntime), [pykeio/ort](https://github.com/pykeio/ort) | Silero/VieNeu native inference từ Rust; vẫn có C/C++ libraries |
| TTS reference/assets | [pnnbao97/VieNeu-TTS](https://github.com/pnnbao97/VieNeu-TTS) | v3 Turbo graphs/reference behavior; host Rust đã port preset FP32 CPU path, còn quality/full SDK gates |
| Phonemizer/tokenizer | [pnnbao97/sea-g2p](https://github.com/pnnbao97/sea-g2p), [huggingface/tokenizers](https://github.com/huggingface/tokenizers) | Text/phoneme/token mapping giữ behavior TTS |
| Async runtime | [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | Async I/O, scheduling/cancellation; không tự thành durable goal engine |
| HTTP clients | [seanmonstar/reqwest](https://github.com/seanmonstar/reqwest) | LLM/OA/API traffic qua broker policy; chưa chốt LLM provider/model |
| Typed serialization | [serde-rs/serde](https://github.com/serde-rs/serde), [serde-rs/json](https://github.com/serde-rs/json) | Messages/config/tool arguments; JSON hợp lệ chưa đủ authorization |
| Local state | [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | SQLite access; NORA tự xây state/effect semantics |
| Windows integration | [microsoft/windows-rs](https://github.com/microsoft/windows-rs) | Windows API bindings; operation support/permissions phải test |

**Sửa link quan trọng:** GitHub cũ tazz4843/whisper-rs đã archived; README nói chuyển sang Codeberg và tiếp tục maintenance. Không chọn repository GitHub archived làm upstream mặc định. [Migration notice](https://github.com/tazz4843/whisper-rs/blob/master/README.md).

## 11. Ứng viên/tùy chọn, không cài mặc định tất cả

| Nhu cầu | Repo ứng viên | Gate trước khi thêm |
|---|---|---|
| AEC/noise processing | [tonarino/webrtc-audio-processing](https://github.com/tonarino/webrtc-audio-processing) | Windows build/runtime/license, playback reference, echo quality, CPU/RAM; wrapper có native C++ và version caveats |
| MCP connectors | [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) | Chỉ approved servers/tools, version/auth/scope; SDK không sandbox server hoặc cấp quyền filesystem |
| Browser automation | [mattsse/chromiumoxide](https://github.com/mattsse/chromiumoxide) | CDP/browser access scope, current browser compatibility, reliable effect verification và whole-tree memory |
| Native UI thay WebView | [emilk/egui](https://github.com/emilk/egui) | Matched UX/accessibility/resource tests; không ship đồng thời hai UI chỉ để dự phòng |
| Rust LLM SDK | [0xPlaygrounds/rig](https://github.com/0xPlaygrounds/rig) | Chỉ components thực sự cần; không tự thêm shell/plugins/coding-agent powers; không thay durable/security runtime |
| Wake word | Chưa chốt repo/model | Vietnamese “Nora ơi”, license/data rights, FAR/FRR, noise/replay, idle resources |

API-first là mặc định. Chromiumoxide là client Rust nhưng browser vẫn là process nặng cần tính vào 2 GB. OpenClaw, LangGraph, Pipecat, LiveKit không phải các dependency mặc định của phương án client Rust này; chúng là prior art/phương án khác để đối chiếu khi có nhu cầu. Agency-agents/skills dùng hỗ trợ quy trình phát triển không tự trở thành runtime dependency của NORA.

CaMeL/Fides/Task Shield/RTBAS là prior art thiết kế/evaluation, không phải các security modules chỉ cần ghép repo là có Giữ Ý production. Xem [research landscape](../research/novelty-landscape.md) và [security research](../research/novelty-security.md).

## 12. Phần cần tự xây trong repo NORA

Foundation đã có core contracts/router, resource/permission types, SQLite goal/effect transitions, notification policy/acknowledgements, Rust audio/TTS host và CLI verifier. Còn phải tích hợp supervisor/scheduler, broker/IPC/grants enforcement, Giữ Ý/provenance/data-flow runtime, production outbox, LLM/app adapters, OA gateway, LifeMate adapter và release tooling. Có primitives không đồng nghĩa các vòng sản phẩm đã chạy end-to-end.

Không repo upstream nào hiểu thay mình domain authority của merchant, quyền user, semantics đúng của external effect hoặc lúc nào nên nói. Đây là phần sản phẩm và trách nhiệm vận hành của NORA. Port policy Python phải giữ behavioral tests, không bỏ kiểm tra chỉ để claim Rust nhanh/nhẹ.

## 13. LifeMate và đường phát hành

Competition adapter nhận audio/context/history/provided tool schemas → ASR/intent → tool planning/execution trong simulator → ordered calls + state hash lấy từ environment. Missing info/unsupported theo devkit; không thêm desktop notifications/approval calls ngoài contract. Hosted API/network rules và data rights còn cần xác minh trước integration. [LifeMate](../lifemate-2026.md).

Build/release: pin code/runtime/model manifests → tests/CI/licenses/dependency review → matched voice/task/security/hardware evaluation → signed installer/update → partner pilot → independent review/rollback/incident drills → release cho support matrix đã khóa. Repo public không chứa competition/private/customer data hoặc credentials.

**Next implementation slice:** mic/text → ASR → một scoped note action → readback → event/TTS; đồng thời dựng evaluator/resource harness. Chỉ mở rộng goal/workers/Giữ Ý sau baseline. Các dependency/model/Windows choices ở trên cần spike trước chốt; tài liệu này không chứng minh NORA đã chạy hoặc đạt 2 GB.
