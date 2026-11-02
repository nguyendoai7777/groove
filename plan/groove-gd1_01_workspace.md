# groove-gd1_01 — workspace

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~350 dòng |
| Phụ thuộc | — |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `Cargo.toml`, `deny.toml`, `crates/*/Cargo.toml`, `core/{paths,i18n}.rs`, `app/main.rs`, `i18n/*/common.toml` |

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 01. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Khai sẵn mọi crate, mọi `mod` với tệp rỗng và mọi phụ thuộc cho các task sau theo "Luật song song" của mục 6; `cargo deny check licenses` xanh trên Windows, macOS.
- Viết `groove/CLAUDE.md` đủ lệnh, bố cục và luật cho task sau; mã mới ở `crates/`, chưa đụng `src/`, `src-tauri/`.
- Worktree build được với `materiant` đường dẫn tương đối (như fastcast: lối nối `.claude/worktrees/materiant`).
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
## 2b. Phân vai
| Phần việc | Ai |
|---|---|
| Toàn bộ task (D-14, dòng Model) | `[opus]` |

## 6. Nghiệm thu
- C1. Cho workspace mới, khi `cargo build`, clippy `-D warnings`, `cargo test`, thì xanh. `[opus-run]` ✅
- C2. Khi `cargo deny check licenses` (cả cây Windows, macOS, Linux), thì `licenses ok`. `[opus-run]` ✅
- C3. Khi `cargo tree --target` Windows, macOS, Linux mặc định, thì không có `alsa`; Linux bật `linux-alsa` thì có. `[opus-run]` ✅
- C4. Cho `src-tauri`, khi `cargo metadata --manifest-path src-tauri/Cargo.toml`, thì vẫn là gói riêng. `[opus-run]` ✅
- C5. Khi mở `groove`, thì có cửa sổ trống 800 × 600 tiêu đề Groove X, không giành focus. `[opus-run]` ✅, xem mục 9: máy chỉ có một màn
- C6. Cho hai catalog, thì cùng bộ khoá và biến; không chữ toàn hoa; thiếu khoá thì về tiếng Anh rồi về chính khoá. `[opus-unit]` ✅
- C7. Test `paths` port nguyên từ `src-tauri` và đường dẫn dữ liệu, cache. `[opus-unit]` ✅
- C8. Cho `--display secondary` trên máy một màn, thì thoát mã 2, không mở cửa sổ. `[opus-run]` ✅

## 8. Nợ chuyển task khác
- 13, 17, 22, 25, 26, 27 nối khởi tạo bằng đúng một lời gọi trong `app.rs` (`CLAUDE.md`, mục Luật song song).
- 25 viết thân `instance::forward_to_primary`.
- 10 đặt mã `cpal` sau cfg `linux-alsa`, nhánh còn lại trả `Error`.
- 02 đặt `i18n::set_language` theo cài đặt; 02 giữ hằng đuôi tệp nhạc ở `model` (S22).
- 12 tự viết Fisher-Yates, không có crate `rand` (S4): seed từ `std::collections::hash_map::RandomState`.

## 9. Bàn giao thực tế
- Soát 02/11/2026: `cargo build`, clippy `-D warnings`, `cargo test` (22 test ở `groove-core`), `cargo deny check licenses`, `cargo fmt --check` đều xanh.
- Chạy thật: `pwsh tools/run.ps1`, cửa sổ trống đúng, RAM 12,2 MB (debug, cột Memory), `foreground_unchanged=True`. **Máy lúc này chỉ có một màn** (DISPLAY1, không có DELL), lần chạy đầu vì vậy rơi về màn chính khoảng 3 giây, không lấy focus. Đã sửa ngay: `--display` không thấy màn thì thoát, `tools/run.ps1` từ chối khi chỉ một màn. Chưa kiểm được ở màn DELL.
- Chưa build được trên macOS, Ubuntu; giấy phép hai nền tảng đó soát qua `deny.toml`, cây phụ thuộc qua `cargo tree --target`.
- Opus chọn 02/11/2026:
  - `cpal` optional trên Linux sau feature `linux-alsa`, tắt mặc định: chưa kéo `libasound` khi chủ dự án chưa quyết. Hệ quả: bản Linux mặc định chưa có tiếng.
  - Spec ghi `notify` 8 + "debouncer-full bản stable tương ứng": dùng 0.5. `rubato` 5, `rusqlite` 0.41, `lofty` 0.25, `id3` 1.
  - Thêm ngoài mục 5 của đặc tả vì task sau cần mà không được sửa `Cargo.toml`: `base64`, `sha2`, `unicode-normalization`, `crossbeam-channel`, `futures`, `anyhow`, `thiserror`, `rusty-leveldb`, `raw-window-handle`, `objc2*`, `embed-resource`. Tất cả qua `cargo deny`.
  - Thêm tệp ngoài danh sách task: `core/logging.rs` (M8, không task nào sở hữu), `app/dev.rs`, `tools/run.ps1` (chạy thử ở màn phụ), `rustfmt.toml`. `instance.rs` có thân giả để `main` không phải sửa ở task 25.
  - `app/app.rs` là ngoại lệ của Luật song song, xem `CLAUDE.md`: khởi tạo phải nối ở một chỗ.
  - Cửa sổ trống còn thanh tiêu đề OS để đóng được; task 03 đổi sang không viền.

## 10. Đề xuất cải tiến
- Bảo mật: không áp, chưa có luồng dữ liệu nào.
- UI/UX: `--display` rơi về màn mặc định đè màn đang dùng, đã sửa (mục 9).
- RAM: phụ thuộc khai sẵn chưa dùng không vào exe, cửa sổ trống 12,2 MB (debug). Đo bản release ở task 29.
- Pro: không áp.
