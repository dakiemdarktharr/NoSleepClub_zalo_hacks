# ADR-0003: Public research và tài nguyên cuộc thi

Status: **Accepted**. Date: 2026-10-31.

## Context

User yêu cầu public repo. Thể lệ Zalo AI Challenge 2026 giới hạn phân phối dataset và đặt điều kiện công khai mã thi trong thời gian thi. Chưa truy cập dataset/devkit hoặc đăng ký thi.

## Decision

Repo public chứa tài liệu gốc và policy reference độc lập, không dùng để sinh submission. Không lưu audio/dataset, private outputs, credentials hoặc weights chưa rõ quyền. Competition implementation/data workspace cần gate quy định track và điều kiện chia sẻ chính thức trước khi được thêm vào repo. Không tự đăng diễn đàn hoặc chấp thuận điều khoản thay user.

## Consequences

Repo giúp phát triển sản phẩm nhưng chưa là solution dự thi. MIT không cấp quyền với external datasets/weights. Nếu reference code sau này trở thành một phần mã thi, phải đánh giá lại việc chia sẻ theo quy định hiện hành. [Thể lệ chính thức, mục 4 và 8](https://dl-challenge.zalo.ai/zalo-ai-challenge-regulations-vi.pdf).
