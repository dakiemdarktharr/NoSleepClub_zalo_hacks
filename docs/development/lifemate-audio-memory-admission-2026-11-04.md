# LifeMate: cần kiểm chứng riêng bộ nhớ của đường audio có LoRA

Ngày 04/11/2026. Đọc source hiện tại và log đã kiểm tra hash cho thấy lượt
native-capacity-v4 chỉ kiểm LLM với adapter có cùng hình dạng. Nó không nạp ASR.
CLI audio lại giữ LLM sống rồi khởi tạo Whisper, nên hai model cùng chiếm GPU.

Lượt LLM-only đó còn ít nhất **274 MiB VRAM**. Log audio R2, dùng đúng bản
Whisper large-v3-turbo Q5 và native executable đã pin, ghi riêng trọng số GPU
**573,45 MB**; còn có KV cache và compute buffers. Vì vậy không được dùng kết
quả LLM-only để tuyên bố cấu hình audio 16K + LoRA vừa GTX 1650. Đây là suy luận
từ các lần đo riêng, chưa phải một lượt audio mới đã thất bại.

R2 đã chạy đồng thời GPU ASR và LLM với context 8192, `--cpu-output on`, không
có LoRA đã học; mức GPU trống thấp nhất là 415236096 byte. Kết quả đó cũng không
chứng minh cấu hình 16384 + LoRA mới đủ bộ nhớ. Những lần audio cũ không đạt
chất lượng theo hạn 30 giây vẫn được giữ nguyên.

## Thay đổi bước nghiệm thu kế tiếp

Sau khi chuỗi huấn luyện/R20 kết thúc và R21 build Linux qua, cần đo **cả ASR,
LLM, adapter, KV cache và phần bộ nhớ tăng lúc prefill** trước lượt audio rộng.
Ứng viên đầu tiên để đo là context 16384, batch128/microbatch32, CUDA Graphs của
LLM tắt, các transformer layer trên GPU, `--cpu-output on`, GPU ASR cùng model
và beam1. Tùy chọn CPU output hiện có chuyển cả `output.weight` và
`token_embd.weight` sang CPU; nó có thể ảnh hưởng độ trễ và phải được đo.

Ứng viên này chưa được chạy hoặc chứng nhận. Nếu không qua, giữ nguyên bằng
chứng thất bại rồi xem xét cấu hình có ít layer trên GPU hơn trong một lượt
thử riêng. Không tự fallback, cắt history, giảm context để che lỗi, nới guard
hay đổi bản đối chứng giữa chừng. Base và adapter phải dùng cùng cấu hình
audio trong lượt so sánh. R20 text đã cố định vẫn giữ nguyên.

Đường ASR CPU hiện có là phương án chẩn đoán riêng, không được coi tương đương
về thời gian với GPU ASR hoặc dùng để suy ra máy A30 sẽ đạt 30 giây. Trên A30,
cấu hình cùng GPU cho ASR/LLM vẫn cần benchmark thật. Không đổi mục tiêu chất
lượng và không biến kết quả kiểm bộ nhớ thành điểm cuộc thi.

## Bằng chứng

- `.artifacts/audio-capacity-review-v1.json`: source pins, archive/log pins,
  số đo và cấu hình ứng viên chưa chạy.
- `.artifacts/native-lora-context-v4/independent-audit.json`: LLM-only;
  SHA256 `812456115b2612890d0f1688f17c746723069b23bc4c4edafa9570676c5d1447`.
- `.artifacts/lifemate-v2-r2/audio-control-result.zip`: archive thực;
  SHA256 `dc12f473dbd9aac9923a84fe95ba7974e8bb4e1370cfca6a7ae6b2712ea68e22`.
- `crates/nora-lifemate/src/main.rs`: vòng đời LLM và ASR cùng tồn tại.
- `crates/nora-lifemate/src/local.rs`: CPU output placement.
- `crates/nora-audio/src/whisper.rs`: đường Whisper CPU đặt `use_gpu(false)`.

Lượt rà soát này không chạy model, không đọc nhãn/public/private environment,
không đổi source đã đóng băng hoặc tiến trình đang chạy, không tạo điểm mới.


Đã viết và đưa source của phép thử combined-residency sang lattice tại
`.tools/native-audio-lora-capacity-v1/`. Rust metadata/type-check trên Windows
qua với warnings-as-errors; không link hoặc chạy model. Probe giữ owner ASR
qua các lượt LLM short/long ở cả base và adapter; lượt long phải báo12k–15k
prompttokens. ASR chỉ warmup silence, chưa kiểm audio thực. Source/hash và phạm
vi nằm trong `.artifacts/native-audio-lora-capacity-v1/preparation.json` cùng
`staging-receipt.json`. Còn phải link với R21 Linux và chuẩn bị supervisor đo
cả hai native process/guard/cleanup sauR20; chưa có kết quả fit hay điểm chất lượng.

Đã chuẩn bị và stage `native-audio-lora-capacity-v1/build.py` để link probe sau
khi R21 Linux build và hai lượt R20 kết thúc. Builder yêu cầu lock chung,
CPU tối đa hai core, nice15, RAM2GiB/swap0 và host còn ít nhất4GiB; compiler
chạy trong nhóm tiến trình được theo dõi và thu dọn. Nó build riêng thư viện
R21 trước khi link, vì receipt R21 hiện pin binary ứng dụng nhưng chưa pin
root rlib được sao chép từ cache. Source, binary và log kiểm thử R21 được
đối chiếu trước/sau. Sáu kiểm thử metadata trên Windows qua; plan mode đã
chạy trên lattice và không khởi động compiler/model. Receipt:
`.artifacts/native-audio-lora-capacity-v1/build-preparation.json`.
Builder đã stage, chưa execute; supervisor cho lượt inference kết hợp vẫn
chưa hoàn thành. Kết quả này không chứng minh đủ VRAM hoặc cải thiện điểm.

Sau đó đã hoàn thiện và stage source supervisor cùng scope worker. Mười
kiểm thử metadata qua trên cả Windows và Linux; chưa chạy GPU/model. Supervisor
ràng buộc đúng chain64/adapter/GGUF, đo hai native process và GPU allocation
trong mỗi lượt long request, giữ giới hạn8GiB/swap0/host4GiB/GPU trống256MiB.
Scope worker lưu memory peak và bộ đếm OOM trước khi cgroup biến mất; thiếu
receipt cuối hoặc thu dọn chưa được chứng minh đều không được công nhận qua.
Mẫu telemetry vẫn là đo theo thời điểm, không phải quan sát GPU liên tục.

Source, hash và phạm vi kiểm chứng nằm trong
`.artifacts/native-audio-lora-capacity-v1/run-preparation.json` và
`run-protocol.md`. Chưa có probe binary Linux, lượt inference kết hợp hay
điểm chất lượng mới. Không tự tạo job cạnh tranh với chuỗi SFT/R20 đang chạy.
