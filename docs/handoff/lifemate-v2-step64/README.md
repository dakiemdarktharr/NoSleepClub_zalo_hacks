# LifeMate V2 — bàn giao checkpoint 64, 05/11/2026

**Đây là model nghiên cứu đã train 64/1.082 tác vụ, chưa phải production và chưa có điểm chất lượng của adapter này.** Người dùng yêu cầu xuất bản bản hiện tại để thành viên khác tiếp tục nghiên cứu; việc xuất bản này thay thế điều kiện chờ 100% đối với riêng bản bàn giao, không thay đổi mục tiêu chất lượng.

[Tải các tệp trong GitHub Release](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/releases/tag/lifemate-v2-step64-research-20261105). Mã nguồn làm việc được lưu trên `main`; tag của release giữ một commit cố định. [manifest.json](manifest.json) ghi SHA256, kích thước, toàn bộ thành phần của các gói và giới hạn bằng chứng.

## Các gói tải về

| Tệp | Nội dung |
|---|---|
| `nora-lifemate-v2-step64-adapter.zip` | PEFT LoRA FP32 `adapter_model.safetensors`, config gốc, model card gốc và license Qwen; khoảng 61,4 MB |
| `nora-lifemate-v2-step64-resume.zip` | `checkpoint-0064/state.pt` và manifest: adapter parameters, optimizer, CPU/CUDA RNG, cursor; khoảng 92,9 MB |
| `nora-lifemate-v2-step64-research-support.zip` | Worker/helpers huấn luyện đã đóng băng, thứ tự dữ liệu, bằng chứng chuỗi checkpoint, bản sửa kiểm tra adapter, scripts đánh giá và chuẩn bị ASR+LoRA; khoảng 9 MB |
| `nora-lifemate-v2-r21-source.zip` | Snapshot Rust R21 nguyên byte đã chuẩn bị trước đó; Linux R21 chưa build |
| `manifest.json`, `SHA256SUMS` | Kiểm tra tính toàn vẹn; không phải điểm chất lượng |

Trọng số base, dataset/audio/labels/environments chính thức và API keys không nằm trong các gói. Tải dữ liệu từ tài khoản cuộc thi được cấp quyền qua [portal LifeMate](https://challenge.zalo.ai/portal/lifemate). Các script lịch sử có đường dẫn tuyệt đối và các hash nghiệm thu của lattice; gói support là hồ sơ nghiên cứu, không phải bộ cài có thể chạy nguyên lệnh trên mọi máy.

```bash
gh release download lifemate-v2-step64-research-20261105 \
  --repo dakiemdarktharr/NoSleepClub_zalo_hacks --dir release-step64
python scripts/verify_lifemate_release.py release-step64
```

Script chỉ đọc/hash các tệp và kiểm archive; không deserialize `state.pt`, không nạp model. Chỉ dùng `state.pt` đã xác minh với loader checkpoint tương ứng và `torch.load(..., weights_only=True)`. Để inference, dùng safetensors trong gói adapter.

## Model và môi trường

Base là [Qwen/Qwen3-4B-Instruct-2507](https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507), revision **`cdbee75f17c01a7cc42f958dc650907174af0554`**. Model card upstream ghi license Apache-2.0. Phải giữ cùng base/tokenizer revision; config adapter gốc có đường dẫn lattice, nên khi nạp trên máy khác hãy truyền base đã tải về một cách tường minh.

- LoRA rank 8, alpha 16, dropout 0; 252 linear modules, 504 tensors, 16.515.072 tham số FP32.
- Training: HF NF4 double quantization, FP16 compute; AdamW8bit, lr `2e-4`, betas `(0.9,0.999)`, eps `1e-8`, decay 0, clip 1; một task/update, seed 42. Có CPU offload/checkpointing và các helper chia tensor để vừa GTX 1650.
- Environment đã dùng: Python 3.12, Torch `2.8.0+cu128`, Transformers `5.13.0`, PEFT `0.21.0`, bitsandbytes `0.50.1`, tokenizers `0.22.2`, safetensors `0.8.0`, NumPy `2.5.4`.
- 64 update theo thứ tự cố định đã hoàn tất: 291.318 sequence tokens và 5.816 target tokens. Đây chưa phải một epoch/full pass của 1.082 tác vụ.
- Lattice: GTX 1650 4 GiB, Ryzen 5 4600H, RAM 16 GB. Training job giữ RAM 8 GiB, swap 0, host available ≥4 GiB, GPU free ≥512 MiB. Desktop NORA vẫn có ngân sách riêng 2 GB + API; cấu hình thi được phép dùng A30/64 GB.

Ví dụ **nạp adapter trên một máy đủ RAM/VRAM**, không phải cấu hình inference đã benchmark trên GTX 1650:

```python
from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import PeftModel

# BASE_DIR là snapshot đầy đủ của revision chính xác ở trên, tải trước khi offline.
BASE_DIR = "/path/to/Qwen3-4B-Instruct-2507-cdbee75f"
tokenizer = AutoTokenizer.from_pretrained(BASE_DIR, local_files_only=True)
base = AutoModelForCausalLM.from_pretrained(
    BASE_DIR, dtype="auto", device_map="auto", local_files_only=True
)
model = PeftModel.from_pretrained(base, "release-step64/adapter", local_files_only=True)
model.eval()
```

Giải nén gói adapter vào `release-step64` trước ví dụ trên. Prompt phải theo `crates/nora-lifemate/src/actor_v2.rs` và packer/training contract. Đoạn nạp này không tái hiện cách quantize/offload khi train và chưa được chạy như một benchmark mới. Không gọi API ở chế độ thi.

## Đã kiểm chứng và chưa kiểm chứng

Chuỗi 64 update qua kiểm tra thứ tự, checkpoint và báo cáo tài nguyên; toàn bộ hash của các tệp tải từ lattice khớp. Kiểm tra CPU thực trên lattice ngày 05/11 đã chứng minh **504/504 tensors trong adapter bằng đúng checkpoint 64**, không nạp base model hay khởi tạo CUDA. Bản kiểm tra cũ V2 lỗi `UnboundLocalError` do `import importlib.metadata` bên trong `main` che tên `importlib` dùng trước đó. Bản V3 đưa import lên module scope và qua trên trọng số thật; giữ log thất bại V2 trong gói support.

**Chưa có:** GGUF của checkpoint 64 đã export/byte-audit, lượt so sánh R20 base/adapter, Linux R21 build, combined ASR+LoRA memory gate, broad audio 153/310, benchmark A30, điểm public NORA hoặc điểm chấm ẩn. Pipeline tự động dừng tại lỗi V2, chưa được khởi động lại. Không dùng adapter capacity hai update cũ thay cho checkpoint này.

Điểm lịch sử **51,0149%** thuộc R14, 12 mẫu reference-text dùng lại, deadline chẩn đoán 180 giây, model/policy khác; **không phải điểm của checkpoint 64**. Với comparator 30 giây, R14 đạt 0/12. Mục tiêu >90%, 99% và 100% đều chưa được xác nhận.

## Việc người tiếp nhận nên làm tiếp

1. Kiểm các gói bằng script trên, đọc `evidence/chain-audit-0064.json`, `evidence/saved-adapter-audit-v3.json` trong gói support và model/training contract trong checkpoint manifest.
2. Hoàn tất export F32 GGUF cho adapter **64 bước** và đối chiếu đúng 504 tensor/scale. Bản export V2 đang chờ receipt tên cũ; cần nối receipt V3 bằng một phiên bản mới có hash riêng, giữ nguyên bằng chứng cũ.
3. Chạy R20 base/adapter tuần tự cùng cấu hình đã cố định, không BM25; giữ nguyên scorer, SDK replay, tất cả failures, full history, context 16K. Protocol R20 trong support mới hơn mục batch256/128 trong protocol SFT lịch sử: dùng batch128/micro32, CUDA Graphs tắt, CPU output off, seed42/temp0.
4. Sau khi review R20: build R21 Linux, chạy phép đo ASR+LoRA cùng resident; chỉ sau đó chạy toàn bộ 153 internal-validation audio dưới deadline 30 giây. Tập 153 có phần đã dùng để tuning, không gọi là blind test. 310 outer là tập riêng; nếu dùng kết quả để sửa model phải ghi nhận việc tái sử dụng.
5. Nếu tiếp tục train trên lattice, resume từ checkpoint 64/cursor64 bằng đúng contract và optimizer/RNG; không chạy lại 64 mẫu đầu. Chỉ một job dùng GPU/lock tại một thời điểm. Sang máy khác cần dựng lại môi trường và điều kiện nghiệm thu phù hợp, không tắt guard để vượt qua hash/path checks.
6. Đo trên A30 thực trước khi kết luận về thời gian chấm. Public Submissions chấm JSONL do nhóm sinh; không phải chỗ cung cấp phiên chạy model A30. Điểm local, public và private phải báo riêng.

Tài liệu đọc tiếp: [kiến trúc V2](../../research/lifemate-v2-architecture.md), [lỗi policy R14](../../development/lifemate-r14-policy-errors-2026-11-04.md), [ASR/LoRA memory gate](../../development/lifemate-audio-memory-admission-2026-11-04.md), [lattice so với A30](../../research/lifemate-lattice-vs-a30-2026-11-04.md).
