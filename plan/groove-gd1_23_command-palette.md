# groove-gd1_23 — command-palette

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~550 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_22, groove-gd1_05 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/command_palette.rs`, `i18n/*/palette.toml` |

## 0. Bàn giao từ groove-gd1_22 (02/11/2026)

<!-- handoff: groove-gd1_22 -->

- **Đã có sẵn để dùng**: `groove_core::cli::{COMMANDS, CliSpec, CliArg, is_cli, filter, resolve, CliCommand, CliError}`; `crate::commands::{run, run_cli, CliOutcome, play_search_result, QUICK_ACTIONS, key_label}`.
- **Enter (S21)**: `cli::resolve(chữ_đang_gõ, dòng_đang_chọn)` rồi `commands::run_cli(lệnh, cx)`, trả `Task<CliOutcome>`. `Done`: đóng bảng. `ShowHelp`: giữ bảng, điền `/` vào ô. `NoMatch`: báo không có bài. `Err(CliError)` từ `resolve`: không đóng, hiện lời báo theo biến thể (chữ thuộc vùng `palette`).
- **Tab**: `spec.completion()`. **Mô tả lệnh**: tra catalog theo `spec.name`, core không giữ chữ.
- **Chọn bài**: `commands::play_search_result(song.track, cx)`, không đóng bảng (đặc tả 1.9). **Thao tác nhanh**: `commands::run(cmd, cx)` cho từng `QUICK_ACTIONS`, nhãn `key_label(cmd, cx)` (`None` thì không hiện `Kbd`); `cmd.name()` là khoá catalog.
- **Mở bảng**: `ShellEvent::OpenSearch` đã phát từ cả ô tìm và `Ctrl+K`, kể cả khi đang gõ trong ô nhập.
- **Cần biết**: bảng là dialog nên phím toàn cục tự tắt khi mở, kể cả `Ctrl+K`; không cần chặn thêm. Dialog phải đi qua `materiant::overlay` (`stack::depth > 0` là tín hiệu chặn).

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Tìm**: `Library::search(query, limit)` trả `SearchResults { seq, songs }`. Lượt cũ còn trong hàng ra `LibraryError::Superseded`, im lặng bỏ qua; kết quả có `seq` nhỏ hơn cái đang hiện cũng bỏ (S18). Truy vấn rỗng trả rỗng.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 23. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
## 8. Việc thiếu từ làn khác

- Không có. Cần chỗ treo bảng trong cây cửa sổ nên thêm `Slot::CommandPalette` vào `app.rs` (xem mục 9 và `plan/confirm.md`).

## 9. Bàn giao thực tế (02/11/2026)

- **Đã làm**: `views/command_palette.rs` (`CommandPalette`, `install(window, cx)`), `i18n/{vi,en}/palette.toml`. `app::run` gọi `command_palette::install` một lần; bảng nghe `ShellEvent::OpenSearch` (ô tìm, `Ctrl+K`, kể cả đang gõ).
- **Hành vi**: dialog 600 px `align_top`; trống: ba `QUICK_ACTIONS` (nhãn phím `key_label`), Enter chạy rồi đóng. Có chữ: debounce 250 ms, `Library::search(.., 50)`, `Superseded` thử lại tối đa 2 lần rồi im lặng, kết quả `seq` cũ bỏ; chọn bài `play_search_result`, không đóng; bài đang chọn của player hiện bars. `/`: lọc `cli::filter`, Tab `completion()`, Enter `cli::resolve` + `run_cli`; `Done` đóng, `ShowHelp` điền `/`, `NoMatch` và `CliError` hiện lời báo dưới ô, không đóng. ↑ ↓ quay vòng bỏ tiêu đề nhóm, rê chuột dời con trỏ, nhấp dòng đang chọn là chạy, Esc đóng (Dialog), mở lại thì ô trống. Đóng bảng thì xoá danh sách bài (RAM).
- **Phím ↑ ↓ Tab Enter** đi qua `cx.intercept_keystrokes` (ô nhập một dòng giữ ↑ ↓ và Tab cho nó). Chỉ chạy khi bảng mở.
- **Sửa `app.rs` (ngoài danh sách task 23, đã ghi `plan/confirm.md`)**: thêm `Slot::CommandPalette`, một dòng vẽ nó trong `Shell::render`, một lời gọi `install` trong `run`. Dialog của materiant phải nằm trong cây của view; chưa có chỗ nào khác.
- **Quyết định đã chốt (không đoán nghiệp vụ)**: tiêu đề nhóm cao bằng dòng thường (danh sách ảo đòi dòng đều); mô tả thao tác nhanh và lệnh lấy từ catalog `palette`, không hardcode; chữ "Empty Placeholder" của bản cũ thay bằng `Không tìm thấy kết quả cho "<chữ>"`.
- **Case**: C1 quick action chạy + đóng; C2 quay vòng; C3 tìm rồi chọn bài không đóng; C4 gõ nhanh chỉ lượt cuối; C5 không kết quả; C6 `/` lọc + Tab; C7 Enter chạy lệnh đang chọn rồi đóng, lệnh `/n`; C8 lệnh sai báo, gõ tiếp xoá báo; C9 `/help`; C10 `/play` không khớp báo; C11 Esc, mở lại sạch; thêm hai test thuần (`step_selection`, `song_description`). 13 test, tất cả `[sonnet-test]`, xanh.
- **Soát**: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test --no-fail-fast` (toàn workspace), `cargo deny check licenses`, `cargo fmt --all --check`: xanh. Lần chạy `cargo test` đầu có test engine `idle_closes_stream_and_keeps_position` đỏ (đã ở `_backlog.md`, không dính task), chạy lại xanh.
- **Soi cải tiến**: bảo mật không áp (không ghi đĩa, không IPC); UX: dùng hết bằng bàn phím, lời báo kèm cách gỡ, vi / en cùng bộ khoá; RAM: một view nhỏ, danh sách bài nhả khi đóng, không thêm phụ thuộc; Pro không áp. Không có đề xuất ngoài phạm vi.
- **Chờ kiểm tay** `[owner-qa]` (đã vào `plan/_backlog.md`): nhìn bảng thật (khung đệm 24 px của `Dialog` có thể rộng hơn bản cũ), hover, thanh cuộn, `ShellEvent::OpenSearch` nối thật từ ô tìm và `Ctrl+K` (không có test vì `Shell` global của `app.rs` là riêng tư).
