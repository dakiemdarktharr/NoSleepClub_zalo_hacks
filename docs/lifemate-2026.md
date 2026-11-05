# LifeMate 2026 và NORA

## Trạng thái tiếp tục sau rà dự án — 03/11/2026

Q3_K_M của Qwen3-4B-Instruct-2507 đã tải/verify theo SHA `9c6e0763577125a994a9bea0bbd7a737ac4498b8a6a4e0f788727553af1806c9`, 2.075.618.400 bytes. Trong protocol Q3 riêng, cả hai smoke ASR đã có 3/3 complete dưới30s, SDK replay0mismatch/0reject; observed release flags đều truncated0. Min sampled GPUfree A/B 585.105.408/486.539.264bytes (>256MiB guard). Đây chưa là score hoặc hard peak proof.

Cặp Q3 đầu cuối đã hoàn tất: **Whisper turbo 8,3333%; PhoWhisper 3,3333%**, cùng30development và mỗi arm1generation deadline failure giữ trong denominator. Nonempty-perfect0/1; state matches7/7; cả hai SDK replay30/0mismatch/0reject và recorded observations/state khớp. Pho giảm5pp, không đạt task gate, không promote. V1 auditor có assertion dành cho alias bị áp nhầm cho baseline; V2 ghi correction hậu kiểm và giữ mọi call/args/output/score nguyên trạng. Inner ask-policy violations A/B5/9 được ghi riêng, không bị che bởi SDK acceptance. Truncation coverage chưa chứng minh; observed flags không có giá trị khác0. Thay Q3 đồng thời với ASR trong so sánh historical Q4 không tạo đối chứng quantization; cặp mới chỉ cô lập ASR dưới cùng Q3. Tham khảo [review dự án](development/project-review-2026-11-03.md).

Rust có mode opt-in `compact-typed-clarify-experimental`: constrained schema cho đúng target/params và guard trước SDK, giữ optional fields; membership không chứng minh missingness. Các snapshot v30/v31 build/test riêng trên lattice, không thay v29 hoặc default mode.

**Kết quả v30 sau đó:** cặp30 cùng source/ASR/LLM cho baseline8,3333%, clarification5,0000%; nonempty-perfect0/0, failure1/1; replay SDK exact, initialinputs30/30khớp. Không promote.8refusals là cùng một params-list có phần tử trùng, lặp đến deadline; native không hỗ trợ `uniqueItems`. V31 thay constraint này bằng enum mọi danh sách không trùng có thứ tự trong giới hạn tài nguyên; Windows287tests/Linux106tests qua,2native synthetic probes qua. Chưa có task score v31. Tham khảo [review](development/project-review-2026-11-03.md).

Các profile E2E Q4 mới đã dừng theo gate: allGPU/6144 dừng self-imposed GPUfree256MiB guard khi sampledfree172MiB (chưa OOM);31layers/6144 có3/3generation deadlines;34layers/5120 có2/3deadlines. B/full của những profile đó chưa chạy. Chúng cho thấy đánh đổi memory/latency của các profile đã thử, không chứng minh PhoWhisper kém hoặc mọi Q4 đều không thể chạy. Driver-reserved thực tại thời điểm guard chưa được sample trực tiếp; phần dư total−used−free chỉ là suy luận. Giữ full rawreceipts/protocols riêng.


Cập nhật **03/11/2026, Asia/Saigon** từ portal đã đăng nhập và devkit chính thức. Adapter đang được phát triển cục bộ; chưa gửi submission hoặc có điểm NORA trên leaderboard. [Portal LifeMate](https://challenge.zalo.ai/portal/lifemate)

## Tiếp tục trên lattice — 03/11/2026

**V13 training update/save/reload đã qua kiểm toán:** một full945-token internal-train example, NF4 Qwen3.5-2B/rank16 trên mọi186text linear modules (16.819.200trainables),1AdamW8bit update thực và same-process new-adapter reload. Independent CPU audit kiểm372safetensors tensors finite/shape/rank/coverage,186Bnonzero; tensor/logit roundtrip exact theo receipt. Bỏ unused vision storage giảm190.087.680CUDAallocatedbytes, synthetic checkpoint CPU-transport123tokens khớp loss/allgradients. Scope kernel8GiB/swap0, host4GiB/GPU512MiB guards; minsampledGPUfree1.220.542.464bytes, peakcgroup2.432.143.360bytes. Chưa đo chất lượng, full-training fit hoặc fresh-process model reload; adapter không promote. V12 global-counter guard failure giữ riêng, scope policy change được ghi rõ trong [review](development/project-review-2026-11-03.md). Weights ở lattice,6metadatafiles/archive riêng SHA `177f178187964a0035febf5c48e8de8027e701b61bd457f49b405832a96d0728`.

Mục tiêu formal là **100% điểm tác vụ thực**; **99% là checkpoint trung gian**. Cả hai chưa đạt: kết quả audio tốt nhất đã chấm vẫn là 13,5556% trên 30/1.235 mẫu development. Holdout 310 mẫu chưa được đánh giá, chưa có public/private score hoặc entry NORA. Các mốc Colab phía dưới được giữ như nhật ký lịch sử từng snapshot.

**V11 — real train-only backward capacity, chưa full training fit:** 7 ví dụ đã được chọn trước, tất cả thuộc internal training; không chọn validation, outer holdout, benchmark30 hoặc public. Mỗi ví dụ dùng một process mới, giữ full causal input và thực hiện đúng một backward, 0 optimizer steps.

| Full tokens của ví dụ | Kết quả V11 |
|---:|---|
| 945 | Backward được accepted, 24 gradients hữu hạn |
| 1.940 | Backward được accepted; GPU free thấp nhất 542.310.400 bytes |
| 2.413 | 24 gradients hữu hạn, rồi guard dừng; không accepted case |
| 2.655 / 3.496 / 3.790 / 5.177 | Chưa chạy sau lần dừng đầu tiên |

Ví dụ 1.940 chỉ dư **5.439.488 bytes (~5,19 MiB)** trên guard 512 MiB. Với ví dụ 2.413, GPU free xuống **424.869.888 bytes**; cả current-free check và baseline-adjusted peak-reserved check đều không đạt. Peak CUDA allocated/reserved là **3.144.938.496 / 3.290.431.488 bytes**. Đây là self-imposed guard stop, **không OOM**; không hạ guard hoặc tự retry. Length riêng lẻ chưa chứng minh mọi input cùng độ dài đều fit.

Independent audit đã kiểm tra hai private gradient payloads của hai cases accepted; case dừng chỉ có gradient metadata hữu hạn, không có payload accepted. Reader tài liệu chỉ đọc source/selection hashes và metadata, không tải raw training text hoặc công bố private IDs/gradients. Toàn plan vẫn incomplete: **0 optimizer/update/adapter save**, chưa sustained same-process training, full-example capacity, numerical GGUF/export parity hoặc chất lượng tác vụ. Token/template parity và V10 synthetic parity giữ phạm vi riêng. Audit SHA `56dec768f5d3a9e23885f544daa2647556d12124b8bfd2b385d2a6b4c6b73611`; evidence riêng trong `.artifacts/qlora-real-train-result-v11/`. Các số GPU là saved checkpoints/Torch peaks, chưa chứng nhận global peak hoặc ngân sách desktop.

**PhoWhisper Q5 — ASR-only đã đo, chưa phải điểm tác vụ:** hai full runs dùng đúng **30 development samples / 32 utterances** (30 latest + 2 user-history), cùng source/binary/native/beam1/audio đã freeze; chỉ thay ASR model. Mỗi model có một lượt full mới sau smoke riêng, không có LLM resident hoặc SDK task execution trong phép đo này.

| ASR-only, cùng 30/32 | Whisper turbo Q5 | PhoWhisper Q5 |
|---|---:|---:|
| Word edits / reference words | 204 / 526 | 170 / 526 |
| Word-edit percentage local | 38,7833% | 32,3194% |
| Character edits / reference characters | 531 / 2.179 | 437 / 2.179 |
| Character-edit percentage local | 24,3690% | 20,0551% |
| Normalized exact | 1 / 32 | 1 / 32 |
| Median ASR/sample | 2.541,13 ms | 1.601,75 ms |
| P95 nearest-rank ASR/sample | 2.585,30 ms | 1.788,48 ms |
| Max ASR/sample | 7.538,22 ms | 4.593,01 ms |

Pho có 18 utterances ít character edits hơn, 9 nhiều hơn, 5 bằng; giảm 4,3139 điểm phần trăm CER local và median sample ASR giảm 939,38 ms trong cặp đã đo. Ba numerical screening flags đã đăng ký đều đạt, **chỉ đủ ưu tiên một thí nghiệm downstream riêng**. Manual rubric về entity/negation/time/amount chưa làm; chưa có kết quả ASR→LLM→SDK mới hoặc production promotion. Chuẩn hóa NFC/lowercase, punctuation thành khoảng trắng, collapse whitespace, không đổi cách viết số; character count có spaces. Đây không phải official WER, task accuracy hoặc 100−CER accuracy.

Startup ASR quan sát A/B là3.231/2.221ms, whole-run85,65/54,91giây; không là cold-start guarantee. Full-run sampled peak RSS A/B **451.612.672 / 448.258.048**, PSS **441.567.232 / 438.287.360**, GPUdevice **966.787.072 / 1.065.353.216bytes**, swap0. Pho dùng sampled GPU nhiều hơn trong ASR-only; chưa chứng minh co-resident ASR+LLM vừa VRAM hoặc toànapp đạt2GB. Prelaunch checks host>=4GiB/GPUfree>=1GiB và sampled-runtime host>=4GiB/GPUfree>=256MiB đều đạt; sampling1s không là hardquota.

Bốn receipts report processgroupempty; collection auditor riêng xác nhận8owner/supervisorPIDs đã hết. Một PID transient vanished/zombie ở Bsmoke không còn group persistent theo các receipts/finalchecks. SIGINT/SIGTERM có owned-group cleanup; SIGKILL/powerloss không intercept được. Loopback/local-server scope được kiểm tra bằng code/receipt; OS-level egress không bị chặn trong research run. Collector hiện tại không SSH hoặc tác động model job đang chạy. Raw archive20members SHA `ad4c8a877d4504227b6fa134eb7ee2580535720b90022acf2ac4827f0b4d49c7`; local readback SHA `9d62a7ec526a8ecba05ce884d457bbd72740cf7fcf07e1d163621fc062293f60`. Score/reference/transcripts đều riêng tư, không đưa vào repo.

**Phạm vi holdout trong scoring:** historical task scorer SHA `efa87d2b3187abc1c1421356b7e230a41ea153b9ef52d4bc74465768453241e0` đã deserialize toàn bộ train/gold receipts trước khi chọn predictions, gồm outer-holdout rows trong process memory. Điểm cũ chỉ chấm selected development; chưa có holdout model score/tuning hoặc bằng chứng labels vào model. Vì vậy “holdout chưa đánh giá” **không có nghĩa labels chưa từng được parse trong process scoring**. Giữ nguyên outputs/scores/hash lịch sử; filtered scorer và equivalence tests đang được chuẩn bị. ASR scorer v2 riêng lọc IDs/fields trước decode values: chỉ đọc development references hậu kiểm sau khi hai outputs hoàn tất, không deserialize holdout references/labels; dataset bytes vẫn được đọc cho hash.

**V10 numerical control:** independent audit pass12children/12comparisons, mỗichild đúng1synthetic backward,24/24gradients byte/numeric equal, objective/loss và active-tile logits hashes khớp. Shared LoRA/software; minGPUfree573.767.680bytes, minhost11.557.945.344bytes;0optimizer/noOOM/adapterupdate. Phạm vi là **cross-process synthetic**: same-process full multi-target/EOS, ordinary full-head parity và real-example training/update/save capacity/quality vẫn chưa chứng minh. Audit SHA `5a5341333769a7132d77cea5e585e10d86deb2dbd5636957a323daba15921e51`; saved counters chưa chứng nhận continuous global peak hoặc desktop cap.

**Cặp alias v29 đã chấm xong — loại profile alias này:** A/B dùng cùng source v29, context 6.144, model/ASR/SDK/deadline và đúng 30 IDs đã freeze; A compact-typed, B descriptive name/schema aliases.

| Paired 30 v29 | Điểm local | Perfect / nonempty-perfect | State matches | Failed |
|---|---:|---:|---:|---:|
| A — baseline mới | 8,3333% | 2 / 0 | 5 | 0 |
| B — alias experimental | 6,6667% | 2 / 0 | 5 | 0 |

B giảm 1,6667 điểm phần trăm: 0 mẫu tăng, 1 giảm, 29 giữ điểm. Cả 2 perfect/arm đều zero-action; nonempty-perfect 0/0. Profile không đạt gates gain >=10pp và thêm >=2 nonempty-perfect nên **không promote**, dù cả 60 mẫu complete dưới 30 giây và SDK integrity pass. Kết luận áp dụng rule/profile đã freeze, chưa loại mọi phương pháp alias hoặc chứng minh thống kê từ 30 development đã được phân tích trước.

Collection giữ 37 raw files/run và immutable hashes; source/assets/executable/supervisor/params/IDs khớp. Initial inputs khớp bytes 29/30; một khác biệt history có typed tool name được đổi đúng design, inverse mapping chỉ những field đó trả về equality 30/30. Independent SDK replay 30/arm có 0 mismatch/reject; 52/53 actual observed results và final receipts khớp, 5 unexecuted refusals/arm giữ nguyên. Full-split 147-entry alias map chuyển proposal về tên SDK gốc, 0 mapping errors; 75 responses/releases/arm đều truncated0, không OOM/CUDA/context/transport/deadline errors. Receipt/mapping hợp lệ chưa chứng minh hiểu đúng intent.

Sampled peak PSS A/B là **1.182.103.552 / 1.184.163.840**, RSS **1.272.700.928 / 1.274.773.504**, GPU device **3.751.804.928 bytes** cả hai; swap0, sampled mỗi giây. Prelaunch guards không được auditor hậu kiểm xác nhận độc lập; sampling chưa chứng nhận cap desktop 2 GB/SSD 15 GB/A30. Giữ nguyên source/SDK/gold/scorer/membership; best historical 13,5556% không đổi, chưa có holdout hoặc điểm chính thức.

Evidence: `.artifacts/lifemate-alias-v29-paired-audit-result-v1/paired-aggregate.json` SHA `b494e03028f6463128f64c32a0c0c579c8e382da09c89de35474f718fe886781`; independent readback SHA `93d581f5641e80a6cbc45547283dcf294d60bb94becbb67d0afe4a111719aa10`. Raw collections/replays ở `.artifacts/lattice-runs/alias29-{a,b}-paired030/`, archive SHA A `1154df32784ef70fea4e9b8f06fcbe1236411a2d73c9642d3fa4114238337dd5` / B `4f245b60855515ab57375e44291c2b36ff26aff7648228c99d42a96e6f88bfaa`.

**V29 — source/build, token probe và technical smokes:** source mới `ad76310ddacc7c5e8ad53801ca681e33e0992f5baefceb1eee2e2680a96c8b48` có 127 files được kiểm tra từ bundle. Linux log ghi **99 tests pass, 3 ignored** (73 lib, 19 workflow, 4 alias, 1 CLI, 2 example); release build hoàn tất. Mode opt-in `compact-typed-alias-experimental` đổi tên công cụ model-visible theo catalog, rồi map structured proposals về tên SDK gốc trước security checks. Bijection dùng full split catalog 147 tools; user text/raw results không bị thay tên toàn cục, permissions/SDK/state receipts vẫn giữ quyền quyết định. Đây chưa phải default hoặc một model mới.

Alias token probe dùng 75 observed historical prefixes/arm: A thật từ v19, B là counterfactual translation theo Rust, không phải B đã sinh. Pinned HF/GGUF có 0 render/token-ID mismatch trên 150 rows, dùng **actual matching decision schemas**. Max prompt A là 3.223, B là 3.256; B + 768 reserve = 4.024, còn 2.120 tokens ở context 6.144. Old A khớp 75 render hashes/counts; readback chỉ ra non-system changes nằm ở structured tool fields. Có 0 generation/GPU/network; timing không isolated. Chưa bảo đảm trajectory mới, constrained generation, training fit hoặc quality.

Hai smoke v29 đã collect/replay: **A 3/3 và B 3/3 complete dưới 30 giây**, IDs/order/pins/parameters khớp trừ mode/tag. Input đầu khớp bytes 3/3; SDK replay có 0 mismatch/rejection cả hai, actual observations 7/arm và final receipts khớp. Alias proposals được kiểm tra về tênSDK gốc qua 147-entry map, 0 alias errors; 8 response/release records mỗi arm đều truncated0, không OOM/CUDA/context/transport/deadline errors. Technical screen đạt trong scope hậu kiểm; cặp30 mới sau smoke đã chấm riêng ở trên và profile alias bị loại. Sample resources không chứng nhận hard cap, A30 hoặc production.

Original prereg có lỗi chép manifest hash bị lặp substring. Original file được giữ nguyên; erratum SHA `c376c477851d9f246cd26735d3370367639db9a5122af228b54dea3d4dd3988f` ghi rõ **sau A smoke, trước B**, chỉ sửa transcription theo ZIP/ready đã pin; không thay source, cohort, mode, profile hoặc thresholds. Source/pins thực của hai smoke đã khớp độc lập. Evidence: `.artifacts/v29-phowhisper-metadata/continue-v29.log`, `.artifacts/lifemate-alias-v29-token-result-v2/independent-readback.json` và `.artifacts/lattice-runs/alias29-{a,b}-smoke003/`.

**PhoWhisper assets mới trên lattice:** receipts cho biết F16 đã chuyển đổi/verify **1.533.763.076 bytes**, SHA `24433822f81ab63826f35b4f6b6202c3724b0b43473700125c54029339ffee63`; Q5 đã tạo **539.212.484 bytes**, SHA `e2cd4ceda28646fc74e48f171fad8ebd75e0ab06817d70349bc42ae2335bfc27`. Source PhoWhisper-medium revision `55a7e3eb6c906de891f8f06a107754427dd3be79`; input manifest khớp metadata đã pin. Download-only receipt chưa khẳng định F16/Q5 ready; conversion/quantization receipts sau đó mới ghi trạng thái này. Cả ba ghi `asr_tested=false`, không đọc dataset/gọi LLM API. Collector chỉ lấy 5 metadata/log files đã whitelist, kiểm tra archive/member hashes; **không đọc hoặc rehash weights**. Những asset receipts không đo ASR/quality/latency/full-app fit; ASR-only diagnostic mới có scope riêng ở trên. Metadata archive SHA `475556455301e7bf7c31e377b4355d118430d08a13b1c8fee5c1e82a32d25c6f`; receipts trong `.artifacts/v29-phowhisper-metadata/`.

**Kết quả cặp 30 v28 — đã hoàn tất và kiểm tra độc lập:** A dùng compact-typed; B thêm hai ví dụ synthetic cố định. Cả hai giữ cùng source v28, context 6.144, 4B Q4_K_M toàn GPU/output CPU, Whisper GPU beam1, SDK/deadline và 30 IDs đã freeze.

| Paired 30 | Điểm local | Mẫu hoàn hảo | Nonempty hoàn hảo | State matches | Failed |
|---|---:|---:|---:|---:|---:|
| A — baseline mới | 8,3333% | 2 | 0 | 5 | 0 |
| B — fixed few-shot | 11,6667% | 3 | 0 | 7 | 2 |

**Loại profile B này theo screening rule đã đăng ký:** mức tăng chỉ 3,3333 điểm phần trăm, thấp hơn gate 10; không thêm nonempty-perfect nào so với yêu cầu ít nhất 2; số lỗi tăng từ 0 lên 2. Cả hai lỗi B là generation deadline và vẫn nhận failed. Mọi mẫu có elapsed dưới 30 giây; trạng thái failed không được đổi chỉ vì nằm dưới deadline. Cả 2/3 mẫu hoàn hảo đều có gold actions rỗng; mẫu duy nhất tăng điểm cũng là zero-action. Có 1 mẫu tăng, 0 giảm, 29 giữ điểm; đây chưa chứng minh cải thiện task completion thực hoặc khả năng tổng quát.

Hai ZIP có 37 whitelist members/run; raw hashes và frozen IDs/thứ tự/pins/parameters đều khớp. Initial input messages khớp bytes 30/30; system B bằng A cộng đúng một suffix cố định. SDK replay độc lập 30/arm có 0 mismatch/rejection; replay riêng 52/41 observed results và final receipts đều khớp, 5 unexecuted refusals/arm giữ nguyên. Native logs có 75/67 release records, đều truncated0; B chỉ có 65 captured responses vì hai request bị deadline. Không thấy OOM/CUDA/context/transport hoặc model chết trong hai lượt. Prelaunch guards không được auditor hậu kiểm xác nhận độc lập; các gates chất lượng đã thất bại nên không có promotion.

Sampled peak PSS A/B là **1.179.256.832 / 1.180.797.952**, RSS **1.270.095.872 / 1.271.488.512**, GPU device **3.751.804.928 bytes** cả hai; swap0. Sampling mỗi giây chưa chứng nhận Windows cap 2 GB, tổng app, SSD15 GB hoặc A30. Đây là một cặp A-trước-B trên development đã được phân tích trước; metric local dùng fuzzy matching xấp xỉ, không phải điểm chính thức. Giữ nguyên gold/scorer/membership và mọi failed samples. Best historical audio vẫn 13,5556%; chưa đánh giá holdout hoặc nộp entry NORA.

Evidence riêng: `.artifacts/lifemate-fewshot-paired-audit-result-v1/paired-aggregate.json`; independent readback SHA `64c3d9da73ef3ecb6984025dc8fd0076cf868ccee02e6b655d00eba0e6a275db`. Archive A/B SHA `2030030b6baa7973dfa2c4b135f21aef60a4be9455ad016e041b792bbbc96cf8` / `fc31891e7b45f46799333a0550be737813c9650c1d73c41df864f32c563c03c1`. Collection receipts bất biến và replay/audit riêng nằm trong `.artifacts/lattice-runs/fewshot6144-{a,b}-paired030/`.

**V9 synthetic diagnostic:** năm phép so sánh có đủ dữ liệu đều khớp 24/24 gradient tensors theo bytes, numeric equality và exact loss, gồm cross-mode trên hai fixtures L347. Bốn child processes đã quan sát dùng cùng LoRA initialization/software fingerprint. Readback riêng kiểm tra hashes, finite values, layout và byte equality của 7 payloads đã lưu. Đây là scoped pass giữa aligned-reference và selected loss, chưa phải ordinary full-head equivalence.

Toàn matrix vẫn **incomplete/không accepted**: multi-target full repeat1 có gradient hữu hạn rồi dừng tại guard GPU free **527.630.336 bytes**, dưới ngưỡng 512 MiB không đổi. Có 7 accepted cases, chưa chạy EOS-boundary, không OOM, 0 optimizer steps hoặc adapter save. Peak CUDA allocated/reserved là 2.587.696.640/3.187.671.040 bytes; sampled RSS cao nhất 2.204.426.240 bytes. Saved checkpoints/Torch peaks chưa chứng minh continuous global peak hoặc cap desktop. Chưa có full-example training/update/save capacity, chất lượng tác vụ hoặc nguyên nhân riêng của V7 drift. Independent audit SHA `91f3caedf47a4288c100f6dab969ec2b22b13ede08d884d5b2c4521119554109`; local readback SHA `9f17d522059fa8cfa68b4de37c3fb9cb97dd285784e448f41be7a316fbd226f4`, trong `.artifacts/qlora-isolated-result-v9/`.

**Incident deployment v28:** arm A `fewshot-a-smoke003` dùng Qwen3-4B Instruct Q4_K_M, Whisper GPU beam1, full catalog, context **8.192**, toàn bộ lớp GPU và output CPU. Model nạp được rồi lỗi **CUDA out of memory** tại request đầu, trong `ggml_cuda_graph_launch_chunk`/`cudaGraphInstantiate`. Mẫu đầu lỗi transport, hai mẫu sau báo model process đã chết: **3/3 failed, 0 SDK calls**. Supervisor `exit_code=0`, `status=complete` chỉ ghi nhận đã xuất đủ 3 receipts; chưa chấm điểm hoặc replay. Theo screening rule đã đăng ký, dừng profile này trước B và hai lượt 30; mọi profile thay thế cần protocol riêng, không đổi riêng một arm.

Archive whitelist 10 files có SHA-256 `be2392f8fd80d25e1ff21c89ac9ee9c60138b9fc1f611415d57e9d7bcd6dcb94`; receipt collection độc lập SHA-256 `22c9484fc92c0e53b51b63de73be78669816ecd5fd6fc07467c552586be9997e`. Source v28 `b3741911604269ba5342baca85f2ed498da8ba31be546b1856fa083170d0fa61`, 124 source files, executable/native pins, thứ tự 3 development IDs đã freeze và resource peaks đều khớp. Sampled peak device GPU **3.902.799.872**, GPU owned **3.846.176.768**, PSS **1.089.678.336**, RSS **1.180.377.088 bytes**, swap 0. Lấy mẫu mỗi giây không ghi lại peak cấp phát tức thời đã OOM; chưa kết luận model 4B không thể chạy trên mọi profile 4 GB, chất lượng few-shot hoặc A30. Evidence riêng: `.artifacts/lattice-runs/fewshot-a-smoke003/independent-collection.json`.

**Amendment v2 — context 6.144 cho cả A/B:** protocol mới giữ nguyên source v28, assets, placement, SDK, deadline và frozen IDs; chỉ context chung đổi từ 8.192 xuống 6.144. Paired smoke đã được collect và replay độc lập:

| Smoke 6.144 | Complete | Failed | SDK calls | SDK replay mismatch/rejection | Sampled peak device GPU |
|---|---:|---:|---:|---:|---:|
| A — compact-typed | 3/3 | 0 | 7 | 0/0 | 3.749.707.776 bytes |
| B — thêm hai ví dụ synthetic cố định | 2/3 | 1 generation deadline | 4 | 0/0 | 3.749.707.776 bytes |

Mẫu B lỗi kết thúc ở 28,502 giây và vẫn giữ `failed`; supervisor exit0 không đổi trạng thái đó. Cả 3 IDs/thứ tự và source/executable/native/model/ASR/supervisor/parameters khớp, trừ mode/tag. Input messages đầu khớp bytes 3/3; system B bằng A cộng một suffix cố định. SDK replay chính thức nguyên trạng không mismatch/reject; kiểm tra riêng 7/4 observed results cũng khớp. Logs không có lỗi OOM/CUDA/context/transport hoặc model chết; 8/7 native releases hoàn tất đều `truncated=0`. Theo v2, timeout mẫu thông thường không tự thành lỗi triển khai hệ thống: **paired deployment screen đạt trong phạm vi quan sát; một mẫu vẫn thất bại và chưa có task accuracy**. PSS/RSS A là 1.148.390.400/1.239.199.744, B là 1.131.310.080/1.222.103.040 bytes; swap 0. Sampling mỗi giây chưa chứng minh hard cap.

Archive A/B SHA-256 lần lượt `de279b52d54679c1f3005ac69c32ff192d253082516a0f6db389d10a82095617` / `2548e02f6177dcd05232b2a1b482503cde33446144c9b4ee42a33553c9d2181e`; receipts riêng trong `.artifacts/lattice-runs/fewshot6144-{a,b}-smoke003/`. Hai smoke không dùng làm task score. Cặp 30 chạy mới sau smoke đã hoàn tất; kết quả và quyết định loại profile B ghi ở trên.

**Probe CPU 4B trước profile 6.144:** trên dynamic prefixes của 30 traces lịch sử v19, A/B có 75 positions/arm, tổng 150 rows. Pinned HF và GGUF formatter/tokenizer báo 0 render/token-ID mismatches; local readback xác nhận coverage/frozen IDs, render hashes, counts/fit và report pins. Max prompt A là 3.223, B là 3.484; B + 768 reserve = 4.252 < 6.144 cho những prefixes đã đo. Probe dùng dummy empty response schema, 0 generation/GPU/network; chưa kiểm chứng constrained decoder, trajectory mới, full-corpus 4B actor V2, numerical parity, training fit hoặc task accuracy. Lượt đầu dừng guard RSS 3 GiB tự đặt, không OOM. Lượt sau dùng guard RSS 4 GiB cho nghiên cứu, host available tối thiểu 4 GiB; sampled RSS peak là 3.434.545.152 bytes, host available thấp nhất 10.282.139.648 bytes. Controller báo server đã exit và key đã xóa; đây chưa chứng nhận desktop 2 GB. Report SHA `15c873fd1534810ae45140a623257fa8b75adb17ce49e3f4512b505c07acb391`; local readback SHA `0141b2726e0d9fd4907b0821e53595be4ec0b3e4b57fe39c4b7ac9cac90b9755`.

Đã kết nối SSH tới **lattice** qua cấu hình sẵn có: Arch Linux, Ryzen 5 4600H (12 logical CPUs), RAM 16.092.512.256 bytes, GTX 1650 4.096 MiB VRAM, compute capability 7.5, driver 615.71.09. Môi trường build riêng dùng CUDA 13.0.88, GCC 13.4 và Python 3.12. CUDA probe đã cấp phát, thực thi kernel và đọc lại kết quả 42 thành công. Native `llama-server`, `whisper-server` và NORA Rust release đã build xong; source, dataset và model được kiểm tra SHA-256.

Các lượt dưới đây dùng cùng 3 mẫu development đầu, ASR GPU và deadline 30 giây/mẫu:

| Profile | Cấu hình chính | Mẫu complete không lỗi | Kết quả |
|---|---|---:|---|
| v13 | Qwen3.5-4B, toàn GPU | 0/3 | Whisper hết VRAM trước khi bắt đầu mẫu |
| v14 | Qwen3.5-4B, 24 lớp GPU | 0/3 | Cả 3 mẫu hết thời gian xử lý |
| v15 | Qwen3.5-4B, output/embedding CPU | 0/3 | Nạp được cả hai model; prompt dài khiến cả 3 mẫu hết thời gian xử lý |
| v16 | Qwen3.5-4B, output CPU, prompt compact | 2/3 | Điểm local 0%; 1 mẫu lỗi |
| v16 dense | Qwen3-4B Instruct, output CPU, prompt compact | 3/3 | Điểm local 0%; trả danh sách hành động rỗng |
| v17 | Qwen3.5-4B, sửa thứ tự decoder và guard ID | 1/3 | Điểm local 0%; 2 mẫu lỗi |
| v17 dense | Qwen3-4B Instruct, cùng bản sửa | 3/3 | 20,18–25,20 giây; điểm local 0%, 0/3 state matches |
| v18 native | Qwen3-4B Instruct, native-compact | 0/3 | Hai prompt 6.165/6.461 tokens vượt context 6.144; một mẫu hết thời gian xử lý; điểm local 0% |
| v19 native | Rút gọn native schemas | 0/3 | Không thấy context overflow; cả 3 mẫu hết generation deadline; điểm local 0% |

Các profile Qwen3.5 dùng context 16.384 tokens, dense dùng 6.144 tokens; đây là so sánh cấu hình tổng thể, chưa tách riêng tác động của model. Supervisor dành 1,5 giây cuối cho receipt, nên lỗi hết thời gian xử lý thường được ghi ở khoảng 28,5 giây. Trạng thái tiến trình kết thúc bình thường và receipt replay hợp lệ không thay cho điểm tác vụ. SDK replay các profile đã chấm đều không mismatch.

Profile **v17 dense** có sampled peak RSS **1.417.113.600 bytes**, PSS **1.326.104.576 bytes**, GPU device **3.749.707.776 bytes**, GPU thuộc tiến trình NORA **3.693.084.672 bytes**. Số đo lấy mẫu mỗi giây; RSS cộng có thể đếm lại trang dùng chung, còn device GPU gồm desktop baseline. Các quan sát này chưa chứng nhận Windows private commit 2 GB, toàn bộ app, package SSD 15 GB hoặc A30. Desktop API vẫn có ngân sách riêng trong validation desktop.

Các lượt R0/R1 dùng source v19, snapshot `65cee9bfd1851cecbf48ee4b3f70586dba85ef300990660403d8f6937db46f3d`. Prompt compact dùng chữ ký công cụ ngắn và bớt dữ liệu lặp; schema gốc và SDK vẫn kiểm tra execution. Guard đã chặn việc coi tên người là ID, nhưng biết nguồn ID vẫn không chứng minh đúng ý định. Native/Rust checks gắn với từng snapshot đã chuẩn bị. Cache control dùng source v20; adapter xLAM và checks sau đó dùng source v21. Working tree local tiếp tục có thay đổi mới hơn; không coi các snapshot này hoặc toàn bộ local code là đồng nhất.

Hai lượt **compact-typed trên cùng 30 development** đã chấm xong, ASR Whisper GPU beam1, context 6.144 và source v19 ở trên:

| Lượt | LLM và tensor placement | Điểm local | Mẫu hoàn hảo | State matches | Mẫu lỗi |
|---|---|---:|---:|---:|---:|
| R0 | Qwen3-4B Instruct 2507 Q4_K_M; output/embedding CPU | 8,3333% | 2/30 | 5/30 | 0/30 |
| R1 | Qwen3.5-2B Q4_K_M; toàn GPU, CPU output off | 3,0000% | 0/30 | 8/30 | 15/30 |

R0 mất 5,40–27,96 giây/mẫu, median 17,29 giây. R1 có 7 HTTP400, 5 lỗi hết model-turn budget, 2 generation deadlines và 1 lỗi hết call budget. Cả hai file kết quả không có schema-invalid calls; SDK replay riêng đều không mismatch hoặc schema rejection. State matches không đồng nghĩa điểm tuyệt đối, nhất là khi mẫu lỗi phải nhận 0.

R0 có sampled peak PSS **3.106.097.152**, RSS **3.197.038.592**, device GPU **3.751.804.928 bytes**, không swap. R1 có PSS **10.495.360.000**, RSS **10.583.572.480**, device GPU **2.405.433.344**, swap **42.696.704 bytes**. Chưa chứng minh tài nguyên gây ra các lỗi R1. Loại **profile R1 đã đo**, chưa kết luận mọi model 2B đều kém. Hai lượt đổi cả model và tensor placement, nên chưa là ablation model thuần túy; đây cũng không phải kiểm chứng RAM desktop 2 GB, SSD 15 GB hoặc A30.

Audit router từ vựng không gọi model/SDK, chỉ so recall của các cặp `(sample, gold tool name)` sau khi router trả kết quả: **2.242/2.682 = 83,5943%** trên transcript chuẩn 1.235 development, **55/68 = 80,8824%** trên ASR thật 30 mẫu. Gold không được đưa vào router; holdout bị loại. Vì bỏ sót công cụ cần dùng, đã **loại hard filter và gỡ tích hợp mode routing khỏi app**; giữ module và audit để nghiên cứu. Recall này không phải điểm tác vụ hoặc benchmark NORA trên toàn bộ development.

Lượt cache control **v20** trên cùng 30 mẫu, cùng Qwen3.5-2B/ASR/native binaries/context/compact-typed có điểm **3,0000%**, 8 state matches, 0 mẫu hoàn hảo và 15 lỗi như R1. So sánh JSONL xác nhận **30/30 records giống nhau và toàn bộ bytes giống nhau**, SHA-256 `9b15ebbd816997cf8be0eead1cd2bcb395eed6c731c71ec6bbc9d816427e1dd7`. Đây là quan sát của hai lượt đã đo, chưa chứng minh determinism cho mọi lượt.

| Sampled peak | R1 v19 | Cache control v20 |
|---|---:|---:|
| PSS | 10.495.360.000 bytes | 1.457.882.112 bytes |
| RSS | 10.583.572.480 bytes | 1.548.320.768 bytes |
| Swap | 42.696.704 bytes | 0 bytes |
| Device GPU | 2.405.433.344 bytes | 2.405.433.344 bytes |

Hai knobs cùng đổi: cache RAM mặc định **8 GiB → 0**, context checkpoints **32 → 4**; chưa tách tác động riêng từng knob. Source v20 cũng có prototype chưa dùng trong đường 2B đã đo. v20 ghi `llama-server` là contributor RAM lớn nhất tại sampled tree peak; v19 không có per-process samples để xác định owner của peak cũ. Việc giảm RAM không cải thiện điểm của hai lượt này và chưa chứng nhận hard cap desktop 2 GB, toàn app hoặc A30.

Source **v21**, SHA-256 `3f50338d52a84931d8704bfa709ab64c9ee42f42eaf417dd3f331ad3c709766d`, qua **55 Linux tests**, fmt và release Clippy all-targets; gồm **12 tests executor kế hoạch, 6 tests parser xLAM**. `plan.rs` vẫn chưa tích hợp với LLM hoặc kiểm thử end-to-end NORA. Adapter xLAM bỏ chat parsing theo format đã chọn và qua chu trình **synthetic call → result → done**. Probe audio thật 3 mẫu đều lỗi: **2 context overflows, 1 deadline**; chưa chạy đủ 30, chưa có điểm task accuracy mới từ xLAM. Receipt synthetic format đã được lưu tại `.artifacts/lattice-v21-xlam-format-probe/result.json` và ghi rõ không dùng dataset/SDK. Kết quả audio 3 mẫu ở đây vẫn là quan sát từ remote logs; receipt audio đầy đủ còn chờ tải về.

Semantic router **v22** đã hoàn tất recall audit trên ASR thật của 30 development: giữ **52/68 = 76,4706%** tool cần dùng; chỉ **18/27** mẫu có gold calls giữ đủ toàn bộ tool, fallback toàn catalog 5/30, trung bình chọn 15,40/39,83 tools. Median router **3.301 ms**, max **4.601 ms**. Source SHA-256 `b38efadf23c1520b299c89d11f2fad6928ff4635811d189a53178fd92eca746a`; report/receipt đã tải và kiểm chứng. Không đưa gold vào router, không gọi SDK hoặc hosted API. **Loại semantic hard filter**, như router từ vựng trước đó; không đổi recall thành task accuracy hay coverage inference 1.235 mẫu.

Probe **PlanTyped v23** đã hoàn tất trên Qwen3.5-2B và Qwen3-4B Instruct GPU. Với mỗi model, LLM sinh **một plan** khi chưa biết kết quả weather, rồi Rust thực thi lại chính plan đó với callback synthetic `rain=false` và `rain=true`: nhánh false chỉ đọc, nhánh true mới tạo effect; cả hai model pass cả hai nhánh. Hai model không được yêu cầu sinh plan byte-identical.

| Probe synthetic | Elapsed toàn lượt, gồm khởi tạo model | Kết quả |
|---|---:|---|
| Qwen3.5-2B Q4_K_M | 5,52935 giây | Một plan pass cả hai nhánh |
| Qwen3-4B Instruct 2507 Q4_K_M | 8,18259 giây | Một plan pass cả hai nhánh |

Probe **không dùng ASR, dataset, SDK hoặc hosted API**, không đo task accuracy hay toàn pipeline NORA. Source v23 SHA-256 `481cf947d3ff5996a7fa8168d6e89b5063216af0489092bfde0d3851a4f941df` qua **62 Linux tests** (51 lib, 1 preflight, 8 workflow, 2 semantic-example), fmt, release Clippy và release build. Export ZIP SHA-256 `d3d83b976848cf7151dd0bf8bdad672e7a9870e9bf01537b14ff9621c0fa8c46`; receipts đã được đối chiếu với file giải nén.

Runner opt-in **`plan-typed-experimental` đã được đo audio thật ở v25** trên 3 development samples, Qwen3.5-2B Q4_K_M, Whisper GPU beam1 và context 12.288. Kết quả **0%**, 0 mẫu hoàn hảo, 1 state match, **3/3 failed**: hai mẫu `local model did not finish its decision`; mẫu còn lại hết generation deadline sau 7 provenance refusals cho ID chưa được quan sát. Không suy ra nguyên nhân duy nhất từ các logs này. Lượt 30 mẫu **chưa được mở**.

Elapsed 20,55–28,50 giây/mẫu. Sampled peak RSS **1.499.197.440**, PSS **1.408.026.624**, device GPU **2.457.862.144 bytes**, swap 0. SDK replay riêng 0 mismatch/0 schema rejection. Supervisor exit0 chỉ cho biết batch kết thúc; không biến các mẫu failed thành thành công hoặc chứng nhận RAM hard cap 2 GB. Synthetic v23 vẫn pass, chưa được nâng thành bằng chứng chất lượng tác vụ.

Source v25 SHA-256 `01d115c667c52c6f9a4cab129109cb6b08d060c82c175914e78c83e143d08c7f`; archive SHA-256 `ead0f9417cd3c9fb5fca0c707b63b3ed7677f2b88de7de8c68ceb7024606aaed`. Receipts/report/replay đã tải và đối chiếu. Cặp **v26 xLAM progressive/full-catalog** đã chạy trên cùng 3 development samples, source `e75d4bfd0a3c04a073597519ab16db93c939faea68c289231f2a99aa575b8c7f`, xLAM-2-3B Q4_K_M, context 8.192, CPU output on, native pins và ASR beam1 giống nhau:

| Profile | Local score / failed / state matches | SDK calls mỗi mẫu | Sampled PSS / device GPU |
|---|---|---|---:|
| Progressive schema-on-demand | 0% / 3/3 / 1/3 | 1, 1, 1 | 1.065.694.208 / 2.912.944.128 bytes |
| Full catalog | 0% / 3/3 / 1/3 | 0, 0, 0 | 1.056.154.624 / 2.910.846.976 bytes |

Progressive gặp invalid/unavailable calls, loop hết turn budget và empty completion không sửa được action chưa hoàn tất. Full catalog cả 3 hết generation deadline trước SDK call. Hai replay đều 0 mismatch/0 schema rejection, swap 0; **chưa mở lượt 30 và chưa có quality gain**. Smoke chỉ có 3 mẫu/profile; ASR mẫu 2/3 của full catalog chậm hơn progressive, nên chưa quy toàn bộ chênh lệch thời gian cho cách đưa schema.

V26 qua 85 Linux tests (64 lib, 1 CLI, 18 workflow, 2 semantic-example); core-only Windows qua 79 tests, Clippy và fmt. Phạm vi checks gắn với source v26, không thay accuracy. Default desktop không đổi. Pure HTTP loopback test đã chứng minh **socket cleanup không tiến triển ngoài `block_on` trong fixture `current_thread`**: bản test hợp lệ fail ở 1,13 giây, đổi sang shared runtime `new_multi_thread` với một worker thì pass ở 0,11 giây. Test đầu tiên bị reactor panic là setup không hợp lệ, không dùng làm bằng chứng cơ chế. Ba profile actual audio **v27** sau runtime fix đã chấm, source `64d12e50ce265140505dce4581a6054f77209e79dbe8c81552a9b061a13f7441`, context 8.192:

| Profile v27 | Score / failed / state matches | SDK calls mỗi mẫu | Sampled PSS / device GPU |
|---|---|---|---:|
| xLAM full catalog | 0% / 3/3 / 1/3 | 0, 0, 0 | 1.044.081.664 / 2.910.846.976 bytes |
| Qwen3.5-2B native progressive greedy | 0% / 2/3 / 1/3 | 2, 7, 10 | 1.377.778.688 / 2.422.210.560 bytes |
| Qwen3.5-2B native progressive custom sampling | 0% / 1/3 / 1/3 | 1, 2, 3 | 1.374.185.472 / 2.422.210.560 bytes |

Cả ba replay 0 mismatch/0 schema rejection, swap 0, chưa mở 30. Qwen sampling đã đo dùng `temperature=0.7, top_p=0.8, presence_penalty=1.5`; [model card đã pin](https://huggingface.co/Qwen/Qwen3.5-2B/blob/15852e8c16360a2fea060d615a32b45270f8a8fc/README.md) ghi text non-thinking `1.0/1.0/2.0`, nên đây là custom sampling, chưa phải đối chứng theo đúng text preset của nhà phát hành. Greedy dùng temperature0. Số failed giảm trong 3 mẫu nhưng điểm vẫn 0%, chưa là quality gain hoặc ước lượng tổng quát.

Matched xLAM full-catalog ASR v26 → v27 lần lượt **2.570,71/5.681,06/5.587,79 → 2.574,02/3.401,73/3.754,18 ms**. Đây là quan sát 3 mẫu sau đổi background runtime, chưa cô lập causal impact, và cả hai profile vẫn 3 generation deadlines/0% task score. Ba cặp `messages` JSON trong traces v26/v27 xLAM full catalog khớp nhau. Với task đầu, server-relative launch + `model_ms` cho mốc client-return xấp xỉ **32,390143 s (v26)** / **32,751803 s (v27)**, còn cancel log là **38,530698 s** / **32,898310 s**: khoảng chênh xấp xỉ **6,141 s** / **0,147 s**. Công thức này có sai số vì `model_ms` bọc cả invocation ngoài server launch; các clocks riêng theo từng server. Đây là evidence timing matched cohort, chưa chứng minh GPU contention hoặc causal ASR/quality gain. V27 Linux checks 86 tests (65 lib, 1 CLI, 18 workflow, 2 example); Windows core 80 (62 lib + 18 workflow). Pure socket-cleanup proof giữ nguyên scope fixture; chưa khẳng định toàn bộ deadline/accuracy đã sửa. [Kế hoạch và bằng chứng kiến trúc](research/nora-architecture-experiments.md).

Audit độc lập toàn 30 mẫu **R0 8,3333%** đối chiếu gold/actual traces/receipts xác nhận: hai mẫu perfect đều có gold actions rỗng do lệnh phủ định/rút lại; không có mẫu gold không rỗng perfect. Có 5 state matches nhưng 2 mẫu trong đó tool F1 bằng 0; hash có thể giữ nguyên khi chọn sai route/ý định. Một tác vụ khác hoàn tất effect đúng nhưng chỉ được điểm một phần vì lookup query khác chữ hoa/thường ở tham số exact-match; không sửa gold/scorer để xóa khác biệt này.

Phân loại post-hoc ghi primary gap ASR semantic loss 9/30, lookup/context/entity 7/30 và condition/exception/negation 4/30; các tags chồng lấp rộng hơn. Đây là phân tích traces/reference text, chưa chứng minh root cause duy nhất, WER hay lợi ích của một counterfactual model. Không thay dataset, scorer, membership hoặc loại mẫu khỏi evaluation. Audit riêng tư `.artifacts/lattice-r0-error-taxonomy.md/json`; public docs chỉ giữ aggregate.

Tokenizer pilot CPU trên dataset V1 đã hoàn tất exact tokenization của **5.096 records/model**, gồm 1.082 semantic train, 153 semantic validation, 3.377 actor train và 484 actor validation. Với template/tokenizer đã pin, mọi full sequence không truncation và nằm trong 8.192 tokens; actor full max **7.728 (2B)**, **7.940 (4B)**. Receipt không có weights/model-class load, model inference, GPU hoặc training; network attempts 0. Đây **không chứng minh training vừa VRAM/RAM** hoặc có task accuracy. Capacity QLoRA **v1 đã launch nhưng dừng preflight**: guard nhận nhầm desktop compositor là model cạnh tranh, chưa load quantized model, **0 optimizer steps**, không có parity hoặc capacity measurement. Đây là guard-plumbing failure, chưa phải bằng chứng 2B không vừa GPU hay cần giảm model. Không kill process hoặc đổi profile/retry trong v1. V2 sau sửa guard đã **load 617 tensors không thuộc MTP**, giữ embedding FP16 tied và inject 12 LoRA modules q/v; sau load còn **1.745,2 MiB GPU free**. Synthetic masked backward dừng tại điều kiện gradient thiếu/không hữu hạn: guard gộp hai trường hợp, chưa ghi parameter hoặc xác định missing/NaN/Inf. Tiếp đó report serialization lỗi `set` trong loading metadata; raw logs được giữ và failure evidence được dựng lại, không giả nhận có report hoàn chỉnh.

V2 terminal, **0 optimizer steps**, chưa có training capacity, parity hoặc task quality; không có bằng chứng OOM để quy lỗi cho model size. Compositor resource fallback gắn PID/starttime/UID/comm/driver-name và mức ≤16 MiB; `exe` chưa xác minh vì EACCES, GPU memory vẫn tính vào baseline, không kill process. **V3 diagnostic GPU đã chạy và terminal failed**, không phải chỉ CPU prep nữa. Synthetic full-masked loss **2,6261706352** finite với scale **128**; 24 gradient tensors đều present, nhưng bốn tensor LoRA A/B của q/v tại layer3 có tổng **34.816 NaNs**, 0 Inf. Hai mươi tensors ở các layers sau đều finite (có tensors zero). V3 đã ghi report hoàn chỉnh, **0 optimizer steps**, không OOM; recorded CUDA reserved max **2.105.540.608 bytes**, GPU free min **1.662.189.568 bytes**. Đây là diagnostic synthetic34tokens, chưa full-training fit, parity pass hoặc task quality; chưa tách scale với FP16/torch fallback để xác định nguyên nhân.

Source script V3 SHA-256 `886b6c7aff3c9c0fbee0c939665361d631b63b39ba3bdc0139069c05dbf335e6`, receipts tại `.artifacts/qlora-capacity-result-v3/`. Capacity profile này dùng V1 full-schema, không phải dataset runtime V2. **V4 synthetic-only đã hoàn tất 6backward cases**, không optimizer hoặc adapter change. Full-masked/selective ở scale 128 đều có 4 NaN gradient tensors layer3; ở scales 8/1, cả 24 tensors present và finite, loss 2,6261706352 không đổi. Tuy nhiên **strict per-tensor parity fail ở cả 8/1**, global relativeL2 lần lượt 0,002839730/0,002673819; chưa có parity success, training fit hoặc quality.

Boundary backward output không hữu hạn đầu tiên được ghi ở `layer4.block`, **chưa phải exact root operation**. V4 giữ 0 optimizer steps; **V5 matched control đã hoàn tất 8 backwards, 0 optimizer steps**, adapters unchanged. Full/full và selective/selective repeats bitwise-identical trong mỗi head precision, nhưng cross-mode strict gradients vẫn fail:

| V5 cross-mode | Mismatched elements | Nonzero sign flips | Max absolute gradient delta | Min cosine |
|---|---:|---:|---:|---:|
| Native FP16 head | 1.001 | 87 | 0,0015926361 | 0,9999940026 |
| CPU FP32 head | 755 | 60 | 0,0007858276 | 0,9999957620 |

Vì CPU FP32 head vẫn không pass strict comparison, **predeclared head-only criterion chưa đạt**; chưa xác định root arithmetic operation. CPU-head chỉ là research reference, body/backward vẫn FP16/NF4. Process RSS peak 4.392.218.624 bytes, CUDA reserved 2.365.587.456, GPU free min 1.400.045.568, swap delta 0; đây chưa phải full-training fit hoặc desktop budget compliance.

Audit CPU độc lập đã kiểm tra hashes/tính lại 8 derivative arrays. Analytic global-norm1 clipping + zero-moment first Adam step vẫn cho cross-mode relativeL2 **0,05380475 (native)** / **0,04446094 (FP32 head)**, max delta gần 2×lr. Đây chỉ analytic comparison, **không optimizer update**, không training quality. **V6 real-model synthetic L347 đã chạy ordinary-full backward**: all **86.167.040 FP16 logits** finite, loss **2,858117342**, 24 gradients finite. Sau backward, GPU free **473.104.384 bytes (~451,19 MiB)** thấp hơn self-imposed **512 MiB headroom guard**, nên dừng; CUDA allocated peak **3.005.579.776**, reserved **3.292.528.640 bytes**. **Không OOM, 0 completed diagnostic cases, chưa chạy aligned modes và 0 optimizer steps**. Guard không bị hạ, chưa chứng minh parity/training fit hoặc model4GB không thể chạy. **V7 selected-first đã chạy**, completed 6/12 cases. Case thứ 7 (multi-target aligned-full) loss/24 gradients finite rồi guard 512 MiB dừng ở GPU free **531.824.640 bytes (~507,19 MiB)**, allocated peak **2.538.638.848**, reserved **3.233.808.384 bytes**. Không OOM, optimizer hoặc adapter save; full test matrix chưa đủ.

L347 short-target completed cả selected/full repeats: strict mismatch/sign-flip counts **571/45 (selected repeat)**, **190/15 (full repeat)**, **443/34 (cross-mode)**. Multi-target selected repeats strict pass nhưng không bitwise-identical; full counterpart chưa complete. **Không overall repeatability/parity acceptance hoặc tile-only cause**. **V8 selected-only deterministic control đã được audit độc lập**:6backwards trên3known synthetic fixtures, mỗi fixture 2 repeats; cả 3 pairs có 24/24 gradient tensors byte-identical và exact loss. Saved resource guards pass, GPU free min **628.293.632 bytes**, 0 optimizer. Scope chỉ selected-only same-mode repeatability với pinned model/device/software vàfixtures này; determinism settings đổi cùng lúc nên chưa cô lập nguồn V7 drift. **Chưa aligned-full cross-mode parity, full-example capacity hoặc task accuracy.** Independent audit SHA-256 `1d5de34ed352807a5a1abb1bf558a3a4b9f4001312673cf5ea0d67aa9a76ad4a`.

GGUF↔HF template/token probe **attempt1 CPU-only** dừng trước comparison vì script giả định sai substring/build number trong `build_info`; native/model/source pins vẫn khớp. Processed examples0, owned server/key cleanup đã xác nhận; không generation/inference/GPU/training. Corrected **probe2 đã complete và được kiểm toán độc lập** từ receipts local: **3.861 unique actor records (3.377 train/484 validation)**, cả 8 prefix/full byte/token counters đều match HF, 0 discrepancies/truncation. Scope là pinned **Qwen3.5-2B GGUF** và HF tokenizer cùng model; chưa xác nhận 4B hoặc model inference/training/SDK/task accuracy. Controller/server đã exit và ephemeral auth dir/key được xác nhận không còn. Agent-turn error trước đó không phải process result; terminal status ở đây dựa trên actual receipts và independent collection.

Report SHA-256 `bd627bdccca31eddec61e86b17984ab875b0748bcfea362afda561351ea8f968`; independent audit `42ba3ab907c917e4af39162f8d150b9ddba1c02c5f0d416da188bdfba2caac83`. Không generation requests hoặc source/model change; các activity objects do owner quản lý vẫn được docs update giữ nguyên. Active GPU và CPU-activity fields do owner quản lý, docs update giữ nguyên. Active job do owner quản lý và không bị update tài liệu thay đổi. [Evidence và roadmap](research/nora-architecture-experiments.md).

Audit riêng gold **train_1696** dùng một `ToolEnv` với 3 lượt validate/call tuần tự; 53 file SDK Python khớp archive chính thức. Cả 3 validate pass, `ok=true`, nhưng call cuối có **`done=false`**; result khớp label, ordered calls và state hash khớp oracle lịch sử. Script oracle cũ chỉ đếm `not result.get("ok")`, nên “0 call failures” không đếm accepted no-op. Đây chưa phải SDK/label inconsistency hoặc bằng chứng task completion. Không đọc environment trực tiếp, không gọi `get_state`, không loại mẫu khỏi evaluation hoặc sửa scorer/dữ liệu.

Audit mới **toàn development** xác nhận **1.235 samples, 2.811 SDK calls** theo thứ tự `validate → call`: 0 validation rejection, 0 actual/label-result mismatch, 1 `done=false`, anomaly file 0 bytes. Mỗi sample dùng một ToolEnv; không đọc environment trực tiếp, không gọi state/hash APIs. Script decode dòng train để lấy ID rồi bỏ holdout trước khi truy cập label hoặc tạo ToolEnv; không dùng label holdout, không khẳng định bytes của chúng chưa được decode. Đây chỉ xác minh **training-source observation parity**, không phải model accuracy hoặc mọi goal/effect đều hoàn thành. Receipt summary SHA-256 `ce965897983e6e617fc5204aa640af45daa12c5463fd16478ac78a63e1e225b4`; script thực SHA-256 `8115a9ebf056f4d65cd92e1224878f684181a08464923a27cdf3ad39bfcc5e7f`.

Actor dataset **runtime v2** đã được chuẩn bị/validation riêng theo contract `compact-typed/full-catalog` Qwen3-4B Instruct: **3.377 training records từ 1.082 IDs**, **484 validation records từ 152 actor IDs**. Tất cả **153 validation evaluation IDs** được giữ; `train_1696` chỉ quarantine actor trajectory vì gold `done=false`, không sửa label/scorer hoặc loại khỏi evaluation. Outer holdout310 không vào model examples, 30 baseline IDs validation-only.

Có **3.861 future-poisoning/input-readback checks** và 6 structural tests pass; **115 recorded messages, gồm 55 observations, trong 30 traces** khớp bytes khi dùng cùng supplied actual ASR. Đây chưa phải Rust export parity toàn corpus. Actual GGUF–HF rendered-prefix/token parity về sau đã pass cho 2B theo receipt riêng ở trên; 4B trên toàn bộ actor V2 chưa được probe; phép kiểm tra 4B historical-prefix 75/arm có phạm vi riêng ở trên. Exact **HF tokenizer V2 đã hoàn tất 3.861 records/model** trên hai pinned tokenizers, có input/target hash readback, prefix/token identity, target/EOS/order/roundtrip và completion-span metadata checks; không truncation hoặc loại thêm record. Không model/training/inference/GPU/network.

| HF tokenizer V2 | Prompt median / p95 / max | Full sequence max | Fit prompt +768 reserve ở 6.144 |
|---|---|---:|---:|
| Qwen3.5-2B | 2.389 /3.750 /5.166 | 5.177 | 3.861/3.861 |
| Qwen3-4B Instruct | 2.443 /3.848 /5.334 | 5.345 | 3.861/3.861 |

4B còn 42 tokens sau reserve 768 ở ceiling 6.144; đây là HF template-length evidence;2B actual GGUF prefix đã verified riêng,4B overhead và khả năng runtime/training vẫn cần bằng chứng. V1 tokenizer5096 được giữ như lịch sử, không dùng thay V2. Report SHA-256 `2badee05b950e0c53d8ad16f64095941c247e6f5cb33982f4518afda6557b998`. Chưa train/inference/SDK execution khi tạo dataset. Manifest SHA-256 `9a87a6312389a155d8458c43f7f5d4dbcd215f782493b64dbfb790e9860f2db0`; validation SHA-256 `6b407fc708df38b7762c7ae85357c9d9a8c03e41a679ec4add329950e9d2e73f`.



So sánh ASR riêng trên cùng 30 development samples/32 utterances:

| Beam | Character-edit local | Word-edit local | Median sample ASR |
|---|---:|---:|---:|
| 1 | 24,36898% | 38,78327% | 2.550,12 ms |
| 5 | 29,27949% | 43,34601% | 2.653,76 ms |

Beam5 tốt hơn 6, kém hơn 8 và bằng 18 utterances theo character edits. **Giữ default beam1** vì tổng lỗi và median latency đều tăng với beam5 trong phép đo này. Đây là diagnostic post-hoc: transcript chuẩn chỉ được nạp khi chấm sau inference; chuẩn hóa NFC, lowercase, thay dấu câu bằng khoảng trắng, không mở rộng cách viết số. Các chỉ số này không phải official WER hoặc điểm tác vụ LifeMate; chưa suy ra chất lượng toàn bộ tập dữ liệu.

Các kết quả trước trên cùng 30 development được giữ để đối chiếu:

| Lượt chạy | Điểm local | Phạm vi |
|---|---:|---|
| PhoWhisper + legacy | 9,6667% | Audio thật |
| PhoWhisper + native greedy | 13,2222% | Audio thật |
| Transcript chuẩn + legacy | 15,5556% | Diagnostic planner, không tính là voice accuracy |
| Transcript chuẩn + native greedy | 21,8283% | Diagnostic planner, không tính là voice accuracy |

Qwen3-ASR-0.6B-hf đạt CER local 21,84488%, median 1.076,99 ms trên 32 utterances, chưa tốt hơn PhoWhisper theo CER. Diagnostic Qwen3-4B Instruct Q8 bị ngắt ở 8/30 mẫu khi VM Colab mất; chưa có điểm để kết luận. Colab báo hết quota GPU miễn phí; công việc hiện chạy trên lattice. Chưa commit/push hoặc nộp entry NORA.

## Hai cấu hình riêng

| Chế độ | LLM | Ngân sách |
|---|---|---|
| Desktop Windows | API `gpt-6-luna`, reasoning `xhigh`; voice local | Giữ cap NORA 2.000.000.000 bytes RAM commit, GPU 4 GB, SSD 15 GB |
| LifeMate offline | Qwen3.5-4B Q4_K_M local + Whisper large-v3-turbo Q5_0 | Được người dùng cho phép dùng cấu hình thi: A30, 16 CPU cores, 64 GB RAM |

Không tăng cap desktop để chạy bản thi. `BudgetJob::install()` vẫn từ chối giá trị vượt 2 GB; `install_contest()` là constructor riêng. Cap 64 GB không tạo thêm RAM cho laptop hiện tại. Các phép đo desktop trước đây không chứng nhận tài nguyên hoặc chất lượng của bản offline.

## Yêu cầu quyết định cách triển khai

| Nội dung | Hợp đồng từ portal/devkit |
|---|---|
| Đầu vào | WAV tiếng Việt, context thời gian/timezone/location/profile, app tools có schema và history |
| History | Effects đã có sẵn trong môi trường; không replay hành động cũ |
| App state | Chỉ đọc/thay đổi qua `ToolEnv` được BTC cung cấp; không đọc, sao chép hoặc parse `<split>_environment/` |
| Session | Tạo đúng một `ToolEnv(split, sample_id, root=...)` cho mỗi mẫu; không reset để thử lại |
| Tool calls | Ghi mọi `env.call()` theo thứ tự thực thi, kể cả lời gọi bị từ chối khi thực thi |
| Schema | Có thể dùng `env.validate()` trước; validate không phải một tool call đã thực thi |
| Thiếu thông tin | `sys.ask_missing_info` với `tool_name` và danh sách `params` còn thiếu |
| Ngoài phạm vi app | `tool_calls: []` |
| Đầu ra | UTF-8 JSONL: `id`, `tool_calls` gồm `name`/`arguments`, `state_hash` từ `env.get_state_hash()` |
| Inference | Offline, không gọi API/dịch vụ ngoài |
| Model | Tổng hệ thống dưới 6B parameters, LLM dưới 5B; context cấu hình tối đa 16.384 tokens |
| Thời gian | 30 giây/mẫu; mẫu lỗi hoặc quá giờ nhận 0 điểm |
| Phần cứng chấm | Một NVIDIA A30, Xeon Gold 6442Y 16 cores, RAM 64 GB |
| Reproducibility | Decoding/seed cố định; nếu không tái lập, BTC có thể chạy Docker 5 lần rồi lấy trung bình |
| Dữ liệu/model ngoài | Công khai trước 26/10/2026 và đáp ứng điều kiện giấy phép của track |
| Test data | Không dùng người hoặc model ngoài gán nhãn public/private test để train/tune |

Nguồn: các tab Description, Evaluation, Rule và Submission Guidance của [LifeMate](https://challenge.zalo.ai/portal/lifemate); interface được đối chiếu trực tiếp với SDK v3.2.0 đã tải. Không gửi audio, context hoặc app data của cuộc thi tới LLM API. Quy định cho phép tạo synthetic training data trong giai đoạn phát triển không cho phép gọi dịch vụ ngoài lúc chấm.

## Điểm và giới hạn của mục tiêu formal 100% / checkpoint 99%

Với mẫu có gold calls, công thức công bố kết hợp final-state equality với tool-call F1: state sai hoặc không có call khớp nhận 0; khi state đúng, điểm là `2M / (N_pred + N_gold)` với matching một-một. Tham số không fuzzy cần khớp chính xác; chuỗi có `x-match: fuzzy` dùng token F1 lớn hơn 0,6 sau chuẩn hóa NFC và chữ thường. Với gold calls rỗng, chỉ dự đoán rỗng được điểm. Lỗi/timeout luôn nhận 0. [Evaluation](https://challenge.zalo.ai/portal/lifemate)

`scripts/evaluate_lifemate_train.py` chỉ là **bản tính điểm local cho train**, không phải scorer của BTC. Portal chưa mô tả đầy đủ cách tách dấu câu cho fuzzy matching; script công khai lựa chọn tách theo khoảng trắng. Báo cáo phân biệt coverage toàn bộ partition với điểm của một tập con; `--require-complete-perfect` từ chối run thiếu mẫu, lỗi, sai state hoặc dưới điểm tuyệt đối. Receipt cần được replay kiểm chứng riêng qua SDK trước khi tin hash.

Public test không có gold labels trong gói tải về. Vì vậy không thể tự xác nhận 100% public bằng scorer train. Cần điểm từ BTC; điểm private cũng chưa thể chứng minh trước khi có kết quả chấm. Không đổi tên kết quả oracle hoặc unit tests thành độ chính xác NORA.

Snapshot leaderboard đọc lại chiều 02/11: **TrustedAI dẫn đầu cột SLM với 96,51**, Akatsuki 95, phoaln 91,67. Dòng TrustedAI có APIs/Others 96,66 để tham khảo. Portal ghi chỉ cột SLM dùng xếp hạng, các cột khác để tham khảo. Mục tiêu formal của NORA là 100%, checkpoint trung gian 99%; leaderboard có thể đổi sau thời điểm đọc. [Public Leaderboard](https://challenge.zalo.ai/portal/lifemate)

## Dữ liệu đã tải và kiểm tra

| Artifact | Quan sát |
|---|---:|
| Devkit `VERSION` | v3.2.0, build 24/10/2026 |
| Archive bytes | 876.430.231 |
| SHA-256 archive | `57d371518996ec70d3af86402b7eba61208093c82955b9807a7ab39604b47320` |
| Train / public test | 1.545 / 634 mẫu |
| Train catalog | 46 apps, 147 tools |
| Public catalog | 36 apps, 122 tools |
| Audio header audit | 6.969 WAV, tất cả PCM16 mono 16 kHz; dài nhất 22,3335 giây |
| Train development / holdout | 1.235 / 310 mẫu, chia bằng hash ID trước khi tuning |

Archive được tải từ Data tab sau khi người dùng đăng nhập. Tài nguyên nằm trong `.artifacts/` được Git bỏ qua. Chỉ kiểm tra header audio ở bước audit; transcript/labels không được đưa vào model inference. Holdout dùng `SHA256("NORA-LIFEMATE-HOLDOUT-v1:" + id) mod 5 == 0`; train runner mặc định chỉ chọn development. Không dùng holdout để tạo retrieval examples hoặc tune prompt.

## Luồng hiện có

```text
Khởi động native model local và warmup ASR bằng silence synthetic
  → đọc input metadata và whitelist tools đúng installed_apps
  → bắt đầu clock của mẫu, tạo một ToolEnv
  → ASR các WAV history và yêu cầu mới nhất
  → Qwen sinh JSON quyết định có giới hạn
  → SDK validate → env.call → quan sát kết quả
  → Qwen quyết định bước phụ thuộc kế tiếp hoặc kết thúc
  → đối chiếu ledger với SDK → lấy official state_hash
  → ghi submission.jsonl và metrics riêng
```

Phần điều phối là Rust trong `nora-lifemate`. Python chỉ là bridge nhỏ để dùng **SDK BTC nguyên trạng**, không có planner hoặc tool đọc state. Input types không deserialize `label`/`audio_text`; inline app state cũng bị loại. Native model chỉ nhận kết nối loopback do tiến trình này sở hữu, không có cấu hình URL cloud. API key desktop không được đọc hoặc truyền cho subprocess.

Planner bị giới hạn 12 lượt, 32 calls/mẫu, context tối đa 16.384, temperature 0 và seed 42. Planner được chỉ thị lấy ID từ context/history/tool results trước khi thực hiện bước phụ thuộc. `finished=true` chỉ kết thúc sớm khi các outcomes thành công và không có `done=false`. Sau `sys.ask_missing_info`, dừng các effects tiếp theo.

Mẫu lỗi ASR/LLM vẫn giữ effects đã thực hiện và receipt của SDK; không bỏ mẫu khỏi batch hoặc tự chế hash. Runner dành thời gian cuối mẫu cho receipt và ghi rõ `failed`/`timeout` trong metrics. Nếu chính bridge hỏng, không thể tạo receipt hợp lệ: batch dừng thay vì xuất hash giả.

GPU ASR dùng native whisper-server riêng; CPU worker giữ làm fallback để so sánh. Inference GPU thật đã chạy trên lattice; lượt Windows trước đó chưa chạy được vì host thiếu RAM commit. Loopback bind và secret URL prefix của ASR giảm khả năng truy cập trái phép; chúng không thay cho OS sandbox. Docker cần chạy `--network none`.

## Kiểm chứng Windows lịch sử và phạm vi hiện tại

| Phép kiểm chứng | Kết quả | Chứng minh điều gì |
|---|---|---|
| Oracle replay gold train qua SDK | 1.545 mẫu, 0 lỗi calls, 0 state mismatch | SDK/data sanity; không phải NORA inference |
| NORA bridge + SDK với oracle calls | 1.545 sessions, 3.541 calls, 0 result/receipt mismatch | Bridge bảo toàn SDK semantics |
| Rust tests LifeMate/platform | 25 passed, gồm 6 workflow tests, cleanup probes và CLI preflight | Controller, boundaries, receipts, cleanup và cap desktop |
| Python tests LifeMate | 16 passed | Metric/coverage mirror, bridge, input layout, asset staging |
| Clippy LifeMate/platform | Passed, all targets/features, `-D warnings` | Static checks của phần thay đổi |
| NORA ASR → LLM → tools thật | Đã chấm 30 development qua nhiều profile trên Colab/lattice | Kết quả hiện tại ở đầu tài liệu; chưa có điểm chính thức |
| A30/offline Docker | Chưa build/chạy | Chưa chứng nhận môi trường BTC |

Hai lượt nạp LLM dưới cap desktop 2 GB thất bại; đây là lý do tách profile thi theo quyết định của người dùng. Lượt CUDA với profile 64 GB đã nạp LLM nhưng CPU ASR của mẫu đầu vượt deadline. Lượt đó dùng khoảng 5,93 GB private commit đỉnh và 3,08 GB GPU dedicated được sample trên RTX 5050 8 GB; không phải phép chứng nhận A30 hoặc GPU vật lý 4 GB. Không có prediction hoàn tất để tính điểm NORA. Log thất bại được giữ, không loại khỏi báo cáo.

Một lượt Python bridge test từng trả exit code 1 thay vì 2, chưa lấy được nguyên nhân ở lượt đó. Chạy lại riêng bridge và sau đó toàn bộ tests đều qua. Đã bổ sung stderr vào assertion để lần tái hiện sau có bằng chứng chẩn đoán; không kết luận đây là lỗi RAM khi chưa có chứng cứ.

Windows contest Job giữ handle đến khi tiến trình kết thúc và dùng `KILL_ON_JOB_CLOSE`; đã kiểm thử child/grandchild bị dọn cả khi supervisor thoát bình thường lẫn bị terminate. Đây là kiểm tra với tiến trình giả nhỏ, chưa thay thế phép đo crash của native model thật. Desktop giữ constructor và cap cũ. [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)

Toàn workspace qua 205 Rust tests và Clippy all-features. Sau đó thay đổi riêng LifeMate để kiểm tra cấu hình trước khi nạp model; 25 tests LifeMate/platform, targeted Clippy và release build mới đều qua. Binary candidate 3.435.520 bytes, SHA-256 `e09d6d23ae7483ac6679013cb73720abaff1a07eb6ed89bd171fc0c636813e9e`. Chạy `--help` không nạp model. [Receipt tổng hợp](lifemate-validation.json) giữ phạm vi và hash log kiểm chứng.

## Cách tái tạo

Build trên Windows từ repo root, không cần `.env.local` cho chế độ thi:

```powershell
$env:CARGO_BUILD_JOBS = '1'
./scripts/Build-Rust.ps1 -Action Test -Package nora-lifemate,nora-platform
./scripts/Build-Rust.ps1 -Action Clippy -Package nora-lifemate,nora-platform
./scripts/Build-Rust.ps1 -Action Release -Package nora-lifemate
python -m unittest discover -s scripts/tests -p 'test_lifemate*.py' -v
```

Chỉ chạy inference khi có đủ bộ nhớ thực. Dùng đường dẫn tuyệt đối tới Python, bridge, llama-server, whisper-server và models. `--asr-server` chọn GPU ASR; thiếu flag này dùng CPU worker. `--trace` chỉ được phép với train; trace có dữ liệu cần giữ private. `--limit` và `--start-index` dùng cho chẩn đoán, không được nhầm với submission đủ coverage.

Assets được pin trong [manifest](../assets/manifests/lifemate-offline.v1.json). Tổng logical bytes hai model là 3.314.979.083; con số này chưa gồm native runtime/image. Tensor metadata GGUF ghi khoảng 4,206B stored parameters; cộng ASR 809M khoảng 5,015B. Đây là kiểm tra mô tả, không thay cho cách BTC đếm architecture parameters.

Đã đối chiếu exact bytes/hash với metadata Hugging Face ở revision đã pin: file Qwen có last-file-commit ngày 02/03/2026, file Whisper Q5_0 ngày 01/10/2024, đều trước cutoff 26/10/2026. Đây là bằng chứng ngày tồn tại của hai artifact, không phải xác nhận quyền dữ liệu training hoặc security certification. Nguồn: [Qwen file metadata](https://huggingface.co/api/models/unsloth/Qwen3.5-4B-GGUF/tree/e87f176479d0855a907a41277aca2f8ee7a09523?expand=true), [Whisper file metadata](https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1?expand=true).

Chuẩn bị model local cho Docker mà không tải hoặc gửi dữ liệu ra ngoài:

```powershell
python scripts/prepare_lifemate_assets.py --llm PATH_TO_QWEN.gguf --asr PATH_TO_WHISPER.bin
```

Helper verify size/hash và stage vào `models/` được ignore, ưu tiên hardlink cùng filesystem. Giữ các file này bất biến; thêm `--copy` nếu cần bản byte copy độc lập. Recipe [lifemate.Dockerfile](../containers/lifemate.Dockerfile) pin source SHA, compile CUDA SM80 cho A30, giữ non-root và chỉ đưa hai model vào image. Máy hiện tại không có Docker/WSL; recipe vẫn là **candidate chưa build**, cần hoàn thiện notices và pin image digest trước khi phát hành.

Trên Linux GPU host đã được chuẩn bị, lệnh dưới đây là hướng dẫn **chưa được xác minh bằng lượt chạy**. Input mount read-only; output là thư mục mới do UID chạy container sở hữu:

```bash
docker build -f containers/lifemate.Dockerfile -t nora-lifemate:candidate .
mkdir result
docker run --rm --network none --gpus device=0 --cpus=16 \
  --memory=64g --memory-swap=64g --pids-limit=128 \
  --cap-drop=ALL --security-opt=no-new-privileges --read-only \
  --tmpfs /tmp:rw,nosuid,nodev,size=4g --user "$(id -u):$(id -g)" \
  -v /absolute/path/to/official/data:/data:ro \
  -v "$PWD/result:/app/result:rw" nora-lifemate:candidate
```

Bootstrap chọn private/public/train theo metadata dưới `/data`, hỗ trợ mount cả root hoặc một split, không mở environment. Docker entry cấu hình context 16.384 và ghi `/app/result/submission.jsonl`. Bài thi yêu cầu đường dẫn `result/submission.jsonl`; cách BTC khởi động container và tính cold start cần đối chiếu thêm khi có hướng dẫn Docker cuối cùng.

Đánh giá local train sau khi có predictions thật:

```powershell
python scripts/evaluate_lifemate_train.py --root DATA_ROOT --gold-receipts PRIVATE_GOLD_REPLAY.jsonl --predictions RUN/submission.jsonl --metrics RUN/samples.jsonl --partition development --require-complete-perfect --report NEW_REPORT.json --details NEW_DETAILS.jsonl
python scripts/verify_lifemate_replay.py --root DATA_ROOT --split train --predictions RUN/submission.jsonl --report NEW_REPLAY_REPORT.json
```

Oracle receipts và replay verification nằm ngoài inference. Replay chỉ kiểm tra tính nhất quán execution/hash; nó không chứng minh yêu cầu người dùng đã được hiểu đúng.

## Kiểm thử trên Colab miễn phí

Theo quyết định người dùng ngày 02/11/2026, không chạy model tiếp trên laptop. Phiên Colab đã cấp T4 GPU với khoảng 12,67 GB RAM; đây là môi trường kiểm thử bổ sung, không phải cấu hình A30 của BTC. Chỉ dùng quota miễn phí, không mua compute hoặc gọi hosted LLM.

Portal Public Submissions chỉ nhận file kết quả `.jsonl`, không nhận source để chạy GPU. ZIP chính thức là input; file sample submission có 3 dòng, không đủ 634 public samples và không phải output NORA.

Tạo kit riêng, không chứa dataset, model weights, `.env` hoặc Git metadata:

```powershell
python scripts/prepare_lifemate_remote_bundle.py --output .artifacts/nora-remote-gpu-NEW --embed-source
```

Notebook nhúng source ZIP đã pin SHA256 để không phụ thuộc hộp chọn file của trình duyệt. Không có `drive.mount()`; lưu notebook trong Drive và cấp toàn bộ Drive cho runtime là hai thao tác khác nhau. Quyền chia sẻ notebook cần giữ riêng tư. Chuẩn bị VM tải các assets đã pin, build native servers cho compute capability thực tế rồi build Rust. Log subprocess được chuyển vào notebook; wrapper mới dùng nhóm tiến trình riêng và dọn descendants khi kết thúc. Cleanup wrapper còn cần được kiểm chứng trên Linux trước khi gọi đó là crash certification.

Gói đầu gồm 109 source files, SHA256 `bec8085e0d6d8e63e2c9c5d72560a62a2f26a0248cd7d6155ef636ab031100f9`, đã xác minh trên VM. Lượt build Linux đầu đang chạy. Notebook thực nghiệm xếp smoke 3 public samples và separate SDK replay trước lượt đủ 634 mẫu; partial smoke không được dùng làm submission. Preflight đủ coverage, đúng receipts và không timeout vẫn không phải accuracy. Chỉ ghi nhận public score sau khi Zalo thực sự chấm entry.

Lượt build đầu đã hoàn tất `llama-server`, đang build `whisper-server`. Log phát hiện upstream tự lấy UI assets theo `latest` dù không dùng WebUI lúc inference. Đây là khoảng trống pin build inputs, không phải bằng chứng bị xâm phạm. Đã sửa helper và Docker recipe với `LLAMA_BUILD_UI=OFF` cùng `LLAMA_USE_PREBUILT_UI=OFF`. Helper chuyển UI cache cũ ra khỏi build tree, kiểm tra generated source không còn asset arrays và xác minh hash native executable trước khi chạy. Notebook đang chờ thực thi bước sửa/relink và ghi receipt; chưa gọi đó là hardening đã pass trên VM. Cơ chế fallback/cache được đối chiếu tại [CMake options của revision đã pin](https://github.com/ggml-org/llama.cpp/blob/8642541b255bfb5dc39cd351a178be09013a55d6/CMakeLists.txt) và [UI provisioning script](https://github.com/ggml-org/llama.cpp/blob/8642541b255bfb5dc39cd351a178be09013a55d6/scripts/ui-assets.cmake).

## Các bước còn phải đạt

1. Giữ V8 repeatability, V9 năm phép so sánh synthetic đã pass và 2B GGUF/HF formatting làm bằng chứng có phạm vi riêng. Toàn matrix V9, ordinary full-head parity, full-example update/save capacity, 4B full-corpus actor V2 template và e2e accuracy vẫn chưa verified. Giữ guard 512 MiB, thresholds và evaluation membership; chưa nâng synthetic/formatting thành task score.
2. Đạt checkpoint 99% rồi mục tiêu formal 100% trên toàn bộ development đã chọn; kiểm tra exact calls/state, timeout và coverage. Kiểm tra lại 5 lượt cấu hình freeze để phát hiện nondeterminism.
3. Freeze prompt/model/config rồi đánh giá blind holdout 310 mẫu. Nếu dùng lỗi holdout để cải thiện, ghi rõ holdout đã hết blind và tạo vòng đánh giá độc lập phù hợp.
4. Build và test offline Docker trên A30/64 GB; kiểm tra network isolation, output path, cold start và deadline theo hướng dẫn BTC.
5. Chạy đủ 634 public samples, kiểm tra format/replay và nhận điểm chính thức trước khi gọi đó là public 100%. Hidden score vẫn là kết quả riêng chưa biết.
6. Chỉ commit/push trực tiếp `main` khi đạt ngưỡng chất lượng thực đã kiểm chứng; checkpoint trung gian là 99%, mục tiêu formal là 100%. Cả hai chưa đạt và chưa commit các thay đổi thi ở thời điểm báo cáo này.

## Mốc thi và công khai

Freeze Docker/code/model trước **16/11/2026 23:59 GMT+7**; private test công bố **20/11 09:00**, kết quả private trước **23/11 23:59**. Portal cho tối đa 5 public submissions/ngày. Kiểm tra thông báo mới trước khi nộp. [LifeMate submission guidance](https://challenge.zalo.ai/portal/lifemate)

[Thể lệ tiếng Việt](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf), mục 4, giới hạn sử dụng/chia sẻ dữ liệu; mục 8.2 đặt điều kiện công khai mã thi trên diễn đàn chính thức nếu chia sẻ trong thời gian thi, trừ ngoại lệ được cho phép. Public GitHub một mình không chứng minh đã đáp ứng điều kiện diễn đàn. Không tự đăng forum hoặc liên hệ BTC trong công việc này.

Không commit dataset, raw audio, environment, predictions/traces riêng tư, credentials hoặc model weights. Dữ liệu thi không tự có quyền dùng để train sản phẩm thương mại. Code license, model license và data rights được đánh giá riêng; MIT của repo không cấp các quyền còn lại.


## Smoke GPU thực và lượt sửa tiếp theo

Bản native Linux/CUDA và Rust release đã build xong trên T4. Smoke001 chạy thật3 mẫu public:2 complete (15,06s và10,25s),1 lỗi transport ASR sau7ms; SDK replay3 receipts không mismatch hoặc schema rejection. Chưa biết task accuracy và chưa nộp entry NORA. Bản ghi metrics/output được giữ trong artifact riêng tư.

Log cho thấy ASR hai mẫu thành công khoảng0,3s; prompt prefill5–6k tokens mất khoảng3,7–4,1s và lặp lại mỗi turn. Nghi ngờ socket keepalive5s đã đóng trong lúc current-thread runtime không được poll khi LLM xử lý. Đã sửa pooling và bật prefix cache; source snapshot b2a1b092ce8cd526cc2109d8345efeba4bfda6cf0111938237bb8e319d9f485a được xác minh trên VM trước kiểm tra Rust và smoke002. Đây là giả thuyết đang kiểm chứng, chưa phải xác nhận100%. Không chạy model trên laptop, không dùng hosted LLM API cho bản thi, không mua compute.


Smoke002 trên cùng T4:3/3 complete, elapsed7,61/8,17/6,74s; ASR0,29–0,33s; replay0mismatch/0rejected. Một mẫu có3schema refusals nội bộ trước khi hoàn tất; đây chưa xác nhận đúng nhiệm vụ. Fmt,16 Linux Rust tests, release Clippy all-targets -Dwarnings và release build đã qua. Native llama relink nhúng0UI assets, tắt provisioning. Full634 public-run-002 đang chạy; chưa có điểm chính thức hoặc entry NORA. Bước nhận kết quả có6 synthetic transport tests qua, không chạy model trên laptop.


## Chất lượng thực sau smoke

Public-run-002 đã dừng chủ động ở77/634 mẫu vì6 lỗi LLM/loop; có62schema refusals. Không còn ASR transport failure cũ được ghi nhận, nhưng chưa đủ điều kiện nộp. GPU processes đã dọn sau interrupt. Không có public score NORA.

Baseline trên30 development samples đạt13,5556% theo metric local,3perfect/9state matches; cả30complete và SDK replay0mismatch. Đây chỉ là30/1235development, không phải fulltrain/holdout score. Không đưa nhãn hoặc transcript chuẩn vào model; gold được replay/chấm riêng sau inference. Traces cho thấy lỗi ASR và LLM hỏi thiếu thông tin/đoánID/gọi công cụ thừa.

Đang kiểm chứng decoder có schema tham số theo từng tool, x-one-of dạngat-least-one và thứ tự tool name trước arguments/actions trước finished. Không đổi SDK, globalJSON semantics hoặc trần desktop. Candidate source7d68247cc80cce7c76a2c247cfbf8114ce896b5174fa51d692df2abff72c212f; chưa có điểm A/B. Chưa commit/push main.


## Kiểm tra chất lượng ngày03/11

Các cấu hình trước vẫn chưa đạt chất lượng: trên cùng30development, legacy13,5556%; typed0,8333%; native sampling6,9048%; native greedy10%. Không chọn typed dùschema validity tốt hơn. Trên cùng10mẫu đầu, legacy/nativegreedy/no-cache đều0%, boundedthinking128tokens5%; không so trực tiếp5%/10mẫuvớiđiểm30mẫu.

ASR được đo riêng bằngRust qua đúngGpuSpeech củaNORA, không nạpLLM hoặcđọcgoldtronginference. Trên30development/32lượtphiênâm, PhoWhisper-medium giảmCERlocal24,5984→20,2386%; median299,72→439,32ms. Đây làdiagnosticNFC/lowercase/alphanumeric, không chuẩn hóacáchviếtsố vàkhôngphảiđiểmBTC. ModelVinAI2024 đãverify LFS/hashvàconvertF16 bằngweights_only, khôngTorchresidenttrongapp. Chưađổidefaultdesktop2GB.

Rust111sourcefilesnapshot908cc5ff20cfa7a61ea7435d3cad844c97366f58347e6e97cd9056ca6bd2fb48 qua21Linux tests,releaseClippy -Dwarnings,releasebuildvàASRexample. ĐangchạyNORAend-to-end vớiPhoWhisper vàpromptbỏthamsốtùychọnthừa; chưa cóđiểm mới,holdout/publicscore,submissionhoặccommit.


## V14 và validation rộng v32

V14 sustained QLoRA hoàn tất3updates rồi guard dừng backward2655: GPUfree530.579.456bytes, thấp hơn512MiB; owner/supervisor/group đã hết, ownswap0, không adapter save. Evidence `.artifacts/qlora-sustained-v14-result/` đã audit/readback7files; không coi là train đủ. V32 có Rust ID-only SHA-pinned cohort, giữ source order/model messages, cấm start/limit và public/private subset. Windows package109pass và Linuxwhisper release alltargets111pass,0fail/3ignored; fmt/Clippy/build Linux qua. Đang đo toàn123validation ngoài benchmark30 với400audio khác nhau bằng native unadapted2B; chưa có điểm hoàn chỉnh.

Hypothesis V32-B đã freeze trong lúc A chạy, trước khi biết điểm A: dùng cùng mọi input/source/asset/profile, chỉ --cpu-output on→off. A còn gần1,8GiB GPUfree nên kiểm tra GPU output placement có thể giảm latency; chưa đo B và chưa đổi default. A phải đủ123 và qua exact SDK audit trước B. V15 training table-offload là pilot riêng,8CPUpreflight checks qua; full945 GPU parity phải đạt trước7updates, chưa GPU launch. Mục tiêu100%/main gate vẫn giữ.


## Kết quả hoàn tất v32 / V15–V16

Hai arm123validation đều2,9810%,failures79/78,nonempty-perfect2/2; no placement promotion. V16 tối ưu CPUtable/cache qua6updates rồi guard dừngfull5177; không fullSFT/qualitygain. Codex đã ghi123open-coded diagnosticcards và sixproxygraders; driftbaseline còn thiếu. [Báo cáo cập nhật](development/lifemate-error-review-2026-11-03.md) ghi số liệu, hashes và giới hạn. GT171–174; chưa99/100, publicsubmission hoặc maincommit.


## 04/11/2026, 21:18 GMT+7: snapshot leaderboard mới

API công khai `leaderboard-model-type` hiện ghi BXT dẫn cột SLM96,58;
TrustedAI96,51 và Tấn Cường - Tèo96,25 đứng tiếp theo. APIs/Others của TrustedAI
là96,66, là cột khác và không thay cột SLM dùng để xếp hạng. Đây là snapshot
10đội đầu, điểm public tạm thời do đội nộp, không phải bằng chứng Docker đã được
BTC chạy/kiểm chứng hoặc NORA đã có điểm. Không suy ra thời gian A30 từ bảng này.
Mục tiêu>90 trung gian và real100 để commit/main giữ nguyên; chưa đạt.

Nguồn: [LifeMate Public Leaderboard](https://challenge.zalo.ai/portal/lifemate).
Response và URL/timestamp/hash lưu tại
`.artifacts/lifemate-leaderboard-20261104T141850Z/receipt.json`.
