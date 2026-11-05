# NORA: thử nghiệm kiến trúc cho LifeMate

Cập nhật ngày **03/11/2026, Asia/Saigon**. Người dùng cho phép thay đổi kiến trúc nếu phương án hiện tại không hiệu quả. Mục tiêu formal là 100% điểm tác vụ thực, với checkpoint trung gian 99%; cả hai chưa đạt. Tài liệu phân biệt kết quả đã đo, phương án đã loại và prototype chưa kiểm thử end-to-end.

Đọc cùng [báo cáo LifeMate](../lifemate-2026.md) và [validation hiện tại](../lifemate-validation.json).

## Rà soát hiện tại: ASR gain không chuyển thành task gain

Cặp Q3 all-GPU/context6144 trên cùng30development cho Whisper turbo **8,3333%**, PhoWhisper **3,3333%**, nonempty-perfect0/1, failure1/1. Calls/observations/state đã replay khớp SDK; không promote Pho cho task. Đây là đối chứng ASR dưới cùng Q3, không đo chất lượng Q3 so với Q4. V2 auditor sửa assertion inner-target bị kế thừa nhầm từ alias mode và giữ nguyên v1/hash; policy violations vẫn được báo riêng (A5/B9). Không thay score formula/cohort, không loại failure; native truncation coverage chưa được chứng minh.

Snapshot v30 thêm **catalog-bound clarification** opt-in: decoder target/param constraints và independent pre-SDK rejection, không thay default. Guard giữ mọi field khai báo kể cả optional vì requiredness không xác định missingness. Cặp30 đã đo baseline8,3333%→clarification5,0000%,nonempty-perfect0/0,failure1/1; SDK exact/inputs30/30khớp,không promote.8local refusals là lặp params trùng của một sample tới deadline; llama.cpp không enforce uniqueItems. V31 sửa bằng enum mọi nonempty unique ordered list trong budget; Windows287tests/Linux106tests qua,2native synthetic probes2/5fields qua,không dataset/SDK/taskscore. Chưa có empirical quality gain v31. Chi tiết và hashes nằm trong [review hiện hành](../development/project-review-2026-11-03.md).

## v28: dừng paired protocol ở deployment smoke A

Common source `b3741911604269ba5342baca85f2ed498da8ba31be546b1856fa083170d0fa61` chứa cả baseline compact-typed và few-shot experimental; source bundle/124 files, executable, native và model pins được kiểm tra. Protocol đã freeze context 8.192, 4B toàn GPU/output CPU và ASR GPU beam1 cho cả A/B. Arm A đã nạp model nhưng CUDA OOM ở request đầu (`cudaGraphInstantiate`); cả 3 mẫu failed, 0 calls. Hai lỗi sau là model process không còn sống. `exit0/status complete` của supervisor chỉ là xuất đủ receipts. Không chạy B hoặc paired30 theo protocol8192; không có kết quả so sánh chất lượng hay lý do promotion.

Receipt độc lập `.artifacts/lattice-runs/fewshot-a-smoke003/independent-collection.json`, SHA-256 `22c9484fc92c0e53b51b63de73be78669816ecd5fd6fc07467c552586be9997e`, xác nhận archive SHA-256 `be2392f8fd80d25e1ff21c89ac9ee9c60138b9fc1f611415d57e9d7bcd6dcb94`, pins, cohort order và counters. Sampled peak GPU device 3.902.799.872 bytes, owned3.846.176.768; Linux PSS1.089.678.336/RSS1.180.377.088 bytes, swap0. Sampling1s không đo instantaneous allocation OOM. Đây là thất bại profile triển khai đã freeze; chưa chứng minh mọi 4B/4GB impossible, effect few-shot hoặc tài nguyên A30. Profile mới phải được preregister riêng và giữ A/B matched trước inference.

## V11: hai train-only backward cases accepted, toàn plan dừng guard

**V11 — real train-only backward capacity, chưa full training fit:** 7 ví dụ đã được chọn trước, tất cả thuộc internal training; không chọn validation, outer holdout, benchmark30 hoặc public. Mỗi ví dụ dùng một process mới, giữ full causal input và thực hiện đúng một backward, 0 optimizer steps.

| Full tokens của ví dụ | Kết quả V11 |
|---:|---|
| 945 | Backward được accepted, 24 gradients hữu hạn |
| 1.940 | Backward được accepted; GPU free thấp nhất 542.310.400 bytes |
| 2.413 | 24 gradients hữu hạn, rồi guard dừng; không accepted case |
| 2.655 / 3.496 / 3.790 / 5.177 | Chưa chạy sau lần dừng đầu tiên |

Ví dụ 1.940 chỉ dư **5.439.488 bytes (~5,19 MiB)** trên guard 512 MiB. Với ví dụ 2.413, GPU free xuống **424.869.888 bytes**; cả current-free check và baseline-adjusted peak-reserved check đều không đạt. Peak CUDA allocated/reserved là **3.144.938.496 / 3.290.431.488 bytes**. Đây là self-imposed guard stop, **không OOM**; không hạ guard hoặc tự retry. Length riêng lẻ chưa chứng minh mọi input cùng độ dài đều fit.

Independent audit đã kiểm tra hai private gradient payloads của hai cases accepted; case dừng chỉ có gradient metadata hữu hạn, không có payload accepted. Reader tài liệu chỉ đọc source/selection hashes và metadata, không tải raw training text hoặc công bố private IDs/gradients. Toàn plan vẫn incomplete: **0 optimizer/update/adapter save**, chưa sustained same-process training, full-example capacity, numerical GGUF/export parity hoặc chất lượng tác vụ. Token/template parity và V10 synthetic parity giữ phạm vi riêng. Audit SHA `56dec768f5d3a9e23885f544daa2647556d12124b8bfd2b385d2a6b4c6b73611`; evidence riêng trong `.artifacts/qlora-real-train-result-v11/`. Các số GPU là saved checkpoints/Torch peaks, chưa chứng nhận global peak hoặc ngân sách desktop.

## PhoQ5 ASR-only, scoring-process scope và V10

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

## V29 paired30: mapping đúng, quality screen không đạt

**Cặp alias v29 đã chấm xong — loại profile alias này:** A/B dùng cùng source v29, context 6.144, model/ASR/SDK/deadline và đúng 30 IDs đã freeze; A compact-typed, B descriptive name/schema aliases.

| Paired 30 v29 | Điểm local | Perfect / nonempty-perfect | State matches | Failed |
|---|---:|---:|---:|---:|
| A — baseline mới | 8,3333% | 2 / 0 | 5 | 0 |
| B — alias experimental | 6,6667% | 2 / 0 | 5 | 0 |

B giảm 1,6667 điểm phần trăm: 0 mẫu tăng, 1 giảm, 29 giữ điểm. Cả 2 perfect/arm đều zero-action; nonempty-perfect 0/0. Profile không đạt gates gain >=10pp và thêm >=2 nonempty-perfect nên **không promote**, dù cả 60 mẫu complete dưới 30 giây và SDK integrity pass. Kết luận áp dụng rule/profile đã freeze, chưa loại mọi phương pháp alias hoặc chứng minh thống kê từ 30 development đã được phân tích trước.

Collection giữ 37 raw files/run và immutable hashes; source/assets/executable/supervisor/params/IDs khớp. Initial inputs khớp bytes 29/30; một khác biệt history có typed tool name được đổi đúng design, inverse mapping chỉ những field đó trả về equality 30/30. Independent SDK replay 30/arm có 0 mismatch/reject; 52/53 actual observed results và final receipts khớp, 5 unexecuted refusals/arm giữ nguyên. Full-split 147-entry alias map chuyển proposal về tên SDK gốc, 0 mapping errors; 75 responses/releases/arm đều truncated0, không OOM/CUDA/context/transport/deadline errors. Receipt/mapping hợp lệ chưa chứng minh hiểu đúng intent.

Sampled peak PSS A/B là **1.182.103.552 / 1.184.163.840**, RSS **1.272.700.928 / 1.274.773.504**, GPU device **3.751.804.928 bytes** cả hai; swap0, sampled mỗi giây. Prelaunch guards không được auditor hậu kiểm xác nhận độc lập; sampling chưa chứng nhận cap desktop 2 GB/SSD 15 GB/A30. Giữ nguyên source/SDK/gold/scorer/membership; best historical 13,5556% không đổi, chưa có holdout hoặc điểm chính thức.

Evidence: `.artifacts/lifemate-alias-v29-paired-audit-result-v1/paired-aggregate.json` SHA `b494e03028f6463128f64c32a0c0c579c8e382da09c89de35474f718fe886781`; independent readback SHA `93d581f5641e80a6cbc45547283dcf294d60bb94becbb67d0afe4a111719aa10`. Raw collections/replays ở `.artifacts/lattice-runs/alias29-{a,b}-paired030/`, archive SHA A `1154df32784ef70fea4e9b8f06fcbe1236411a2d73c9642d3fa4114238337dd5` / B `4f245b60855515ab57375e44291c2b36ff26aff7648228c99d42a96e6f88bfaa`.

## V29: schema aliases và PhoWhisper asset preparation

**V29 — source/build, token probe và technical smokes:** source mới `ad76310ddacc7c5e8ad53801ca681e33e0992f5baefceb1eee2e2680a96c8b48` có 127 files được kiểm tra từ bundle. Linux log ghi **99 tests pass, 3 ignored** (73 lib, 19 workflow, 4 alias, 1 CLI, 2 example); release build hoàn tất. Mode opt-in `compact-typed-alias-experimental` đổi tên công cụ model-visible theo catalog, rồi map structured proposals về tên SDK gốc trước security checks. Bijection dùng full split catalog 147 tools; user text/raw results không bị thay tên toàn cục, permissions/SDK/state receipts vẫn giữ quyền quyết định. Đây chưa phải default hoặc một model mới.

Alias token probe dùng 75 observed historical prefixes/arm: A thật từ v19, B là counterfactual translation theo Rust, không phải B đã sinh. Pinned HF/GGUF có 0 render/token-ID mismatch trên 150 rows, dùng **actual matching decision schemas**. Max prompt A là 3.223, B là 3.256; B + 768 reserve = 4.024, còn 2.120 tokens ở context 6.144. Old A khớp 75 render hashes/counts; readback chỉ ra non-system changes nằm ở structured tool fields. Có 0 generation/GPU/network; timing không isolated. Chưa bảo đảm trajectory mới, constrained generation, training fit hoặc quality.

Hai smoke v29 đã collect/replay: **A 3/3 và B 3/3 complete dưới 30 giây**, IDs/order/pins/parameters khớp trừ mode/tag. Input đầu khớp bytes 3/3; SDK replay có 0 mismatch/rejection cả hai, actual observations 7/arm và final receipts khớp. Alias proposals được kiểm tra về tênSDK gốc qua 147-entry map, 0 alias errors; 8 response/release records mỗi arm đều truncated0, không OOM/CUDA/context/transport/deadline errors. Technical screen đạt trong scope hậu kiểm; cặp30 mới sau smoke đã chấm riêng ở trên và profile alias bị loại. Sample resources không chứng nhận hard cap, A30 hoặc production.

Original prereg có lỗi chép manifest hash bị lặp substring. Original file được giữ nguyên; erratum SHA `c376c477851d9f246cd26735d3370367639db9a5122af228b54dea3d4dd3988f` ghi rõ **sau A smoke, trước B**, chỉ sửa transcription theo ZIP/ready đã pin; không thay source, cohort, mode, profile hoặc thresholds. Source/pins thực của hai smoke đã khớp độc lập. Evidence: `.artifacts/v29-phowhisper-metadata/continue-v29.log`, `.artifacts/lifemate-alias-v29-token-result-v2/independent-readback.json` và `.artifacts/lattice-runs/alias29-{a,b}-smoke003/`.

**PhoWhisper assets mới trên lattice:** receipts cho biết F16 đã chuyển đổi/verify **1.533.763.076 bytes**, SHA `24433822f81ab63826f35b4f6b6202c3724b0b43473700125c54029339ffee63`; Q5 đã tạo **539.212.484 bytes**, SHA `e2cd4ceda28646fc74e48f171fad8ebd75e0ab06817d70349bc42ae2335bfc27`. Source PhoWhisper-medium revision `55a7e3eb6c906de891f8f06a107754427dd3be79`; input manifest khớp metadata đã pin. Download-only receipt chưa khẳng định F16/Q5 ready; conversion/quantization receipts sau đó mới ghi trạng thái này. Cả ba ghi `asr_tested=false`, không đọc dataset/gọi LLM API. Collector chỉ lấy 5 metadata/log files đã whitelist, kiểm tra archive/member hashes; **không đọc hoặc rehash weights**. Asset receipts không đo ASR/quality/latency/full-app fit; ASR-only diagnostic sau đó có scope riêng ở trên. Metadata archive SHA `475556455301e7bf7c31e377b4355d118430d08a13b1c8fee5c1e82a32d25c6f`; receipts trong `.artifacts/v29-phowhisper-metadata/`.

## v28 paired 30: fixed few-shot profile bị loại

Hai lượt mới dùng chung source `b3741911604269ba5342baca85f2ed498da8ba31be546b1856fa083170d0fa61`, Qwen3-4B Instruct Q4_K_M, context 6.144, all-GPU/output CPU, ASR GPU beam1 và đúng 30 development IDs đã freeze. A là baseline compact-typed trên source mới, B chỉ thêm hai ví dụ synthetic cố định. Independent collection xác nhận 37 raw members/run, source/assets/executable/supervisor/parameters/ID order; initial inputs khớp bytes30/30 và system B=A+fixed suffix30/30.

| Chỉ số | A | B |
|---|---:|---:|
| Local score | 8,3333% | 11,6667% |
| Perfect / nonempty perfect | 2 / 0 | 3 / 0 |
| State matches | 5 | 7 |
| Failed generation deadlines | 0 | 2 |
| SDK replay mismatch / rejection | 0 / 0 | 0 / 0 |
| Sampled peak PSS | 1.179.256.832 bytes | 1.180.797.952 bytes |
| Sampled peak RSS | 1.270.095.872 bytes | 1.271.488.512 bytes |
| Sampled peak device GPU | 3.751.804.928 bytes | 3.751.804.928 bytes |

B tăng3,3333pp, chỉ một zero-action sample tốt hơn; 29 bằng điểm, không mẫu giảm. Cả2/3 perfect đều zero-action, nonempty perfect0/0. Profile này **không đạt** ba gates đã đăng ký: gain>=10pp, thêm>=2 nonempty perfect, failed không nhiều hơn A. Mọi elapsed<30giây không xóa hai status failed. Không promote B; chưa loại bỏ mọi phương pháp few-shot, chưa kết luận thống kê hoặc đạt99%/100%.

Independent SDK replay30/arm sạch; actual observations52/41 và final receipts khớp,5unexecuted refusals/arm được giữ. 75/67 native release records đều truncated0; B65captured responses +2deadline releases. Không systemic OOM/CUDA/context/transport/dead-model signals. Receipt integrity không chứng minh hiểu đúng intent; prelaunch guards không được auditor hậu kiểm độc lập xác nhận. Resource sampling1s không chứng nhận desktop2GB, vật lýVRAM4GB dưới mọi tải, SSD15GB hoặc A30. Cặp A-trước-B, fixed30 đã được phân tích trước và local fuzzy metric giới hạn phạm vi kết luận. Best lịch sử13,5556% dùng profile/máy khác vẫn giữ nguyên.

Evidence: `.artifacts/lifemate-fewshot-paired-audit-result-v1/paired-aggregate.json`; independently recomputed saved-detail readback SHA `64c3d9da73ef3ecb6984025dc8fd0076cf868ccee02e6b655d00eba0e6a275db`. Không chạy lại scorer để thay kết quả; không đổi gold, SDK hoặc evaluation membership. Giữ nguyên protocolv1/v2, failed8192 smoke, smokes6144 và toàn bộ paired30 artifacts. Các thử nghiệm tiếp theo cần protocol riêng, không điều chỉnh thresholds theo kết quả này.

## V9: partial isolated numerical parity, matrix chưa accepted

**V9 synthetic diagnostic:** năm phép so sánh có đủ dữ liệu đều khớp 24/24 gradient tensors theo bytes, numeric equality và exact loss, gồm cross-mode trên hai fixtures L347. Bốn child processes đã quan sát dùng cùng LoRA initialization/software fingerprint. Readback riêng kiểm tra hashes, finite values, layout và byte equality của 7 payloads đã lưu. Đây là scoped pass giữa aligned-reference và selected loss, chưa phải ordinary full-head equivalence.

Toàn matrix vẫn **incomplete/không accepted**: multi-target full repeat1 có gradient hữu hạn rồi dừng tại guard GPU free **527.630.336 bytes**, dưới ngưỡng 512 MiB không đổi. Có 7 accepted cases, chưa chạy EOS-boundary, không OOM, 0 optimizer steps hoặc adapter save. Peak CUDA allocated/reserved là 2.587.696.640/3.187.671.040 bytes; sampled RSS cao nhất 2.204.426.240 bytes. Saved checkpoints/Torch peaks chưa chứng minh continuous global peak hoặc cap desktop. Chưa có full-example training/update/save capacity, chất lượng tác vụ hoặc nguyên nhân riêng của V7 drift. Independent audit SHA `91f3caedf47a4288c100f6dab969ec2b22b13ede08d884d5b2c4521119554109`; local readback SHA `9f17d522059fa8cfa68b4de37c3fb9cb97dd285784e448f41be7a316fbd226f4`, trong `.artifacts/qlora-isolated-result-v9/`.

## v28 amended 6.144: token probe và paired deployment smoke

Sau khi giữ nguyên failure OOM ở 8.192, protocol v2 mới freeze 6.144 cho cả A/B, cùng source/assets/SDK/placement/deadline/sample IDs. CPU-only 4B probe dùng 75 observed prefix positions/arm từ 30 v19 traces, tổng 150 rows. Report có 0 render/token-ID mismatch; readback riêng xác nhận frozen IDs/unique positions, render hash equality, token counts và fit flags. Max A là 3.223, B là 3.484; B + 768 reserve = 4.252. Phạm vi là historical prefixes với dummy empty response schema, 0 generation/GPU; chưa chứng minh trajectory mới, constrained decoder fit hoặc 4B actor V2 full-corpus parity. Lượt đầu dừng tại CPU RSS guard 3 GiB, không OOM. Lượt nghiên cứu sau dùng guard 4 GiB, peak RSS 3.434.545.152 bytes, host available thấp nhất 10.282.139.648 bytes; chưa chứng nhận desktop 2 GB. Report SHA `15c873fd1534810ae45140a623257fa8b75adb17ce49e3f4512b505c07acb391`; local readback SHA `0141b2726e0d9fd4907b0821e53595be4ec0b3e4b57fe39c4b7ac9cac90b9755`.

Actual A/B smokes: A có 3 complete/0 failed/7 calls; B có 2 complete/1 generation deadline/4 calls. Mẫu B lỗi vẫn giữ failed tại 28,502 giây. Frozen 3 IDs/thứ tự và source/native/model/executable/supervisor/parameters khớp trừ mode/tag; initial user messages khớp bytes 3/3, system B bằng A cộng suffix cố định. Independent SDK replay có 3 samples/arm, 0 mismatch/reject; 7/4 actual observations khớp. Logs không có systemic OOM/CUDA/context/transport hoặc model chết; 8/7 completed native releases đều truncated0. Paired deployment screen đạt theo v2 cho phép deadline mẫu thông thường; một mẫu vẫn thất bại, chưa có quality screen. Device GPU sampled là 3.749.707.776 bytes cả hai, PSS là 1.148.390.400/1.131.310.080 bytes, swap0. Ba smoke chưa chứng minh lợi ích chất lượng, độ trễ hoặc khả năng tổng quát.

Receipts `.artifacts/lattice-runs/fewshot6144-{a,b}-smoke003/independent-collection.json` có SHA A `b9f6b2bdabc7139546c9cbccf771cca325c6d0a8eb41276bd52ca3a4d37510bc` / B `8246fc9d7417d29a41d1378cebed1e764d256444212624b93439dff063522af7`. Protocol v2 SHA `5148448455e5214e58ec615e7695ddc347e40688285b58473084c96afa091ac2` giữ thresholds ban đầu. Cặp 30 mới sau smoke đã hoàn tất và được chấm riêng ở trên. Historical 8,3333% không được dùng thay A mới; smoke không thay cho điểm và criteria đã đăng ký vẫn giữ nguyên.

## Bằng chứng hiện tại

Lượt `lattice-v19-dense-compact-typed-030` chạy audio thật trên lattice, dùng Qwen3-4B Instruct 2507 Q4_K_M, Whisper large-v3-turbo Q5_0, ASR beam 1, context 6.144 tokens và output/embedding LLM trên CPU:

| Chỉ số | Kết quả quan sát |
|---|---:|
| Development đã chạy | 30 / 1.235 mẫu |
| Điểm theo metric local | 8,3333% |
| Mẫu đạt điểm tuyệt đối | 2 / 30 |
| Trạng thái cuối khớp | 5 / 30 |
| Lỗi xử lý / timeout | 0 / 30 |
| Tool call sai schema trong file kết quả | 0 |
| SDK replay mismatch / schema rejection | 0 / 0 |
| RSS đỉnh được lấy mẫu, toàn cây tiến trình | 3.197.038.592 bytes |
| PSS đỉnh được lấy mẫu, toàn cây tiến trình | 3.106.097.152 bytes |
| GPU device memory đỉnh được lấy mẫu | 3.751.804.928 bytes |

Source snapshot: `65cee9bfd1851cecbf48ee4b3f70586dba85ef300990660403d8f6937db46f3d`. Executable SHA-256: `0feb9445cfe132fbcfdaf01462a9226198e64b5354dabba7e68f6852a8e56d53`. Receipts riêng tư nằm trong `.artifacts/lattice-runs/lattice-v19-dense-compact-typed-030/`, gồm `report.json`, `replay.json` và `run-receipt.json`.

Tài nguyên được lấy mẫu mỗi giây; GPU device memory gồm cả desktop baseline. PSS/RSS Linux không tương đương private commit của Windows, và lượt này không đặt hard cap 2 GB. Vì vậy kết quả chưa chứng nhận trần RAM desktop. Desktop vẫn có cấu hình riêng 2 GB + LLM API; bản thi offline được phép dùng cấu hình A30/64 GB của BTC.

Kết quả audio tốt nhất đã chấm trước đó là 13,5556% trên 30 mẫu, dùng T4 và profile khác. Hai kết quả chưa tách được ảnh hưởng của hardware, model, prompt và runtime. Chưa có điểm chính thức public/private, lượt đánh giá holdout hoặc chứng nhận A30.

Diagnostic dùng transcript chuẩn trên cùng 30 mẫu từng đạt tối đa 21,8283%, với 15/30 trạng thái cuối khớp. Các lỗi vẫn xuất hiện khi loại ASR khỏi đầu vào: thực hiện nhánh điều kiện sai, bỏ qua ngoại lệ phòng có điều hòa, dời giờ bắt đầu nhưng không dời giờ kết thúc, thực hiện yêu cầu đã rút lại và tự chọn người nhận còn mơ hồ. Điều này cho thấy cần kiểm tra cả mô hình lập kế hoạch và cách theo dõi tác vụ. Schema hợp lệ, tiến trình kết thúc bình thường và SDK replay đúng chưa chứng minh hiểu đúng yêu cầu.

## R1 và audit routing đã hoàn tất

R1 dùng **Qwen3.5-2B Q4_K_M**, source/executable v19, cùng 30 audio, Whisper beam1, compact-typed, context 6.144 và deadline 30 giây. R1 đặt toàn bộ tensor LLM trên GPU (`cpu_output=off`), còn R0 đưa output/embedding lên CPU. Vì thay cả model và tensor placement, đây là so sánh profile, chưa tách tác động của model.

| Chỉ số | R0 — dense 4B | R1 — Qwen3.5-2B |
|---|---:|---:|
| Điểm tác vụ local | 8,3333% | 3,0000% |
| Mẫu hoàn hảo | 2/30 | 0/30 |
| State matches | 5/30 | 8/30 |
| Mẫu lỗi | 0/30 | 15/30 |
| Median thời gian/mẫu | 17,29 giây | 21,13 giây |
| Sampled peak PSS | 3.106.097.152 bytes | 10.495.360.000 bytes |
| Sampled peak RSS | 3.197.038.592 bytes | 10.583.572.480 bytes |
| Sampled peak device GPU | 3.751.804.928 bytes | 2.405.433.344 bytes |
| Sampled peak swap | 0 bytes | 42.696.704 bytes |

R1 ghi 7 HTTP400, 5 lỗi hết model-turn budget, 2 generation deadlines và 1 lỗi hết call budget. Cả hai có 0 schema-invalid calls trong file kết quả và SDK replay 0 mismatch/0 rejection. **Loại profile R1 đã đo**; chưa chứng minh các lỗi do RAM/swap hoặc mọi model 2B đều kém. Receipt R1: `.artifacts/lattice-runs/lattice-v19-qwen35-2b-compact-typed-030/`. Các số tài nguyên Linux này không chứng nhận cap desktop 2 GB hay môi trường A30.

Audit router từ vựng lấy đầu vào request/history/catalog hợp lệ, không nhận gold, không đọc SDK environment và không gọi model/API. Gold tool names chỉ dùng sau inference để tính micro recall trên cặp `(sample, tool name)`, chưa xét arguments hoặc effects:

| Cohort development | Tool cần dùng được giữ | Recall | Fallback toàn catalog | Trung bình tool được chọn / catalog đầy đủ |
|---|---:|---:|---:|---:|
| Transcript chuẩn, 1.235 mẫu | 2.242 / 2.682 | 83,5943% | 280/1.235 | 12,68 / 33,50 |
| ASR thật, 30 mẫu | 55 / 68 | 80,8824% | 8/30 | 16,17 / 39,83 |

Router bỏ sót công cụ ngay cả trên transcript chuẩn. Với ASR thật, các thiếu sót tập trung ở calendar, banking, chat OTT và contacts; chuẩn hóa bỏ dấu cũng gây nhập nhằng từ. **Không dùng router này làm hard filter; tích hợp mode routing đã được gỡ khỏi app.** Giữ module và audit example để phát triển phương án có recall tốt hơn hoặc chỉ xếp hạng mà không loại công cụ. Không có điểm audio end-to-end của mode routing. Receipt audit riêng tư: `.artifacts/routing-audit-v1.json`, SHA-256 `4bfc026bee611eaed3770cd56c8009344dd65bcd90a189656788c1ca8fa05219`; không đưa examples/transcripts trong receipt đó lên Git.

## Cache control v20 và probe xLAM v21

Cache control `lattice-v20-qwen35-2b-cache0-typed-030` giữ cùng 30 IDs/thứ tự, LLM/ASR/native binaries, context, ASR beam, mode và tensor placement với R1. Hai knobs thay cùng lúc: **cache RAM mặc định 8 GiB → 0**, **context checkpoints 32 → 4**. Source v20 SHA-256 `62edc36195d0976e1eae965070b48ee5378c1f9ed9251492e9eb4b55746d7444` còn chứa prototype chưa dùng trong đường 2B; không coi hai snapshot source là giống hệt.

| Chỉ số | R1 v19 | Cache control v20 |
|---|---:|---:|
| Điểm local / mẫu lỗi | 3,0000% / 15 | 3,0000% / 15 |
| State matches / mẫu hoàn hảo | 8 / 0 | 8 / 0 |
| Sampled peak PSS | 10.495.360.000 bytes | 1.457.882.112 bytes |
| Sampled peak RSS | 10.583.572.480 bytes | 1.548.320.768 bytes |
| Sampled peak swap | 42.696.704 bytes | 0 bytes |
| Sampled peak device GPU | 2.405.433.344 bytes | 2.405.433.344 bytes |
| Median ASR/mẫu | 2.556,65 ms | 2.557,23 ms |
| Median model/mẫu | 18.542,04 ms | 18.450,47 ms |
| Median tổng/mẫu | 21.129,53 ms | 21.039,41 ms |

Cả **30/30 submission records và bytes JSONL giống nhau**, SHA-256 `9b15ebbd816997cf8be0eead1cd2bcb395eed6c731c71ec6bbc9d816427e1dd7`. Kết quả chỉ xác nhận hai lượt đã đo; chưa là phép kiểm tra determinism nhiều lượt. Chất lượng không tăng khi RAM giảm. Không tách nguyên nhân riêng cache RAM/checkpoints; không quy lỗi R1 cho swap từ tương quan này.

Tại sampled tree PSS peak của v20, `llama-server` đóng góp PSS 1.052.024.832 bytes, `whisper-server` 379.606.016 bytes; các tiến trình nhỏ chiếm phần còn lại. v19 không có `process_memory`, nên không xác định owner của peak cũ. Sampling mỗi giây có thể bỏ lỡ đỉnh ngắn; Linux RSS/PSS chưa chứng nhận hard cap Windows 2 GB, toàn app hoặc A30. Comparison private `.artifacts/cache-policy-v19-v20.json`, SHA-256 `3a2c9bc119197973b914ea66be55f6912ff3430dd8c1d44a60587b311cc54317`; đây là bằng chứng tài nguyên và output đã đo, không phải điểm official.

Source v21 SHA-256 `3f50338d52a84931d8704bfa709ab64c9ee42f42eaf417dd3f331ad3c709766d` qua **55 Linux tests**, fmt và release Clippy all-targets, gồm 12 tests typed executor và 6 tests xLAM parser. Typed executor vẫn chưa được nối với LLM/runtime NORA. Adapter xLAM bỏ chat parsing theo format đã chọn; chu trình synthetic **call → result → done** pass. Probe audio thật 3 mẫu đều lỗi (2 context overflows, 1 deadline), nên chưa mở lượt 30 và chưa có điểm chất lượng mới. Synthetic format receipt đã lưu trong `.artifacts/lattice-v21-xlam-format-probe/result.json`, SHA-256 `9507239810e10a4c3687ec879ab52d6f7ec5767a66663e1e262006a7a5a67288`; receipt ghi không dùng dataset/SDK. Audio probe và checks vẫn có phạm vi logs đã quan sát; receipt audio đầy đủ còn chờ tải.

## Semantic routing v22 và audit oracle

Recall audit semantic router đã chạy trên ASR thật 30 development, source `b38efadf23c1520b299c89d11f2fad6928ff4635811d189a53178fd92eca746a`, Qwen3-4B Instruct Q4_K_M. Router không nhận gold, không gọi SDK hoặc hosted API; gold tool names chỉ dùng để chấm sau khi có output.

| Chỉ số | Lexical router ASR30 | Semantic router v22 ASR30 |
|---|---:|---:|
| Tool cần dùng được giữ | 55/68 | 52/68 |
| Micro recall | 80,8824% | 76,4706% |
| Mẫu gold không rỗng giữ đủ tool | 21/27 | 18/27 |
| Fallback toàn catalog | 8/30 | 5/30 |
| Trung bình tools chọn / đầy đủ | 16,17 / 39,83 | 15,40 / 39,83 |
| Median / max semantic router | — | 3.301 / 4.601 ms |

**Loại cả semantic và lexical hard filters.** Semantic router mất tool cần dùng và thêm latency trước planner; chưa có cơ sở dùng để cắt catalog trong NORA. Chỉ số trên không chấm arguments, effects hoặc task completion. Report SHA-256 `50b4ce9e6fe02699e28d320384245651abc7f1fa2953a6daf613f9547036e60f`, receipt `9091fa58a32eb57c0964a221abee75e19918057640b04febb006b17d43f3694b`, trong `.artifacts/lattice-v22-semantic-router-asr030/`. Chỉ công bố aggregate; private missing examples được giữ riêng.

Replay gold `train_1696` xác nhận SDK source nguyên trạng (53 Python files khớp archive), 3 validate pass và 3 `ok=true`, trong đó một `done=false`. Actual results khớp label, calls/hash khớp oracle lịch sử. Oracle cũ đếm `not result.get("ok")`, nên zero failures không bao gồm accepted no-ops. Chưa có bằng chứng SDK/label inconsistency; không đọc environment trực tiếp hoặc `get_state` trong lần audit này, không sửa scorer/dữ liệu hay loại mẫu khỏi evaluation. Receipt private `.artifacts/lifemate-gold-anomaly-1696.json`, SHA-256 `864cd7fa921e5273d449fd726b9a5401cc57cd948548b49553e3ef5bcaa2b27c`.

## PlanTyped v23: một plan qua hai nhánh synthetic

Cả Qwen3.5-2B và Qwen3-4B Instruct GPU đã pass probe LLM → typed plan → executor. **Mỗi model sinh một plan**, rồi executor dùng lại nguyên plan đó với hai callback weather có kiểm soát. Khi `rain=false`, chỉ đọc weather và skip effect; khi `rain=true`, mới gọi effect với giá trị đã định. Không regenerate plan sau khi biết weather. Hai model có plan riêng, không phải plan byte-identical giữa các model.

| Chỉ số | Qwen3.5-2B Q4_K_M | Qwen3-4B Instruct Q4_K_M |
|---|---:|---:|
| Elapsed probe, gồm init | 5,52935 giây | 8,18259 giây |
| Native prompt eval | 2.022,039 ms / 774 tokens | 5.122,599 ms / 756 tokens |
| Native generation | 2.275,188 ms / 164 tokens | 1.859,682 ms / 75 tokens |
| Cùng plan pass false/true | Có | Có |

Native timing là các pha của request; elapsed bao gồm khởi tạo/runtime và không phải SLO của pipeline audio. Probe không có ASR, dataset, SDK hoặc hosted API, không phải accuracy LifeMate và chưa xác minh tác vụ thật hoặc đầy đủ semantics.

Source v23: `481cf947d3ff5996a7fa8168d6e89b5063216af0489092bfde0d3851a4f941df`; executable `44bc024b0cd0cb8ff252f67f6b524eec7712adff231a36ba6f47fa89c31c4578`. Log `continue-v23.log` có **62 tests pass**: 51 lib + 1 CLI preflight + 8 workflow + 2 semantic-example, 0 failed; fmt/release Clippy/release build qua. Export ZIP SHA-256 `d3d83b976848cf7151dd0bf8bdad672e7a9870e9bf01537b14ff9621c0fa8c46`; nội dung đã đối chiếu với `.artifacts/lattice-v23-plan-probe/`, giữ riêng `receipt.json`, `response.json`, `result.json` của từng model.

## PlanTyped v25: real audio smoke thất bại

`lattice-v25-qwen35-2b-plan-typed-003` đã đo actual runner/SDK với Qwen3.5-2B Q4_K_M, Whisper GPU beam1, mode `plan-typed-experimental`, context 12.288, deadline 30 giây/mẫu:

| Chỉ số | Kết quả |
|---|---:|
| Mẫu / local score | 3 / 0% |
| Failed / perfect / state matches | 3 / 0 / 1 |
| Unfinished decision / generation deadline | 2 / 1 |
| Elapsed/mẫu | 20,55–28,50 giây |
| Sampled peak RSS / PSS | 1.499.197.440 / 1.408.026.624 bytes |
| Sampled peak device GPU / swap | 2.457.862.144 / 0 bytes |
| SDK replay mismatch / schema rejection | 0 / 0 |

Mẫu hết deadline có 7 provenance refusals cho ID chưa được quan sát trước khi dừng; guard vẫn giữ nguyên. Hai mẫu còn lại không hoàn tất decision. Traces không lưu `finish_reason`, nên dùng lỗi trong metrics làm bằng chứng trực tiếp, chưa coi đây là phân tích root cause đầy đủ. Batch/supervisor exit0 không chứng minh mẫu thành công. Lượt 30 **chưa chạy**; không loại 3 failed samples khỏi báo cáo. Source `01d115c667c52c6f9a4cab129109cb6b08d060c82c175914e78c83e143d08c7f`; archive SHA-256 `ead0f9417cd3c9fb5fca0c707b63b3ed7677f2b88de7de8c68ceb7024606aaed`, trong `.artifacts/lattice-runs/lattice-v25-qwen35-2b-plan-typed-003/`. Tài nguyên là samples Linux, không hard-cap/production certification.

Synthetic v23 vẫn pass trong phạm vi toy Boolean branch; kết quả v25 không cho phép nâng luồng typed plan thành cải thiện task accuracy. Best real-audio lịch sử vẫn 13,5556% trên T4/profile khác; checkpoint 99% và formal 100% chưa đạt.

## Audit R0 và phương án v26

Audit read-only toàn 30 R0 giữ nguyên gold, scorer, membership và các kết quả cũ. Report mean 8,3333% được đối chiếu với existing per-sample details; actual calls trong traces khớp observations và separate SDK replay 0 mismatch.

- Hai perfect đều là zero-action gold của lệnh phủ định/rút lại; **0 nonempty-gold perfect**. Một empty match đi cùng mất nội dung/retraction ở ASR nên chưa chứng minh hiểu đúng ý định gốc.
- Trong 5 state matches, 2 có F1=0. Một route sai vẫn có hash khớp; hash equality tự nó chưa chứng minh action semantics.
- Một effect đúng chỉ đạt partial score do lookup query exact-match khác chữ hoa/thường. Giữ nguyên gold/scorer và báo riêng lỗi argument contract; không thêm casing hack theo gold.
- Primary observed gaps: ASR semantic loss 9, lookup/context/entity 7, condition/exception/negation 4. Các tag chồng lấp gồm entity/lookup 16 và unrequested optional arguments 14; không cộng chúng như các mẫu độc lập.

Đây là phân loại post-hoc theo traces/reference text, chưa phải causal ablation hay calibrated ASR WER. Không chạy counterfactual inference trong audit. Private artifacts `.artifacts/lattice-r0-error-taxonomy.md/json`, JSON SHA-256 `d5c83f0f8e315fe0c3d17ba064b5f8f0f505a414f3c58599b0f7670250fd0e71`; public docs không chứa ledger/raw examples.

## Cặp xLAM v26: chưa có quality gain

Hai profile dùng cùng source `e75d4bfd0a3c04a073597519ab16db93c939faea68c289231f2a99aa575b8c7f`, executable, xLAM-2-3B Q4_K_M, native pins, 3 IDs/thứ tự, context 8.192, ASR beam1 và CPU output on. Progressive giữ đủ tool index, mở schema theo yêu cầu; control dùng full catalog.

| Chỉ số | Progressive | Full catalog |
|---|---:|---:|
| Local score / failed / state match | 0% / 3 / 1 | 0% / 3 / 1 |
| Executed SDK calls / mẫu | 1, 1, 1 | 0, 0, 0 |
| Elapsed / mẫu | 21,12–26,11 giây | khoảng 28,50 giây |
| Sampled peak RSS | 1.156.788.224 bytes | 1.146.568.704 bytes |
| Sampled peak PSS | 1.065.694.208 bytes | 1.056.154.624 bytes |
| Sampled peak device GPU | 2.912.944.128 bytes | 2.910.846.976 bytes |
| Swap / replay mismatches | 0 / 0 | 0 / 0 |

Progressive thực thi được một SDK call/mẫu nhưng không hoàn tất đúng: lỗi unavailable/invalid call, hết model-turn budget và empty completion khi SDK action còn unfinished; có một provenance refusal. Full catalog gặp generation deadline ở cả ba mẫu trước SDK call. Supervisor exit0 và replay integrity không đổi 3/3 failed samples thành thành công. **Không mở lượt 30, không promote phương án hoặc suy rộng thống kê từ 3 mẫu.**

ASR full-catalog mẫu 2/3 là 5.681,06/5.587,79 ms, còn progressive 2.593,81/2.568,15 ms. Vì điều kiện timing quan sát không đồng đều, chưa gán causal advantage cho progressive. Pure test sau đó tái hiện lỗi socket cleanup riêng; chưa có actual measured rerun để gán nó là nguyên nhân của deadline native hoặc đo tác động lên chất lượng.

Hai archive SHA-256: progressive `00c4617db7d95a2e320e7e36898257b3ee43133f6ccedf1a357677507647e598`; full catalog `a4bb272bd6c3386afcde4140648ef81fed65767484402d3dd689332da13bf9c8`. Report/replay/receipts được đối chiếu trong `.artifacts/lattice-runs/lattice-v26-xlam-{progressive,full-catalog}-003/`. V26 Linux checks: **85 tests** (64 lib + 1 CLI + 18 workflow + 2 semantic-example); core-only Windows **79 tests** (61 lib + 18 workflow), Clippy/fmt pass. Checks gắn source, chưa chứng minh chất lượng hay hard cap tài nguyên. Default desktop không đổi.

## Socket cleanup: bằng chứng fixture và giới hạn

Fake HTTP loopback không trả response; client timeout 100 ms, sau đó code đồng bộ chờ socket EOF mà không gọi `block_on` lần nữa. Cùng fixture hợp lệ:

| Runtime | Kết quả | Thời gian test |
|---|---|---:|
| `current_thread`, log `http-cancellation-current-thread-v2.log` | Không thấy EOF trong cửa sổ chờ, test fail | 1,13 giây |
| `new_multi_thread`, một background worker | EOF cleanup tiến triển, test pass | 0,11 giây |

Test đầu trong `http-cancellation-current-thread.log` panic vì thiếu reactor context; giữ như lỗi setup, **không dùng để chứng minh cơ chế**. Hai test hợp lệ xác nhận cleanup ngoài `block_on` trong fixture. [Tokio: bridging with sync code](https://tokio.rs/tokio/topics/bridging) mô tả `current_thread` chỉ tiến triển khi gọi `block_on`; background runtime phù hợp khi tasks cần tiếp tục ngoài lời gọi đó.

Factory runtime chung trong code mới dùng một worker background cho LocalModel và GpuSpeech. Đây chưa phải bằng chứng mọi timeout đã sửa, server native hủy generation đúng, hoặc GPU/ASR/task accuracy cải thiện; 1,13/0,11 giây là thời gian pure test, không SLO model. Native v27 đã đo ở ba profile phía dưới; điểm vẫn0%. Causal effect của socket fix trên lỗi thực chưa được cô lập, không promote từ pure fixture hoặc kết quả xLAM.

## V27 actual native controls

Source `64d12e50ce265140505dce4581a6054f77209e79dbe8c81552a9b061a13f7441`, executable `82a03eb422b0b07370c2f415f9726684e6856cd5911f82a0f5dea99ffe99d76e`; cùng context 8.192, Whisper GPU beam1 và deadline 30s. V27 Linux 86 tests (65 lib +1 CLI +18 workflow +2 example), Windows core 80 (62 lib + 18 workflow), source-scoped Clippy/fmt pass.

| Profile | Local score | Failed / state matches | Executed SDK calls/mẫu | PSS / device GPU peak |
|---|---:|---:|---|---:|
| xLAM full catalog, output CPU | 0% | 3 / 1 | 0,0,0 | 1.044.081.664 / 2.910.846.976 bytes |
| Qwen3.5-2B progressive greedy, output GPU | 0% | 2 / 1 | 2,7,10 | 1.377.778.688 / 2.422.210.560 bytes |
| Qwen3.5-2B progressive custom sampling, output GPU | 0% | 1 / 1 | 1,2,3 | 1.374.185.472 / 2.422.210.560 bytes |

Các lỗi mẫu của ba profile đều là generation deadline; Qwen có thêm mẫu complete nhưng chưa đúng tác vụ. Cả ba replay 0 mismatch/0 schema rejection, swap 0 và **chưa chạy 30**. Không có quality gain từ việc số failed giảm 2→1 trên ba mẫu. Mọi failures và sample membership được giữ nguyên.

Sampling params được xác minh từ source v27 đã pin: `temperature=0.7, top_p=0.8, top_k=20, min_p=0, presence_penalty=1.5`. [Qwen model card tại revision đã pin](https://huggingface.co/Qwen/Qwen3.5-2B/blob/15852e8c16360a2fea060d615a32b45270f8a8fc/README.md) phân biệt text non-thinking `temperature=1.0, top_p=1.0, presence_penalty=2.0`; tổ hợp 0.7/0.8/1.5 ở phần VL. Vì vậy gọi profile này là **custom sampling**, chưa phải benchmark text preset nhà phát hành. Greedy temperature 0 có lựa chọn mode khác, nên chưa gọi cặp này là isolated-temperature ablation.

Matched xLAM v26/v27 full catalog dùng cùng model/native pins/context/CPU output/ASR beam; ASR lần lượt 2.570,71/5.681,06/5.587,79ms và 2.574,02/3.401,73/3.754,18ms. Đây là một paired3 observation sau background-runtime change, không randomized/repeated causal test. Cả hai vẫn 3/3 generation deadlines/0% score; chưa chứng minh ảnh hưởng quality hoặc mọi timeout đã sửa.

Ba cặp `messages` JSON v26/v27 xLAM full catalog bằng nhau; UTF-8 serialization cùng quy tắc cũng khớp. Log task đầu cho thấy:

| Server-relative timing | v26 current-thread | v27 background runtime |
|---|---:|---:|
| Task0 launch | 6,527676 s | 6,895942 s |
| `model_ms` | 25.862,467326 ms | 25.855,861404 ms |
| Launch + `model_ms`, xấp xỉ | 32,390143 s | 32,751803 s |
| Cancel event trong server log | 38,530698 s | 32,898310 s |
| Chênh cancel − mốc xấp xỉ | 6,141 s | 0,147 s |

Mỗi server có clock relative riêng. `model_ms` bọc invocation ngoài launch, nên mốc cộng và độ chênh chỉ là xấp xỉ, không phải instrumentation trực tiếp của deadline. Đây là bằng chứng timing ở một cohort đã đối chiếu bên cạnh pure socket fixture; chưa đủ để kết luận GPU contention gây ASR chậm, mọi cancellation đã sửa hoặc latency/accuracy được bảo đảm. Cả hai task profiles vẫn0%/3failed.

Receipts/report/replay trong `.artifacts/lattice-runs/lattice-v27-{xlam-full-catalog,qwen35-2b-native-progressive,qwen35-2b-native-sampling}-003/`; hashes được lưu trong validation. Default desktop không đổi.

## Exact tokenizer pilot và capacity plan

Tokenizer-only CPU pilot **trên dataset V1** đã kiểm tra đủ **5.096 full sequences/model**, không truncation: semantic train/validation 1.082/153, actor train/validation 3.377/484. Qwen3.5-2B actor full max **7.728 tokens**, Qwen3-4B Instruct max **7.940**; mọi sequence trong giới hạn 8.192 với đúng template/tokenizer đã pin. Prefix identity, token roundtrip, target order/EOS đều được kiểm tra đủ các records. Receipt `.artifacts/lifemate-tokenizer-pilot-v1/linux-receipts/report.json`, SHA-256 `ad3094f3572ce26cda64ae20cd3454e83d722a3f49af9a0005cf0b141ce6c940`.

Pilot không load weights/model class, không inference/GPU/training; network attempts 0, các model-runtime imports bị chặn. Điều này chỉ kiểm tra tokenization/độ dài, **không chứng minh QLoRA/full training vừa 4 GB VRAM, tốc độ train hay task accuracy**. Capacity v1 đã launch nhưng guard preflight nhận nhầm desktop compositor, dừng trước quantized-model load, 0 optimizer steps; không có loss/gradient parity hoặc memory-capacity evidence. Không kill process hoặc đổi profile/retry trong v1. V2 đã chạy sau guard correction và cũng terminal failed trước optimizer step; trạng thái các pilot tiếp theo theo handoff owner riêng. Phải đo activations/optimizer/peak memory và giữ nguyên sequences, không truncate mất thông tin để gọi là fit.

V1 evidence riêng `.artifacts/qlora-capacity-result-v1/summary.md` và `verified-result.json`. Nhãn gốc trong report được giữ; verified interpretation xác định guard-plumbing failure, không kết luận thiếu VRAM hoặc tự giảm model từ lần này.

## QLoRA capacity v2: load được, backward chưa qua

V2 receipts đã tải trong `.artifacts/qlora-capacity-result-v2/`; exact reviewed script và terminal logs được giữ. Load đạt 617 non-MTP tensors, tied FP16 embedding và 12 q/v LoRA modules; snapshot sau load có GPU free 1.829.961.728 bytes (1.745,2 MiB). Guard/invariants trong exact source đã đi qua trước `after_lora_injection`.

Synthetic masked backward fail điều kiện `grad is not None` và finite gộp chung. Chưa biết parameter nào, gradient missing hay NaN/Inf, hoặc cơ chế gây lỗi. Sau đó serializing loading metadata chứa `set` gây lỗi phụ; report hoàn chỉnh không được ghi. `reconstructed-failure-evidence.json` là evidence tái dựng từ logs/nguồn đã pin, SHA-256 `31df270067af19aa8eef116d2dcbe3b663ef737250717d19f48c1e808c223c80`, không sửa raw logs để biến thành pass.

**0 optimizer steps**, không có loss/gradient parity pass, backward peak chưa được ghi, chưa chứng minh train vừa 4 GB VRAM hoặc có task quality. Không ghi nhận OOM/unsupported-kernel trong logs này và không quy lỗi cho model size. Compositor fallback là resource-only: PID/starttime/UID/comm/driver-name được anchor, GPU ≤16 MiB vẫn tính baseline; executable không được attestation do EACCES, không kill process. Đây không phải xác minh executable độc lập.

## V3 diagnostics: finite loss, layer3 NaN gradients

V3 đã chạy và terminal failed ở synthetic full-masked backward: loss **2,6261706352233887** finite, input 34 tokens/supervised 10, scale **128**. Tất cả 24 trainable gradient tensors present; bốn tensors q_proj/v_proj LoRA A/B layer3 chứa tổng **34.816 NaNs**, 0 Inf; 20 tensors ở layers 7/11/15/19/23 finite, gồm cả zero gradients. Không còn ambiguity missing-vs-nonfinite của V2, nhưng **chưa chứng minh scale hay FP16/torch fallback là nguyên nhân**.

Report V3 serialize đầy đủ sau metadata-only normalization; **0 optimizer steps**, không OOM, chưa loss/gradient parity pass. Recorded CUDA reserved max **2.105.540.608 bytes**, GPU free min **1.662.189.568 bytes** trong diagnostic; chưa đo full-length training activations/optimizer hoặc task quality. Capacity input vẫn là V1 full-schema; không gán số đo này cho runtime dataset V2.

Exact script SHA-256 `886b6c7aff3c9c0fbee0c939665361d631b63b39ba3bdc0139069c05dbf335e6`; report `.artifacts/qlora-capacity-result-v3/capacity-report.json` SHA-256 `177da564fe5c43ec729f1721dd14c1eafc3b7259ea3dd39043486ff33bf06e00`. Raw logs, gradient diagnostics và empty steps retained. V4 đã hoàn tất sáu synthetic backward cases; kết quả và giới hạn ở phần tiếp theo. Active-job field không bị thay đổi bởi docs update này.

## Development gold/SDK observation parity

Audit `.artifacts/development-gold-observation-audit-v1/summary.json` kiểm tra **1.235 development samples/2.811 calls**, validate rồi call một lần theo gold order: 0 rejects, 0 label-result mismatches, **1 done=false**; `anomalies.jsonl` 0 bytes. Không state/hash access, không trực tiếp đọc environment; mỗi sample một ToolEnv.

Script decode từng train row để lấy ID rồi lọc outer holdout **trước label inspection/ToolEnv**; không dùng label holdout, không khẳng định bytes chưa decode. Summary SHA-256 `ce965897983e6e617fc5204aa640af45daa12c5463fd16478ac78a63e1e225b4`; script actual SHA-256 `8115a9ebf056f4d65cd92e1224878f684181a08464923a27cdf3ad39bfcc5e7f` đã đối chiếu. Đây là observation consistency của training source; chưa chứng minh model hiểu yêu cầu, hành động/gating/completion targets đúng hoặc tác vụ đạt accuracy cao. Giữ nguyên evaluation gold/scorer/membership và tất cả failed runs.

## V4 numerical isolation: finite gradients chưa phải parity

V4 source `b9a1c9f07b43ebafd16f79d745d6acf85eb05c502d667953fd0d834f00f6dc7c` hoàn tất 6 synthetic backward cases (3 scales × full-masked/selective), 0 optimizer steps, adapters không đổi/không save, không network/OOM/swap. Logits và loss FP32 **2,6261706352233887** đều finite.

| Scale | Gradients full / selective | Strict parity | Global relativeL2 |
|---|---|---|---:|
| 128 | Cả hai 4 NaN tensors layer3 q/v, 0 missing | Không đánh giá được toàn 24 finite | — |
| 8 | Cả hai 24/24 finite/present | Fail | 0,0028397302 |
| 1 | Cả hai 24/24 finite/present | Fail | 0,0026738186 |

Thresholds strict elementwise `rtol=.02, atol=2e-5` giữ nguyên; global relativeL2 nhỏ không thay một per-tensor parity pass. V4 không lưu gradient arrays/per-tensor deltas, nên không suy ra cosine/sign flips/analytic Adam updates từ norm summaries. Scales 8/1 finite chỉ hỗ trợ nhận định lỗi nhạy với scale 128 trong fixture, chưa xác định cơ chế hoặc safe training scale.

Boundary backward output nonfinite đầu tiên ghi `layer.4.block`; đây là observed hook boundary, không phải exact failing arithmetic op. Recorded CUDA reserved peak 2.365.587.456 bytes và sampled RSS 2.327.437.312 bytes có overhead trace/synthetic 34 tokens; chưa full-actor-example capacity. Analysis `.artifacts/qlora-numerical-result-v4/analysis.json` SHA-256 `03e89ca43a20a759735ae54100fab9386d8bbdfef563a2ca5a5d2bb2a84a3477`, full report SHA-256 `6b0298daa5fa3f2038de84591bd122f627bf34b0f65e6cf4b99f5564fe0c47c8`.

V5 matched control sau đó đã chạy và kết quả ở phần tiếp theo. V4 terminal không phải successful training/capacity/quality; không tự đổi thresholds hoặc mở 20-step training retry từ kết quả này.

## V5: repeatable within mode, cross-mode parity vẫn fail

Source `763d55de2fdc8fd0225886e89223e26da37552099e084e4f61eca47fb8e8004f`, 8 synthetic backwards tại scale 8: mỗi native FP16/CPU FP32-head có 2 full + 2 selective repeats. Tất cả 24/24 gradients finite; within-mode repeats bitwise-identical/loss equal. **0 optimizer steps**, adapters unchanged, head restored, không network.

| Cross-mode head control | Strict mismatches | Nonzero sign flips | Max absolute delta | Min nonzero cosine |
|---|---:|---:|---:|---:|
| Native FP16 | 1.001 | 87 | 0,0015926361 | 0,9999940026 |
| CPU FP32 | 755 | 60 | 0,0007858276 | 0,9999957620 |

Original thresholds giữ nguyên. Sign flips ở magnitude >atol lần lượt 15/14; nhiều flips gần zero cũng được báo, không loại để gọi pass. Predeclared criterion đòi FP32-head cross-mode all24 strict pass; **không đạt**, nên FP32 head alone chưa giải quyết parity hoặc xác định root-op. Reference chỉ là FP32 output head trên exact frozen FP16 weights; body/backward vẫn FP16/NF4, không phải full-model FP32 reference. Layer4 boundary của V4 vẫn unresolved.

V5 đã giữ 8 derivative arrays (6.684.672 bytes). Audit stdlib CPU độc lập đọc/hash toàn bộ arrays và tính lại 8 comparisons, mismatches/sign flips khớp. Với global norm clip1, Adam zero-moment/epsilon1e-8/weight-decay0/lr1e-4 analytic first-step có cross-mode relativeL2 **0,0538047498/0,0444609424**, max deltas **1,99978/1,99943×lr**. Đây là mô phỏng analytic từ gradients, chưa có optimizer hoặc parameter update và chưa đại diện đầy đủ training step/moments.

Analysis `.artifacts/qlora-parity-result-v5/analysis.json` SHA-256 `0d9e5a6fa5ad550e54025f9cdfd58e9059432ee2dda402c637fc235786d717a3`; independent `offline-array-audit.json` SHA-256 `5da29e945d917b483d39549c745a05238bf183225ded6cbdc7234c63e2492aa9`; audit script SHA-256 `6c79d91e7cd78c85e445f0af3d9bfb6bc3c45354a25afebb9204c04ee3474c3a`. Hashes và analytic metrics đã được read-only CPU recompute, không model/GPU/optimizer.

CPU-head research process RSS peak **4.392.218.624 bytes**, CUDA reserved **2.365.587.456**, GPU free min **1.400.045.568**, swap delta 0. Research-only temporary CPU head không phải desktop deployment profile hoặc full-example training capacity. Chưa task quality/production readiness. Kết quả V6 real-model guard stop và tiến độ GGUF/HF probe ở các phần dưới; V5 không có parity/training promotion.

## V6: finite ordinary backward, resource guard stop trước aligned modes

Source `060def5775f0573fac297e570e1bcdce53d59fdbcd4cb0f07e2a03688ffe6d9b` đã chạy first ordinary-full synthetic case L347 trên real2B model, scale8/supervised10. Materialized **86.167.040 FP16 logits (172.334.080 bytes)** đều finite, loss **2,8581173419952393** và24/24gradient tensors finite. Sau backward, unchanged **512 MiB headroom guard** dừng profile khi free chỉ473.104.384 bytes (451,1875MiB).

| V6 recorded metric | Quan sát |
|---|---:|
| CUDA allocated peak | 3.005.579.776 bytes |
| CUDA reserved peak | 3.292.528.640 bytes |
| GPU free minimum | 473.104.384 bytes |
| Process RSS sampled peak | 2.258.575.360 bytes |
| OOM / accepted cases / optimizer steps | Không / 0 / 0 |
| Aligned modes reached | Không |

First backward có evidence nhưng guard stop trước khi tăng `completed_cases`; failure report đã được ghi, không gọi là completed diagnostic case hoặc parity comparison. Không có adapter save/retry; timings có concurrent CPU work, không throughput benchmark. **Không suy ra 4GB model bất khả thi, parity success hoặc full-example training fit.**

V6 `.artifacts/qlora-aligned-result-v6/analysis.json` SHA-256 `26e1f4d77983df88953713aff7dc8484ba2159359e2d410de76de7379b771fff`; report `f441381df24bf6a13b7c4d6716d8bdb53c6e098deb69e00452c1534bcc0c58f3`. Indexed artifacts/controller/script hashes đã đối chiếu. V7 selected-first sau đó đã chạy và kết quả ở phần tiếp theo; guard giữ nguyên.

## V7 selected-first: partial matrix, short-target repeats chưa qua

Source `656f0782425cb094090739dc973d07ac684dec040d38b0dfb067a6153001ac8a`, planned12backwards/3fixtures,scale8. **6 cases completed**; seventh multi-target aligned-full backward có loss0,562214613/24finitegradients nhưng self-imposed512MiB headroom guard dừng ở free **531.824.640 bytes (507,1875MiB)**. CUDA allocated peak2.538.638.848/reserved3.233.808.384 bytes. Không OOM, optimizer hoặc adapter save; guard không đổi.

| Completed-case comparison | Strict mismatches | Nonzero sign flips | Strict all24 pass | Bitwise equal |
|---|---:|---:|---|---|
| L347 short-target selected repeat | 571 | 45 | Không | Không |
| L347 short-target full repeat | 190 | 15 | Không | Không |
| L347 short full-vs-selected | 443 | 34 | Không | Không |
| Multi-target selected repeat | 0 | 0 | Có | Không |

Các within-mode short-target repeats cũng fail nên **không attribution discrepancy cho tile selection alone** hoặc gọi overall repeatability/parity success. Multi-target full counterpart/repeat và toàn EOS-boundary fixture chưa đo; matrix incomplete. Finite gradients của seventh backward không có completed-case array/acceptance sau guard failure.

Offline audit chỉ đọc six completed derivative arrays, hashes khớp; không model/GPU. `.artifacts/qlora-aligned-result-v7/offline-completed-case-audit.json` SHA-256 `e19f617046dd93eaaeed0066b6d7d65c6c031e495bceadb5457986ea3e7fd2af`; report SHA-256 `fce012458eff37eb0198bb5da43c0510c998b3bb933bbed9c3a0b1dd9cc33804`. Đây chưa full-example training capacity, task accuracy hoặc desktop fit. V8 selected-only repeatability sau đó đã có independent pass trong scope hẹp ở phần tiếp theo; không nâng thành cross-mode/full-training quality.

## V8: scoped selected-only repeatability pass

V8 source `e78d2da885eb6171c5cd7dcc40754f6f9a148bf70912f9e1ca6d4676305e94d1` hoàn tất 6 backwards/0 optimizer. Independent CPU audit verify 6 array hashes: cả 3 known fixture pairs có exact loss và all24 byte/zero-tolerance-numeric equality; original strict test cũng pass.

| Selected-only fixture | Input / supervised tokens | Selected head positions | Repeat result |
|---|---:|---:|---|
| Long prefix, short completion | 347 /10 | 91 | Exact loss +24byte-identical gradients |
| Multi-target completion | 347 /155 | 219 | Exact loss +24byte-identical gradients |
| EOS at tile boundary | 386 /10 | 128 | Exact loss +24byte-identical gradients |

Saved resource checks pass: CUDA reserved peak 3.087.007.744 bytes, GPU free min 628.293.632 bytes, process RSS sampled max 2.277.515.264 bytes, swap deltas 0. Đây là audit saved checkpoints/Torch counters, chưa continuous global-driver peak hoặc desktop/full-training resource certification.

Determinism settings đổi cùng lúc; chưa cô lập individual source của V7 repeat drift. **Selected-only same-mode repeatability đã pass cho pinned model/software/device/3 fixtures này; aligned-full cross-mode parity, full-example gradients/optimizer/save capacity và task quality chưa pass.** Không optimizer/adapter training trong V8.

Independent receipt `.artifacts/qlora-determinism-result-v8/independent-audit.json` SHA-256 `1d5de34ed352807a5a1abb1bf558a3a4b9f4001312673cf5ea0d67aa9a76ad4a`; source và six saved derivative hashes đã đối chiếu. Handoff activity objects vẫn do parent quản lý và không bị docs update thay đổi.

## GGUF↔HF actual-template probe: attempt1 preflight, corrected2B pass

Attempt1 dùng owned CPU-only/no-warmup template/tokenize server, không GPU/generation/model-forward inference. Native/model/source hash pins khớp; script dừng ở `build_info` substring assumption (hardcoded build-number expectation), **0 corpus examples được compare**. Một preflight tokenization request không phải corpus parity. Owned server exited/key removed được ghi trong report; không công bố key hoặc private output.

`.artifacts/lifemate-gguf-hf-v2-attempt1/report.json` SHA-256 `13f3aec2bdd132b520e05b911159184e2e1de22cb3b11eec2f4a84977de8f319`, script SHA-256 `2137cca8b6cf7922170b159b4fd703a36d2a3f04581b6fb3af3aba9b6d02a0ef`. Đây là probe-preflight failure, không bằng chứng actual template/token parity mismatch của model. **Corrected probe2 đã complete**, independent local collection/audit pass: 3.861 unique IDs/records (3.377 train/484 validation),0discrepancies/truncation. Cả 8 counters cho prefix/full rendered bytes, raw-vocab token IDs, pipeline/raw và runtime-add-special token IDs đều 3.861; input/target hashes và lengths khớp pinned HF reference. Controller/server process absence và auth-dir removal được kiểm tra độc lập. Đây là formatting/tokenization parity của **Qwen3.5-2B** GGUF SHA `aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223`, không tự mở rộng sang Qwen3-4B/template khác.

Report `.artifacts/lifemate-gguf-hf-v2-result/report.json` SHA-256 `bd627bdccca31eddec61e86b17984ab875b0748bcfea362afda561351ea8f968`; independent audit SHA-256 `42ba3ab907c917e4af39162f8d150b9ddba1c02c5f0d416da188bdfba2caac83`. Five collected-file hashes đã đối chiếu local/remote receipt; examples file hash `48579bc943602fe8a81310d2b33f941e4b53d6a64f7bd625c5c1c7baeaa21a50`. Source/HF pins giữ nguyên,0 generation/external-network requests, không model-forward inference/SDK/training/accuracy test. Agent-turn error trước đó không được dùng như process-terminal evidence; actual cleanup/status dựa trên receipts.

2B gate này đã pass trong tested non-native typed/textual-catalog profile. Full-corpus Rust-export parity,4B actorV2 full-corpus GGUF parity, actual training-mask application, numerical/model-conversion behavior và task quality vẫn là gates riêng. Active GPU/other-remote-activity objects được giữ nguyên bởi docs update.

## Runtime-matched actor dataset V2

V2 là artifact supervision riêng theo **Qwen3-4B Instruct `compact-typed/full-catalog`**, không train native xLAM/progressive/Plan DSL. Đây là lựa chọn contract theo baseline hiện có 8,3333%, chưa quality promotion hoặc chứng minh SFT fit.

| Scope | Train | Validation |
|---|---:|---:|
| Assigned evaluation/sample IDs | 1.082 | 153 |
| Actor source IDs sau quarantine | 1.082 | 152 |
| Actor records | 3.377 | 484 |

Outer holdout 310 bị loại khỏi model examples, 30 baseline IDs chỉ validation. `train_1696` giữ assignment/evaluation index, quarantine toàn bộ actor trajectory do result `ok=true,done=false`; không sửa gold/scorer hoặc evaluation coverage. Ask suspends không có target complete về sau; done=false không tạo completion target.

Mỗi 3861 record qua future-poisoning test và lặp lại khi readback; 6 structural validation tests pass. Full permitted catalog được giữ, current/future targets/results/final-state không chọn input schemas. `target_text` giữ đúng thứ tự JSON/runtime contract. Message parity kiểm tra 115 messages (55 observations) từ 30 actual traces dùng supplied cùng ASR, byte-exact; chưa phải full-corpus Rust-export parity hoặc model/tokenizer parity.

Exact **HF tokenizer V2 đã hoàn tất 3.861 actor records/model**, không truncation/exclusions, full input/target hash readback và prefix/token/EOS/JSON-order/roundtrip/completion-span metadata checks. Một CPU worker, không model weights/model class/inference/GPU/SDK/training/network. Collection còn đối chiếu độc lập coverage/hash; 7 pure format tests không phải model benchmark.

| Tokenizer | Prompt median /p95 /max | Full sequence median /p95 /max | Prompt+768 fits 6.144 |
|---|---|---|---:|
| Qwen3.5-2B | 2.389/3.750/5.166 | 2.417/3.774/5.177 | 3.861/3.861 |
| Qwen3-4B Instruct | 2.443/3.848/5.334 | 2.471/3.882/5.345 | 3.861/3.861 |

4B margin chỉ **42 tokens** sau 768 reserve ở 6.144. Số fit này áp dụng pinned HF template/messages; **actual GGUF–HF rendered-prefix parity đã pass riêng cho 2B**; 4B actorV2fullcorpus vẫn chưa probe (historical-prefix75/arm là scope riêng đã kiểm tra), chưa full-corpus Rust-export parity, actual training-mask application hoặc VRAM/training/quality claim. Không đổi runtime context từ riêng tokenizer evidence. V1 report 5096 là corpus/messages khác, giữ lịch sử riêng.

Report `.artifacts/lifemate-tokenizer-pilot-v2/report.json` SHA-256 `2badee05b950e0c53d8ad16f64095941c247e6f5cb33982f4518afda6557b998`; collection receipt `c50a0f79268c71c5d47bcaad46134061fb0c2c37403195f398edc39ed1b42038`. Preparation timings 56,13/57,72s là tokenization CPU, không model throughput.

Manifest `.artifacts/lifemate-sft-v2-runtime/manifest.json` SHA-256 `9a87a6312389a155d8458c43f7f5d4dbcd215f782493b64dbfb790e9860f2db0`; validation 6 tests `.artifacts/lifemate-sft-v2-validation.json` SHA-256 `6b407fc708df38b7762c7ae85357c9d9a8c03e41a679ec4add329950e9d2e73f`; contract riêng `.artifacts/lifemate-sft-contract-v2.md`. Artifacts có labels/results private, public docs chỉ aggregates.

## Hướng ưu tiên: chọn công cụ rồi thực thi kế hoạch có trạng thái

Luồng đề xuất:

```text
Audio → ASR → chọn nhóm công cụ liên quan
      → bổ sung công cụ đọc phụ thuộc từ schema/x-ref
      → LLM tạo kế hoạch có kiểu và tham số
      → Rust theo dõi điều kiện, dữ liệu đã đọc và mục tiêu còn lại
      → SDK ToolEnv nguyên trạng → receipt thật
```

Thử nghiệm 2B đối chiếu profile model/tensor placement. Các bước chọn công cụ và thực thi kế hoạch mới đo thay đổi luồng xử lý. Có thể thử chính LLM hiện tại để chọn nhóm công cụ, hoặc dùng retrieval chỉ để xếp hạng catalog; router từ vựng đã đo chưa đủ recall để loại tool. Chưa cần thêm một LLM resident. Nếu thêm model router, phải đếm cả model đó trong giới hạn tổng parameters. Chẳng hạn 4B + router 1,5B + ASR 809M đã khoảng 6,309B, vượt trần 6B ngay theo cách tính làm tròn này.

Router chỉ chọn trong catalog được phép của mẫu. Sau khi chọn tool, bổ sung các bước đọc cần thiết để lấy ID thật; đo recall của các công cụ cần dùng để phát hiện việc router bỏ sót tác vụ. Nếu chưa chắc chắn, cần một đường mở rộng catalog có giới hạn, đồng thời ghi lại chi phí thời gian của fallback.

Kế hoạch có kiểu nên biểu diễn những quan hệ mà JSON tool call hiện tại chưa bảo đảm:

- Điều kiện phải được kiểm tra trước hiệu ứng phụ thuộc; điều kiện sai đóng đúng nhánh đó.
- Dời một sự kiện giữ nguyên thời lượng, trừ khi người dùng yêu cầu đổi thời lượng. Thời gian mới được tính từ sự kiện đã đọc qua SDK.
- ID phải đến từ đúng loại bản ghi và đúng đối tượng cần chọn. Kết quả tìm kiếm nhiều người cùng tên chưa đủ để tự quyết định người nhận.
- Yêu cầu bị phủ định hoặc rút lại không tạo hiệu ứng; các yêu cầu độc lập còn lại vẫn được theo dõi.
- Hoàn tất chỉ khi mọi mục tiêu đã thực hiện, bị hủy rõ ràng, ngoài phạm vi hoặc đang chờ thông tin cần thiết. Danh sách công cụ rỗng tự nó chưa chứng minh hoàn tất.

Luồng typed plan đã được tích hợp thử nghiệm và đo smoke v25, nhưng cả 3 mẫu đều failed. Prototype `plan.rs` và synthetic v23 vẫn có phạm vi kiểm chứng riêng; chưa chứng minh chất lượng NORA end-to-end. Rust sẽ thực thi lời gọi qua SDK và ghi đúng các lời gọi đã chạy; không đọc state/environment trực tiếp, không tự tạo hash, không sửa kết quả sau khi chấm và không gắn câu trả lời vào ID của mẫu.

[ToolLLM](https://arxiv.org/abs/2307.16789) là bằng chứng nghiên cứu cho hướng kết hợp retriever API với huấn luyện dùng công cụ: nhóm tác giả xây dữ liệu trên 16.464 API, dùng retriever chọn API và đánh giá khả năng thực thi, tổng quát hóa sang API chưa thấy. Kết quả đó được đo trên mô hình và benchmark của bài báo. Việc áp dụng một luồng tương tự cho NORA 2B là giả thuyết cần đo trên LifeMate.

## Ma trận thử nghiệm

| Mốc | Thay đổi được kiểm tra | Trạng thái | Điều cần kết luận |
|---|---|---|---|
| R0 — v19 | Dense 4B, compact typed, workflow hiện tại | Đã đo 30 mẫu: 8,3333% | Baseline có receipt, thời gian và tài nguyên |
| R1 — Qwen3.5-2B | Cùng ASR/workflow; CPU output off khác R0 | Đã đo 30 mẫu: 3,0000%, 15 lỗi; loại profile | Chưa quy nguyên nhân tài nguyên hoặc kết luận mọi 2B |
| R1b — cache control v20 | Cùng 2B, cache RAM 0 và checkpoints 4 | 30 mẫu: 3,0000%, 15 lỗi; output giống R1, RAM giảm | Hai knobs chưa được ablate riêng; không chứng nhận hard cap |
| R2 — lexical routing | Chọn tool và mở rộng phụ thuộc | Audit xong; loại hard filter, gỡ mode tích hợp | Recall 83,5943% reference / 80,8824% ASR, chưa đủ để loại tool |
| R2b — semantic routing v22 | LLM chọn công cụ trước planner | Recall 52/68 = 76,4706%; loại hard filter | Audit 30 development, không SDK; chưa là task accuracy |
| R3 — PlanTyped v23 | LLM tạo một plan, executor chạy hai nhánh synthetic | 2B/4B GPU pass; 62 Linux tests pass | Không dataset/ASR/SDK hoặc task accuracy |
| R3c — runner v25 | Opt-in `plan-typed-experimental` | 3 audio: 0%, 3 failed; không mở 30 | Receipt integrity pass, chất lượng chưa đạt |
| R3d — xLAM progressive/full v26 | Cùng 3 mẫu, full-index progressive vs full schema | Cả hai 0%, 3 failed; không mở 30 | Chưa có quality gain; timing confounds còn mở |
| R4a — tokenizer/capacity | Exact template tokenization rồi capacity pilot | 5.096 sequences/model đã đo; chưa train | 8.192 token length fit không đồng nghĩa VRAM fit |
| R3b — xLAM v21 | Adapter/tool parser chuyên function calling | Synthetic call/result/done pass; 3/3 audio probe lỗi | Sửa context/deadline trước lượt 30; chưa có điểm chất lượng |
| R4 — SFT pilot | Adapter học các lượt hành động từ development train | Chưa train | Có cải thiện trên development tách riêng, với chi phí phần cứng chấp nhận được không |

Để so sánh model, ưu tiên giữ cùng corpus, ASR/beam, context 6.144, deadline 30 giây, SDK, metric và chính sách đặt tensor. Nếu thay context hoặc cách chia CPU/GPU để chạy vừa máy, ghi đó là so sánh profile tổng thể. Đo thêm profile tối ưu tài nguyên sau khi có phép đối chứng.

Mỗi mốc cần báo điểm local, số trạng thái khớp, số mẫu hoàn hảo, call thừa/thiếu/sai tham số, timeout, token prompt, thời gian ASR/LLM, RSS/PSS và VRAM toàn tiến trình. Với router, báo cả recall của tool cần thiết; chỉ số này không thay điểm tác vụ. Diagnostic transcript chuẩn dùng để tách lỗi planner khỏi ASR và phải được đánh dấu rõ.

Đầu tiên chạy cùng 30 development để đối chiếu lỗi và tính khả thi. Sau đó dùng một phần development chưa dùng để lựa chọn cấu hình, mở rộng tập đánh giá, freeze model/prompt/runtime rồi mới đánh giá holdout 310 mẫu. Không dùng holdout hoặc public/private gold để tạo retrieval examples, train hay tune. Điểm công khai cần đủ 634 kết quả thật và lượt chấm của BTC.

## Candidate models và provenance

**Qwen3.5-2B có giấy phép phù hợp để tiếp tục nghiên cứu; profile R1 hiện đã bị loại sau phép đo trên NORA.** [Model card chính thức](https://huggingface.co/Qwen/Qwen3.5-2B) ghi Apache-2.0, language model 2B, 24 lớp và kiến trúc lai Gated DeltaNet/attention. Card báo BFCL-v4 thinking 43,6 và TAU2-Bench 48,8; cột Qwen3-4B-2507 tương ứng là 39,9 và 43,2. Đây là số do nhà phát hành báo cáo, không phải kết quả tiếng Việt, LifeMate hoặc GTX 1650 của NORA. Card cảnh báo 2B có thể lặp trong thinking mode. Bắt đầu bằng non-thinking có budget rõ ràng, rồi đo thinking như một profile riêng nếu có lý do.

Official base revision: `15852e8c16360a2fea060d615a32b45270f8a8fc`; metadata HF ghi sửa cuối ngày 02/03/2026. File GGUF dưới đây là bản chuyển đổi của Unsloth, cần giữ riêng provenance của base model và artifact chuyển đổi.

| Artifact | Giá trị pin |
|---|---|
| Repository | `unsloth/Qwen3.5-2B-GGUF` |
| Revision | `f6d5376be1edb4d416d56da11e5397a961aca8ae` |
| Filename | `Qwen3.5-2B-Q4_K_M.gguf` |
| Bytes | `1280835840` |
| LFS SHA-256 | `aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223` |
| Metadata sửa cuối | 02/03/2026 |

Nguồn artifact: [metadata GGUF tại revision đã pin](https://huggingface.co/api/models/unsloth/Qwen3.5-2B-GGUF/revision/f6d5376be1edb4d416d56da11e5397a961aca8ae?blobs=true). Kích thước file 1,28 GB không phải tổng VRAM/RAM khi chạy; cần đo cả ASR, cache, buffers và runtime. Đếm parameters thực của toàn bộ model được đóng gói, kể cả adapter/model phụ, trước khi xác nhận giới hạn thi.

**xLAM-2-3B là một đối chứng nghiên cứu chuyên tool-use.** [Salesforce model card](https://huggingface.co/Salesforce/xLAM-2-3b-fc-r) mô tả huấn luyện multi-turn/function calling, có tag English và giấy phép CC-BY-NC-4.0, kèm phạm vi research-only. Vì vậy không đề xuất làm default thương mại cho NORA. Nếu điều kiện track cho phép, có thể dùng làm diagnostic để xem phần huấn luyện chuyên công cụ có giúp corpus tiếng Việt này hay không; phải kiểm tra đúng chat template/parser và benchmark thực trước khi chọn.

| Artifact | Giá trị pin |
|---|---|
| Repository chính thức | `Salesforce/xLAM-2-3b-fc-r-gguf` |
| Revision | `a40e82c22c3f2b40410186d56c76e3005a1d4813` |
| Filename | `xLAM-2-3B-fc-r-Q4_K_M.gguf` |
| Bytes | `1929902656` |
| LFS SHA-256 | `bd1a0480eadf5eddced7159e4b7aa68cc3cb47715d7410c99d3b4c485e47ae43` |
| Ngày tồn tại weights | Upload ngày 07/04/2025; repo sửa cuối 06/05/2025 |

Nguồn: [metadata revision](https://huggingface.co/api/models/Salesforce/xLAM-2-3b-fc-r-gguf/revision/a40e82c22c3f2b40410186d56c76e3005a1d4813?blobs=true), [lịch sử chính thức](https://huggingface.co/Salesforce/xLAM-2-3b-fc-r-gguf/commits/main). Hai artifact candidate đều có metadata trước cutoff 26/10/2026; việc dùng vẫn phụ thuộc điều kiện giấy phép của track. Không suy ra xLAM có chất lượng tiếng Việt cao chỉ từ benchmark function calling của nó.

## Khi nào chuyển sang SFT

Nếu R1–R3 vẫn yếu trong diagnostic transcript chuẩn, ưu tiên SFT hành động theo miền thay vì tiếp tục chỉnh prompt không giới hạn. Tạo các lượt training từ **development train**: context và catalog hiện tại → gold call tiếp theo → kết quả SDK đã thực thi → lượt tiếp theo. Mỗi target chỉ thấy dữ liệu có trước nó; ID từ một tool result tương lai không được đưa ngược vào input. Dùng input/output format giống runtime dự định triển khai.

[TRL SFT](https://huggingface.co/docs/trl/sft_trainer) hỗ trợ hội thoại có tool calls/results, cột `tools` và loss chỉ trên phần assistant. [PEFT quantization](https://huggingface.co/docs/peft/main/developer_guides/quantization) hỗ trợ LoRA trên base model đã quantize. Có thể thử QLoRA NF4, batch size 1, rank nhỏ, gradient checkpointing và [activation offloading](https://huggingface.co/docs/trl/reducing_memory_usage). Pin phiên bản thư viện trước khi chạy; các tùy chọn trong tài liệu main có thể thay đổi.

GTX 1650 có compute capability 7.5; [bitsandbytes](https://huggingface.co/docs/bitsandbytes/main/en/installation) ghi NF4/FP4 yêu cầu từ 6.0. Điều đó chỉ xác nhận khả năng phần cứng, không đảm bảo một lượt train vừa 4 GB. Trọng số quantized, embedding không quantize, activations, logits, gradients, optimizer và CUDA đều cần bộ nhớ. Dừng các model inference do NORA sở hữu trước pilot; giữ đủ RAM máy cho offloading.

Pilot nên đo khoảng 20 bước với dữ liệu hoàn chỉnh, lưu peak memory và thời gian thực để ước lượng lượt train dài. Nếu không vừa, giảm phạm vi adapter/model hoặc rút gọn catalog và ngữ cảnh theo cấu trúc có kiểm chứng; không cắt mất thông tin cần thiết để ép sequence vừa VRAM. A30/64 GB là môi trường thi cuối được phép dùng, chưa phải máy train đang truy cập được. Chưa có cơ sở hứa số giờ train hoặc độ chính xác 99%.

Sau train, merge/convert/quantize theo pipeline đã pin và đánh giá lại trên development tách riêng rồi holdout sau freeze. Loss giảm và khớp teacher-forced targets không thay thế kiểm thử ASR → planner → SDK đầy đủ.


## Bằng chứng bổ sung cho kế hoạch và executor

[ReWOO](https://arxiv.org/abs/2305.18323) tách bước lập kế hoạch khỏi việc liên tục đưa toàn bộ observations trở lại LLM. Tác giả báo cáo hiệu quả token tăng 5 lần và accuracy tăng 4 điểm phần trăm trên HotpotQA. Kết quả đó hỗ trợ việc thử giảm số lượt LLM, nhưng không đo LifeMate tiếng Việt, model 2B hoặc GTX 1650.

[LLMCompiler](https://arxiv.org/abs/2312.04511) dùng planner, bộ điều phối tác vụ và executor theo đồ thị phụ thuộc. Trong các tác vụ của bài báo, tác giả báo cáo mức tăng tốc latency tối đa 3,7 lần, giảm chi phí tối đa 6,7 lần và tăng accuracy tới khoảng 9 điểm phần trăm so với ReAct. Đây là số liệu bên ngoài, không phải kết quả NORA. SDK LifeMate vẫn cần một môi trường có thẩm quyền và ledger theo thứ tự thực thi; thử nghiệm NORA đầu tiên ưu tiên giảm lượt model và kiểm tra điều kiện bằng Rust, chưa giả định gọi SDK song song là an toàn hoặc nhanh hơn.

Sau snapshot v19, cache control v20 đã giảm RAM nhưng giữ nguyên điểm và output của hai lượt đã đo. Router từ vựng tiếp tục bị loại khỏi hard filter. Source v21 đã qua tests/parser checks và synthetic adapter cycle; probe audio xLAM còn lỗi, typed executor chưa nối vào LLM. Semantic audit v22 cũng đã hoàn tất và hard filter bị loại. Probe PlanTyped v23 pass synthetic nhưng runner v25 thất bại trên 3 audio. Hai profile v26 cũng failed. V27 native controls cũng đã chấm0%; capacity v1 dừng ở guard và v2 dừng ở synthetic gradient check trước optimizer step. V3/V4 đã chẩn đoán scale-sensitive NaNs nhưng strict parity vẫn fail/0 steps; actor dataset V2 đã structural/message/HF-tokenizer validation, còn GGUF/template/training gates. V5 matched controls cũng fail strict cross-mode; V6 ordinary backward dừng ở unchanged headroom guard, chưa tới aligned paths; V7 selected-first partial6/12 và strict short-repeat failures; GGUF probe1 dừng preflight, corrected probe2 đã independent pass chỉ cho2B formatting/tokenization, scope riêng. Không gán kết quả v19–v27 hoặc observation parity/0 steps thành training capacity/model accuracy.

Các bước tiếp theo cần hoàn tất matrix sau V9 partial pass, rồi kiểm tra ordinary full-head và full-example gradient/update/save gates riêng; chưa có training hoặc accuracy result. Giữ 2B formatting proof tách phạm vi với 4B full-corpus, numerical, training và task gates. V27 controls 0%, smoke 6.144, paired30 v28 và alias paired30 v29 được giữ. Fixed few-shot và descriptive alias profiles đã bị loại theo gates riêng; mọi failed samples vẫn có trong đánh giá. Tokenization và observation parity chưa thay cho capacity measurement hoặc model accuracy. Dataset, scorer và membership không đổi. Holdout 310, public 634, A30/Docker, checkpoint 99% và mục tiêu formal 100% vẫn chưa đạt; chưa commit/push hoặc nộp entry NORA.


## V14 và validation rộng v32

V14 sustained QLoRA hoàn tất3updates rồi guard dừng backward2655: GPUfree530.579.456bytes, thấp hơn512MiB; owner/supervisor/group đã hết, ownswap0, không adapter save. Evidence `.artifacts/qlora-sustained-v14-result/` đã audit/readback7files; không coi là train đủ. V32 có Rust ID-only SHA-pinned cohort, giữ source order/model messages, cấm start/limit và public/private subset. Windows package109pass và Linuxwhisper release alltargets111pass,0fail/3ignored; fmt/Clippy/build Linux qua. Đang đo toàn123validation ngoài benchmark30 với400audio khác nhau bằng native unadapted2B; chưa có điểm hoàn chỉnh.

Hypothesis V32-B đã freeze trong lúc A chạy, trước khi biết điểm A: dùng cùng mọi input/source/asset/profile, chỉ --cpu-output on→off. A còn gần1,8GiB GPUfree nên kiểm tra GPU output placement có thể giảm latency; chưa đo B và chưa đổi default. A phải đủ123 và qua exact SDK audit trước B. V15 training table-offload là pilot riêng,8CPUpreflight checks qua; full945 GPU parity phải đạt trước7updates, chưa GPU launch. Mục tiêu100%/main gate vẫn giữ.


## Kết quả hoàn tất v32 / V15–V16

Hai arm123validation đều2,9810%,failures79/78,nonempty-perfect2/2; no placement promotion. V16 tối ưu CPUtable/cache qua6updates rồi guard dừngfull5177; không fullSFT/qualitygain. Codex đã ghi123open-coded diagnosticcards và sixproxygraders; driftbaseline còn thiếu. [Báo cáo cập nhật](../development/lifemate-error-review-2026-11-03.md) ghi số liệu, hashes và giới hạn. GT171–174; chưa99/100, publicsubmission hoặc maincommit.
