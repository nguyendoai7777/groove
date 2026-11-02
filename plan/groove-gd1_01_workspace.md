# groove-gd1_01 — workspace

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~350 dòng |
| Phụ thuộc | — |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `Cargo.toml`, `deny.toml`, `crates/*/Cargo.toml`, `core/{paths,i18n}.rs`, `app/main.rs`, `i18n/*/common.toml` |

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 01. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Khai sẵn mọi crate, mọi `mod` với tệp rỗng và mọi phụ thuộc cho các task sau theo "Luật song song" của mục 6; `cargo deny check licenses` xanh trên Windows, macOS.
- Viết `groove/CLAUDE.md` đủ lệnh, bố cục và luật cho task sau; mã mới ở `crates/`, chưa đụng `src/`, `src-tauri/`.
- Worktree build được với `materiant` đường dẫn tương đối (như fastcast: lối nối `.claude/worktrees/materiant`).
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).