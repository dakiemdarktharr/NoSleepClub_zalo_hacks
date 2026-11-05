# LifeMate: kết quả 123 validation và phân tích lỗi

Cập nhật 03/11/2026. Mục tiêu formal100% và checkpoint99% đều chưa đạt. Không có public score NORA, chưa submit entry và chưa đo A30. Main chưa commit/push phần đang kiểm chứng.

**Trạng thái bàn giao:** goal hiện paused. [Báo cáo tổng hợp khó khăn và bài học](nora-handoff-2026-11-03.md) bổ sung V17 terminal guard-stop sơ bộ và stage Qwen3-4B chỉ có metadata/header. Không chạy thêm thử nghiệm trong lượt lập báo cáo.

## Phép đo hoàn tất

Hai arm dùng cùng123internal-validation IDs ngoài bộ30 cũ, cùng400audio, source, SDK và assets đã pin. Không dùng1082training IDs hoặc310outerholdout. Validation đã từng qua gold replay và teacher-forced tokenization, nên đây là engineering validation, không phải sealed blind test.

| Chỉ số | A: CPU output | B: GPU output |
|---|---:|---:|
| Local historical metric mirror | 2,9810298103% | 2,9810298103% |
| Nonempty perfect | 2 | 2 |
| Failure/timeout được giữ | 79 | 78 |
| SDK observations replay khớp | 673 | 726 |
| Provenance refusals ghi nhận | 240 | 242 |
| Median model time trên đủ123mẫu | 15,880s | 15,021s |

Điểm từng mẫu:0tăng,0giảm,123bằng. Cùng actual initial input bytes ở117mẫu quan sát được;6mẫu còn lại đều lỗi trước model, không tính thiếu input là bằng nhau. Median model time giảm khoảng5,4%, chưa đạt gate20%; không promote hoặc đổi default. Số thời gian gồm failures và6model time0; không phải latency của riêng các ca thành công.

Raw archives SHA-256 A `543a972a1cabec049cca9c1e7814d9466d919125164ccc751de92892456f0c65`, B `2baa7aeb5486572fcc8cea38aeed84a0b5a58d5bdb6bff034bec62edccfc85ff`. Exact SDK replay kiểm recorded calls/results/final state; chưa độc lập suy lại toàn bộ provenance reasons. Metric source giữ SHA `efa87d2b3187abc1c1421356b7e230a41ea153b9ef52d4bc74465768453241e0`. Raw results/labels chỉ ở `.artifacts`, không public vào repo.

## Đã đọc gì và có thể kết luận gì

Codex đã đọc đủ123diagnostic cards, đối chiếu latest transcript chuẩn/thực, lịch sử user, gold calls, mọi đề xuất model phân nhóm lặp, refusal và lỗi. Mỗi card được ghi nhận xét mở trước khi chia nhóm. Đây là agent annotation, không phải human labels hoặc calibrated semantic judge. Traces gốc giữ nguyên và được hash-check. Cards chứa thông tin hậu kiểm; không đưa lại vào inference.

Sáu bộ kiểm tra deterministic tạo baseline riêng cho các bề mặt lỗi:

| Bề mặt | Phép kiểm tra và giới hạn |
|---|---|
| Giọng nói | So latest transcript đã NFC/lower/whitespace; không coi khác cách viết số là sai ngữ nghĩa |
| Phạm vi tác vụ | So multiset tên tools đã thực thi với gold; chưa xác nhận intent/điều kiện chỉ từ tên |
| Grounding | Đếm generic provenance refusals; không chứng minh đúng namespace hoặc đúng entity |
| Tham số | Giữ exact-call sequence và historical fuzzy matching; chưa thay metric BTC |
| Điều khiển hoàn thành | Đếm lặp đề xuất/lời gọi, kết thúc và missing calls; đây là proxy, không semantic judge |
| Ngân sách | Giữ mọi timeout/turn-budget/HTTP failure trong denominator |

Baseline A có16mẫu chưa có quyết định model, gồm6lỗi ASR trước model.45mẫu lặp lời gọi identical đã thực thi;35mẫu lặp đề xuất không rỗng;45mẫu có tổng240provenance refusals.78mẫu không có refusal không đồng nghĩa78mẫu grounding/intent đúng. CER latest micro trên117transcripts quan sát là18,6898%, không bao gồm6transcripts không có; đây không phải task accuracy hoặc ASR benchmark của toàn bộ train/history.

Phải tách lời gọi SDK khỏi effect đã hoàn tất: trong673calls có74calls ở14mẫu trả `ok=true, done=false`. Đã đọc toàn bộ messages của nhóm này: chẳng hạn draft có email không hợp lệ, ID thuộc loại khác, item không nằm trong menu. Những lần đó là no-op hoặc chưa hoàn tất; không gọi là đã lưu nhiều drafts, đặt đồ ăn thành công hoặc tiết lộ vị trí.599calls còn lại không có explicit failure cũng chưa tự chứng minh state change. Bản ghi chú mở ban đầu được giữ cùng errata V2; điểm và traces không đổi.

Quan sát nổi bật:

- ASR có lúc làm mất tên riêng, số điện thoại, thời điểm và cả miền yêu cầu. Có hallucinated video signoffs/promotional text. Không tự khôi phục chữ/số bằng gold hoặc hardcode ID.
- Có ca ASR giữ được yêu cầu rõ nhưng actor vẫn đổi từ chuyển khoản sang trả hóa đơn, chọn sai điểm đón/đến hoặc hủy thêm những đơn không được yêu cầu. Lỗi planner tồn tại độc lập với ASR.
- Actor thường bỏ lookup phụ thuộc, lấy tên làm ID, điền số minh họa, chọn sai một entity đã quan sát hoặc không hỏi dữ kiện còn thiếu. Guard hiện tại chặn ID chưa quan sát nhưng có thể nhận một ID từ namespace khác; không phải bộ chứng minh quyền thực hiện ý định.
- Có ca thực hiện đúng tác vụ chính rồi tiếp tục lặp, thêm effect khác hoặc bỏ bước còn lại. Cần kiểm soát tiến độ và dừng, không chỉ parser/JSON hợp lệ.
- Cùng-run server log ghi4context overflow requests6340/6250/6614/6302tokens, đều vượt6144. Có4HTTP400 trong traces; chưa join request taskID với từng sample, nên không suy rộng mọi HTTP400 ở các run khác cùng nguyên nhân. Không có bằng chứng âm thầm truncation chỉ từ flags vắng mặt.

Private notes SHA `8de1ad62865baa2bc110ded2319e9110c09fb800796c35d2346e84de83c72a18`, cards `b7e86871dba40216a479839f53bae73d36ab3237511d8f75b46c24f5f69628b1`, results `2e19d356cdbe66c0571b94df1f8ecc57d989f218d2cb090c641982b5a2906e42`. Deterministic grader source SHA `ed257f3eda950689cfa35ed8867e90e7f8ed2aee7d42121fa9d5c5a8ad0da482`. Không dùng LLM judge; judge calibration N/A. Capability drift baseline sau đó đã hoàn tất 57% trên 200 MCQs tổng hợp, xem phần cuối; full-length training capacity/resume/export và quality promotion vẫn chưa qua.

## Training capacity: V15/V16

V15 chuyển frozen tied embedding/head sang pinned CPU, dùng native embedding/linear/autograd và khôi phục tensor lưu cho backward. Full945-token logits/loss/toàn372gradient tensors khớp resident-GPU control; CUDA allocated giảm1.017.118.720bytes. Một optimizer update thực hoàn tất rồi guard dừng backward1940 ởGPUfree517.996.544bytes. Không adapter save. Archive `0f267d2a3bcf66d2d12e249f483fcc2c01cb9575a5f32862703ed75019353a25`; failure được giữ.

V16 thêm release unused cache trước table transfer, sau head forward/backward; native gradient math và mọi causal input giữ nguyên. Full1940-token logits/loss/all372gradients exact với resident control. Qua6updates945/1940/2413/2655/3496/3790tokens; guard dừng trong mẫu5177 ởGPUfree509.607.936bytes, thấp hơn512MiB. Không hạ guard, crop context, bỏ mẫu dài hay gọi capacity pass.50cache events giữ live allocation không đổi; có event giải phóng970MiB reserved. [PyTorch2.8](https://docs.pytorch.org/docs/2.8/generated/torch.cuda.memory.empty_cache.html) phân biệt unused cached memory với live tensors; release cache không chứng minh mọi activation dài sẽ vừa.

Fresh CPU audit đọc/hash đủ508.559.360elements của bảng gốc BF16→FP16, khớp hash đã dùng; không khởi tạoCUDA. Parent/group/supervisor đã hết, source pins khớp, swap0 và sampled OOM0. Min host available8.822.005.760bytes; sampled cgrouppeak4.908.933.120bytes. V16 archive7metadatafiles `8c0a511f3dcbf946beb8922751e6ed14e08fa151d207b1b8e83800130908d0ce`, audit `53c896c7dad87f6b607616651c85047697a3750f3f7f30b29c793c3932db83da`. Child status còn `running` vì SIGTERM trước lần ghi tiếp; parent terminal receipt/process check quyết định trạng thái.

Đây là training research trên16GBhost, không desktop2GB certification. Chưa full training, optimizer resume, native adapter export hay quality gain; không dùng loss/gradient parity thay điểm tác vụ.

## Quyết định tiếp theo

Ưu tiên kiểm tra chất lượng model/backend trên input runtime thật, hoàn tất drift suite baseline và sửa pipeline theo lỗi đã quan sát. Có thể đổi architecture; mọi ứng viên phải freeze source/inputs/profile trước đo, giữ failures và original SDK. Không thêm một prompt heuristic chỉ để chữa một ID đã đọc.

Đường SFT chỉ mở khi đủ evaluation/drift gates, capacity cho toàn bộ full-length records và save/resume/export được kiểm chứng. Nếu tiếp tục tối ưu GTX1650, giữ512MiB GPU/4GiBhost guards và kiểm đúng ngữ cảnh dài; không loại outliers. Hướng ASR khác cần audio→SDK evaluation, không promote từ CER/README speed. Public submission vẫn cần634outputs hợp lệ thực, không thể tạo điểm BTC từ train mirror.

## Đối chiếu backend và baseline hồi quy đã hoàn tất

Sáu runtime prefixes được chọn hậu kiểm để chẩn đoán, không ước lượng accuracy. Native Q4 không grammar vẫn chọn sai tác vụ ở các ca quan trọng. Nhánh HF V1 lỗi kernel GQA trước real prompt; V2 qua attention/vision/cache controls rồi lỗi wrapper tokenizer trước real forward. Hai thất bại được giữ nguyên. V3 sửa cách lấy `input_ids`, qua CPU token equivalence trên đúng sáu prefixes, rồi hoàn tất cả sáu HF outputs và EOS. Fresh CPU audit kiểm token hashes, decode đầy đủ, source pins, resources và process cleanup.

HF NF4 vẫn trả hóa đơn thay chuyển khoản, và cũng đề xuất chỉnh đèn/quạt từ yêu cầu điều hòa/nhắc việc. Có một ca HF dừng đúng sau cancellation trong khi native lặp read; có khác biệt định dạng/chọn hành động ở các ca khác. Kết quả chưa xác định riêng tác động của quantization, kernels hoặc model preparation, chưa chứng minh numerical equivalence và không tạo task score. Không có căn cứ chỉ đổ lỗi cho GGUF/grammar. V3 archive SHA `ba0cae7228e36cb474daa9b65368a52c6af7d3337df4a4e6953289fdc0cd3e1e`; audit `3a11c1dee010593b67bef1b7344a88f9888d1b04e7e698de33a978d9cc8d578f`.

Đã freeze và đo **200 câu MCQ tổng hợp, 199 unique model inputs**, chia đều bốn nhóm. Đọc raw logits/log-softmax của token A/B/C/D từ HF base chưa gắn adapter; không sinh rồi parse đáp án hoặc dùng LLM judge. Labels chỉ nằm ở scorer local sau inference. Kết quả là **114/200 = 57%**, theo nhóm: thời gian24/50, số lượng15/50, evidence35/50, safety/completion40/50. Đây là capability-drift baseline của những template này, không phải điểm LifeMate, MMLU hoặc thước đo an toàn production.

CPU trước/sau inference xác nhận mọi prompt/token hashes; đủ200rows, không bỏ mẫu, source/resource/cleanup audit qua. Archive `1370f3651ae2023c521ef0c72a515e3e3533aeafcda9db411ef64a8674a9b9db`; per-item results `2ea73edb9185d06f1e5d2b355567bf4e7b8e7fad94744c147e3dfccca61c9ec2`. Cặp baseline giữ riêng **native audio→SDK 123** và **HF NF4 drift200**, không trộn scores hoặc mặc định cùng numeric backend. Các checkpoint sau phải so lại đúng từng profile. File đo ở `runs/`, bộ đánh giá đã freeze ở `eval/`; chưa full training capacity/resume/export certification.
