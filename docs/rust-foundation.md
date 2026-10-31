# NORA Rust foundation — 01/11/2026

Phạm vi bàn giao: mã nền tảng Rust có thể build/test và chạy local inference thật. Đây là bước chuẩn bị cho việc tích hợp sản phẩm; chưa phải ứng dụng desktop production hoặc bằng chứng toàn bộ NORA đạt 2 GB RAM.

## Đã xây

| Crate | Khả năng đã có | Ranh giới hiện tại |
|---|---|---|
| `nora-core` | Router từ typed task facts; chính sách voice/card/silent; chống báo lặp; checked resource reservations; capability/egress types; fingerprint approval | Chưa có NLP classifier, LLM client, OS enforcement hoặc xác thực producer |
| `nora-audio` | PCM ring và framing có giới hạn; resampling Rubato; endpointing; playback generation; Silero ONNX host; whisper.cpp host; CPAL capture tùy chọn | Capture phải được caller bật; chưa nối mic → ASR → agent → speaker, chưa có AEC hoặc speaker authentication |
| `nora-vieneu` | Port host VieNeu v3 Turbo sang Rust: bounded NPZ/config loader, tokenizer/prompt, speaker projection, tied heads, sampling, KV/codec streaming, preset voices; native sea-g2p | FP32 CPU đã chạy; chưa đủ toàn bộ SDK features, voice quality corpus hoặc INT8 profile |
| `nora-store` | SQLite goals/checkpoints/revisions; effect intent/dispatch/verified/reconciliation; persistent notification acknowledgements | Local trusted supervisor; không tạo external exactly-once, OS isolation hay caller authentication |
| `nora-cli` | Replay, routing, manifest tạo/verify, scoped note có write/readback thật, PCM probe, real VAD/TTS/ASR commands | Công cụ chẩn đoán offline, chưa phải tray app/permission broker |

First-party crates đều `forbid(unsafe_code)`. Các native dependencies/bindings vẫn có unsafe/FFI bên trong; quy tắc này không phải chứng nhận toàn bộ dependency graph memory-safe.

Python không nằm trong Rust runtime. Reference policy Python được giữ làm oracle; script Python so sánh waveform chỉ dùng offline khi phát triển. Không khởi tạo PyTorch, Python worker, LLM local hay process riêng cho từng agent. Workers của sản phẩm sau này là scoped logical jobs dùng LLM API và budget chung.

## Phần upstream được giữ

- Silero: model ONNX; Rust tự quản lý context/recurrent buffers và gọi ORT.
- Whisper: `whisper-rs 0.16.0` / `whisper-rs-sys 0.15.0` gọi whisper.cpp. Không viết lại kernel C/C++ để đạt tỷ lệ Rust hình thức.
- VieNeu: host chuyển sang Rust; ONNX graphs, codec, weights và preset assets giữ nguyên. `ort =2.0.0-rc.15`, CPU runtime kiểm tra là ONNX Runtime **1.31.1**.
- G2P: `sea-g2p-rs` pin commit `e825173f235d08ea19315b2b279fb11153b44cea`, tắt default Python bindings. Tokenizer pin `0.22.2`.
- Resampler dùng Rubato `0.16.2`; audio I/O tùy chọn CPAL `0.16.0`; SQLite qua rusqlite `0.41.1`.

Graph/weights không trở thành Rust khi đổi ngôn ngữ host. Viết lại operator hay quantize cần quality/compatibility tests riêng. [Cargo.lock](../Cargo.lock) là dependency snapshot thực tế; [third-party notices](../THIRD_PARTY_NOTICES.md) giữ attribution.

## Build và chạy

Toolchain pin **Rust 1.99.1**, edition 2024; manifest yêu cầu tối thiểu Rust 1.92. Build host cần nhiều tài nguyên hơn bản phân phối; toolchain, C++ compiler và Cargo caches không phải thành phần phải cài trên máy end user.

Windows native path đã kiểm tra: x64 MSVC C++ tools + CMake + Ninja, PowerShell 7. Script chọn VS tools đã cài và target MSVC; không tự cài hay sửa PATH hệ thống.

```powershell
./scripts/Build-Rust.ps1 -Action Test
./scripts/Build-Rust.ps1 -Action Clippy
./scripts/Build-Rust.ps1 -Action Format
./scripts/Build-Rust.ps1 -Action Release

$nora = './target/x86_64-pc-windows-msvc/release/nora.exe'
& $nora replay examples/progress-events.jsonl
& $nora --help
```

Chỉ kiểm tra core, không native model features: `cargo test --locked --workspace` hoặc `./scripts/Build-Rust.ps1 -Action Test -CoreOnly`. Feature `onnx`, `whisper`, `g2p` độc lập; `capture` thuộc `nora-audio`. Các tests mặc định không mở microphone, gọi API trả phí, tải model hay phát âm thanh.

MinGW/GNU đã compile-check một số features, nhưng native Whisper test linking gặp lỗi archive prefix của upstream Win32 CMake. MSVC là Windows native path đã chạy thành công; không có sửa tay generated registry/cache để giấu vấn đề này. Profile Whisper x86 kiểm tra AVX2/F16C/FMA trước native initialization. Máy cũ hơn cần build/profile riêng. Linux CI core không chứng minh Linux native voice đã qua.

Assets do người dùng chọn và verify trước. Không có auto-download hoặc thực thi hub code. Xem [manifest và revision](../assets/manifests/README.md).

```powershell
& $nora verify-pack 'D:/nora-assets/vieneu-fp32' assets/manifests/vieneu-v3-turbo-fp32.json
& $nora vad-smoke 'D:/nora-assets/ort/onnxruntime.dll' 'D:/nora-assets/silero/silero_vad.onnx'
& $nora render-tts 'D:/nora-assets/ort/onnxruntime.dll' 'D:/nora-assets/vieneu-fp32' 'D:/nora-assets/vieneu-fp32/codec' 'D:/nora-assets/g2p/sea_g2p.bin' 'D:/nora-output/new.wav' 'Xin chào, tôi là Nora.' 48 --greedy
```

Output WAV phải chưa tồn tại, float32 mono 48 kHz; CLI không phát loa. TTS lỗi nếu thiếu model/voice, không tạo giọng giả để thay model. ASR command nhận raw float32 little-endian mono 16 kHz, tối đa 60 giây; chưa chạy quality/resource evaluation với Whisper model thật trong bàn giao này.

## Tối ưu đã thực hiện

Buffer PCM/JSON/NPZ và resource arithmetic có bounds trước cấp phát. Model arrays được đọc streaming vào buffer cuối; không deserialize toàn bộ NPZ sang nhiều bản. Tokenizer không bật thread pool song song mặc định cho single-user host. ORT có một caller-owned environment với global pool intra=2/inter=1, spinning tắt; DLL được bind trước mọi ORT object để tránh ambient DLL khác version. KV ownership, sampling buffers và recurrent VAD state được tái sử dụng. Codec streaming/full chỉ load một mode; prepacked container được chia sẻ trong phạm vi engine.

Release dùng `opt-level=3`, thin LTO, một codegen unit và strip symbols. `GGML_NATIVE=OFF` tránh host-specific tuning, nhưng không có nghĩa mọi SIMD upstream bị tắt. Chưa có bằng chứng những lựa chọn này là optimum toàn bộ workload hay mọi laptop; chúng là baseline có thể đo và điều chỉnh.

SQLite giới hạn main database khoảng 64 MB, cache 2 MB và đặt `journal_size_limit` 8 MB. Giá trị journal không phải hard cap cho WAL đang hoạt động khi có reader giữ lâu. Production cần disk/backpressure và WAL/checkpoint monitoring.

## Kết quả thực đo

Máy kiểm tra Windows 11 x64, AMD Ryzen AI 5 340, 6 cores/12 threads, CPU backend. [Receipt JSON](benchmarks/2026-11-01-windows.json) ghi bytes, options, artifact revisions và executable hash. Các phép đo là fresh process, không flush OS file cache; chỉ đo **một process**, không children/GPU.

| Phép đo | Peak working set | Kết quả |
|---|---:|---|
| VieNeu FP32 + native G2P, một câu preset greedy | 1.160.454.144 bytes | Load 4.518 ms; first chunk 408 ms sau load; synthesis 1.120 ms cho 1,68 giây audio; không gồm ASR/UI/browser/LLM hoặc speaker playback |
| Silero thật, 32 frame silence | 46.047.232 bytes | Load 175 ms, inference 6,08 ms; không phải VAD accuracy benchmark |
| PCM ring, không model | 5.275.648 bytes | Chỉ kiểm tra vòng buffer; không đại diện idle RAM của assistant |

So sánh VieNeu với Python reference đã pin source: cùng FP32 graphs, ORT 1.31.1, preset Hải Đăng, câu “Xin chào, tôi là Nora.” và greedy options. Waveform 80.640 samples; kiểm tra đầu tiên có max absolute error **1,79×10⁻⁶**, RMSE **9,69×10⁻⁸**; lần lặp trên binary cuối có max abs/RMSE **0** trong case này. Reference dùng phonemes từ Rust, không kiểm chứng G2P độc lập; bỏ initialization denoiser không dùng trong preset. Không suy ra chất lượng toàn corpus, random-sampling bit parity, feature parity hay tốc độ Rust/Python tổng quát từ một câu.

## Security và phần chưa hoàn thành

Manifest kiểm tra streaming SHA-256/size và relative paths trong directory capability. Hash phù hợp không chứng minh publisher/model an toàn; production phải có trusted catalog/signature, graph external-data review, DLL search policy và sandbox cho native parsers. Typed policy/fingerprint là primitives; broker phải xác thực caller và enforce quyền/egress tại thời điểm thực thi.

Audit đã phát hiện ZIP 6.0.0 có ba RustSec advisories; nâng **ZIP 9.0.0** trước bàn giao. NORA không gọi ZIP extraction, nhưng vẫn chọn bản vá. [RustSec 0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html), [0368](https://rustsec.org/advisories/RUSTSEC-2026-0368.html), [0369](https://rustsec.org/advisories/RUSTSEC-2026-0369.html). Audit lockfile sau sửa: **0 vulnerability advisories**, giữ cảnh báo unmaintained `paste 1.0.15` qua tokenizer. Đây không phải scan native binaries, weights hay independent production audit; xem [validation](validation.md).

Native model call không bị preempt giữa kernel; cancellation kiểm tra ở ranh giới graph/chunk. Audio queue ngắt không tự hủy goal. Effect không biết kết quả không được retry mù; recovery chỉ chạy khi supervisor giữ exclusive app ownership. Không có generic shell, network listener, tự gửi tin nhắn hoặc tự xin quyền admin trong CLI.

Gates cho bước tích hợp sản phẩm:

1. Nối mic/PTT → VAD → Vietnamese ASR/slot checks → LLM API → scoped action → verified receipt → selected update/TTS. Xử lý timeout, barge-in, lỗi audio và reconnect.
2. User-session supervisor, UI/tray, authenticated IPC, OS grants/revoke, native-host isolation, secret store và egress enforcement. Tách voice stop/pause/cancel; process ownership/recovery.
3. Profile ASR quantized thật, unload/residency scheduling, whole-process-tree peak WS/private commit, GPU dedicated/shared, SSD cache/update staging. **Chưa xác nhận 2.000.000.000 bytes RAM / 4 GB VRAM / 15 GB SSD cho toàn bộ NORA**; không cộng các component peak rồi gọi đó là end-to-end proof.
4. Corpus giọng Bắc/Trung/Nam, số tiền/tên/app, noisy/far-field; human TTS quality và long-text/emotion behavior; matched task completion/security tests, unknown-effect fault injection.
5. Tokenizer maintenance warning, native/runtime advisories, full third-party redistribution notices/SBOM, signed installer/update rollback, CI và production operations gates.

Đường phát triển tiếp theo nằm trong [roadmap](strategy/nora-delivery-roadmap.md). Rust foundation giảm phần phải port ở bước sau; không thay các gates này.
