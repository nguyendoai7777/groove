# groove-gd1_23 — command-palette

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
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