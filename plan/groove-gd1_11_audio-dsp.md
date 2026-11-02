# groove-gd1_11 — audio-dsp

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~300 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `audio/dsp.rs` |

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Đã có**: `audio/dsp.rs` trống, không phụ thuộc `cpal`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 11. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
