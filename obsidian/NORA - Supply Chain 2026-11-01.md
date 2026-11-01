---
title: NORA - Supply Chain 2026-11-01
updated: 2026-11-01
tags: [nora, supply-chain, sbom, licensing, release]
status: partial-inventory-gaps-open
---

# Dependency inventory và bộ notice

[[NORA - Home]] · [[NORA - Desktop Integration 2026-11-01]]

Phần chuẩn bị phát hành này chạy offline từ Cargo metadata, không biên dịch hoặc gọi LLM API. Binary Rust đã kiểm chứng giữ nguyên SHA-256 `d65584f0a69807b577f0d10925cd7b0af6d73bc909c9d286899dbc1b7086e519`.

Đã xuất partial CycloneDX 1.6 build BOM cho 357 packages trong graph normal/build của nora-app Windows all-features. Không phải binary-linkage analysis; composition ghi rõ incomplete. License inventory ghi paths, sizes và SHA-256; export có 251 text blobs riêng biệt, 827.719 bytes.

Còn 18 metadata/packaged-text gaps: một số crate monorepo không kèm file license gốc trong crate archive, cùng trường license metadata bị thiếu của sea-g2p-rs. Đây không phải kết luận vi phạm. Native subcomponents, compiler/std/CRT, ORT DLL, model/dictionary/voice rights và giấy tờ phân phối vẫn cần review.

Exporter giữ nguyên text bytes, loại link/reparse và đường dẫn license ra ngoài package. Copier kiểm lockfile, đường dẫn texts theo hash và kích thước trước khi copy. Local đã qua official JSON schema/formats, 5 Python tests (thêm 1 symlink test skip do Windows không cho tạo link), 4 PowerShell controls và đóng gói thật. Python chỉ là tooling dành cho developer, không đi vào runtime.

Gói local có notice bundle: 1.203.779.356 bytes; `redistribution_cleared=false`, vẫn unsigned validation candidate. Không sửa lại benchmark cũ để tránh lẫn package snapshot với phép đo runtime. Chưa chạy thêm voice hoặc build trên máy đang cần dọn RAM.

[Inventory và cách tái tạo](https://github.com/dakiemdarktharr/NoSleepClub_zalo_hacks/blob/main/docs/supply-chain/README.md)
