# LifeMate: nối adapter đã học vào CLI audio

Ngày 04/11/2026. CLI audio trước đây chỉ gọi `LocalModel::start`, nên không thể
nạp LoRA dù công cụ chẩn đoán text đã hỗ trợ. CLI cũng chưa truyền cấu hình
prefill được dùng trong thử nghiệm dung lượng. Đã bổ sung:

```text
--llm-mode actor-v2-experimental
--lora /path/actor.gguf --lora-sha256 <SHA256>
--prefill-batch 128 --prefill-microbatch 32
```

Hai tùy chọn adapter phải có cùng nhau; batch/microbatch cũng phải có cùng nhau.
Hash, kích thước và header GGUF v3 được kiểm tra trước khi tạo tiến trình model
hoặc file đo bộ nhớ. Hàm kiểm tra asset được dùng chung giữa CLI audio và công
cụ chẩn đoán text. Header/hash xác nhận file, chưa chứng nhận tương thích giữa
base model và adapter. Cấu hình mặc định vẫn chạy không có adapter.

Kiểm chứng Windows: 142 kiểm thử package và 7 kiểm thử công cụ text qua; 3 kiểm
thử vốn được đánh dấu bỏ qua vẫn được giữ nguyên. Clippy với warnings-as-errors
và kiểm tra định dạng qua. Các ca CLI bao gồm thiếu cặp tùy chọn, sai hash,
batch không phải lũy thừa hai và microbatch lớn hơn batch. Các ca bị từ chối
không tạo model, submission, metrics hoặc file đo bộ nhớ.

Build đầu thiếu libclang. Thử bindings đi kèm sau đó thất bại vì layout kiểu C
của Linux không phù hợp Windows; cả hai log thất bại được giữ lại. Lượt qua dùng
bindings được sinh bằng bản portable [libclang 18.1.1](https://pypi.org/project/libclang/18.1.1/),
wheel SHA256 `4dd2d3b82fab35e2bf9ca717d7b63ac990a3519c7e312f19fa8e86dcc712f7fb`,
DLL SHA256 `b13ec96d9a036d6c6c36dd09e00415ead7acc7fcfd40e721bf33ea86b3490f9d`.
Dependency chỉ phục vụ build, nằm dưới `.tools`, có license và receipt; không
cài toàn hệ thống. `Build-Rust.ps1` có thể dùng bản portable này nếu đã hiện diện
và đúng hash, không tự tải về.

Receipt: `.artifacts/audio-adapter-cli-v1-result.json`. Đây chưa phải kiểm thử
model/ASR thực, xác nhận chất lượng hay benchmark A30. Source mới cần snapshot,
build Linux và lượt audio sau khi chuỗi huấn luyện/R20 đang chạy kết thúc. R19
và các script R20 đã đóng băng không bị thay đổi. Docker prototype vẫn dùng
checkpoint cũ; chưa được chứng nhận hoặc chuyển thành gói nộp của checkpoint mới.


R21 source staging (04/11):143 source files are verified on lattice at
`/home/koi/nora-lifemate/source-lifemate-v2-r21`. Manifest
`537338702f1a8c598195c5b8d6c1e51bfb31f4d53f432f44537b67a747d9a9b5`.
Only four existing R19 files changed plus the shared assets.rs helper. No Linux
build or model execution yet; R19/R20 stay frozen. Stage receipt:
`.artifacts/lifemate-v2-r21/staging-receipt.json`. Do not re-run fresh-only staging.


## 2026-11-04: lượt build Linux sau R20 đã được chuẩn bị

Đã chuyển helper build sang lattice, chưa chạy compiler hay model. Helper chỉ
nhận R20 khi cả hai arm có receipt hoàn tất, đã dọn tiến trình và không còn PID
hoặc process group; sau đó giữ khóa chung trong suốt build. Scope bắt buộc 2 GiB
RAM, không swap, tối đa hai core, nice 15 và ẩn GPU; lệnh Cargo chạy offline,
locked, một job. Cache R19 được sao chép độc lập, không dùng hardlink. Bản đối
chứng, source R20 và job huấn luyện đang chạy không bị thay đổi.

Bốn kiểm thử admission bằng metadata tạm qua: receipt thiếu/đang chạy, PID hoặc
process group còn sống, cặp terminal hợp lệ, và giới hạn RAM/swap/CPU không đúng.
Đây chưa phải build Linux hoặc điểm model. Plan mode đã chạy trên lattice và
xác nhận không khởi chạy build. Lệnh thực thi và hash helper được lưu tại
`.artifacts/lifemate-v2-r21/build-plan-v1.json`; chỉ chạy sau khi bộ điều khiển
huấn luyện và R20 kết thúc, không tự động lên lịch thêm job.
