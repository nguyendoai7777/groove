# groove-gd1_17 — library-actions

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~750 dòng |
| Phụ thuộc | groove-gd1_08, groove-gd1_15, groove-gd1_16 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/library_ctl.rs`, `views/{song_menu,playlist_dialog,card_menu}.rs`, `i18n/*/actions.toml` (N9, S2, S3) |

## 0. Bàn giao từ groove-gd1_09 (02/11/2026)

<!-- handoff: groove-gd1_09 -->

- **Phải nối**: giữ `watcher::LibraryWatcher::start(indexer)` suốt phiên trong `library_ctl`, trước `Library::shutdown` gọi `stop`. `start` tự gắn watcher rồi tự so đĩa với DB, đừng gọi `reconcile` thêm. Sau import, bỏ gốc: `refresh_roots()`.
- **N9 đã có dữ liệu**: schema bản 2 có `excluded_paths`. `queries::{add_excluded_path, remove_excluded_path, excluded_paths, is_excluded}`. Quét, watcher, import gốc cha đều tự bỏ qua chỗ đã thêm; hàm chỉ ghi dòng, không xoá bài: xoá bài dưới đó trong cùng transaction.
- **Import đổi**: `ImportReport.removed` thành `marked_missing`, thêm `moved`, `restored`. Toast "đã xoá" giờ nghĩa là "không còn thấy tệp"; playlist vẫn giữ.
- **Bài đang mất** ẩn khỏi mọi danh sách, 30 ngày mới dọn. Muốn cho người dùng dọn ngay: `reconcile::{missing_songs, forget_missing}`.
- **Sự kiện mới phải nghe**: `LibraryEvent::Moved` tới cùng `Synced` khi đổi tên ngoài app. Mở app không đổi gì thì không có sự kiện, không toast.
- **Soát lại khi làm**: `cargo test -p groove-library --test watcher_reconcile`.
- Chi tiết: `plan/groove-gd1_09_watcher-reconcile.md` mục 9.

## 0. Bàn giao từ groove-gd1_15 (02/11/2026)

<!-- handoff: groove-gd1_15 -->

- **Đã có sẵn để dùng**: màn lưới My Music / Playlists ở `views/library_page.rs` (`LibraryPage`, đã gắn vào `Slot::Library` trong `app::run` bằng `library_page::install(cx)`, hàm trả `Entity<LibraryPage>`); `views::{category_card,cover_image}`; global thư viện `app::{set_library, library, try_library}`. Chi tiết: `plan/groove-gd1_15_library-grid.md` mục 9.
- **Nợ chuyển sang đây**: nhánh `Route::Category(..)` trong `LibraryPage::render` đang là chữ giữ chỗ; thay bằng `SongList` của task 16 (task 16 không sở hữu `library_page.rs`).
- **Sự kiện view phát** (`LibraryPageEvent`, nghe bằng `cx.subscribe(&page, ..)`): `Open(CategoryRef)` (đã nối thành `app::navigate`), `PlayAll(CategoryRef)`, `Delete(CategoryRef)`, `Import`, `Rescan`, `CreatePlaylist`. Task này nghe `PlayAll` / `Delete` / `Import` / `Rescan` / `CreatePlaylist`, làm hộp thoại và nghiệp vụ, rồi báo ngược `set_importing(bool, cx)`, `set_rescanning(bool, cx)`; view tự nạp lại lưới khi nhận `LibraryEvent::{Synced, PlaylistsChanged}`. Tên lấy lại entity: `library_page::install` đã trả về; hoặc đăng ký nghe trong `install` bằng một lời gọi của task này ở `app::run`.
- **Bẫy đã gặp**: `occlude()` quanh nút trong lớp phủ thẻ làm `group_hover` tắt (nút biến mất dưới con trỏ); dùng `cx.stop_propagation()` trong `on_click`. Test view dùng thư viện thật ở thư mục tạm cần `cx.executor().allow_parking()` (luồng đọc đánh thức future từ thread khác), mẫu ở mục test của `library_page.rs`.
- **Soát lại khi làm**: không mở DB người dùng khi chạy thử hay test, dùng `LibraryConfig` thư mục tạm.

## 0. Bàn giao từ groove-gd1_08 (02/11/2026)

<!-- handoff: groove-gd1_08 -->

- **Đã có sẵn để dùng**: `groove_library::indexer::Indexer::standard(library)`, sao chép rẻ; `import(path)`, `rescan()`, `backfill_tags()` trả về ngay `IndexJob<T>`, `await` trên UI. Chọn thư mục ở UI, `LibraryCtl::import` chỉ nối vào đây. Chi tiết: `plan/groove-gd1_08_indexer-import.md` mục 9.
- **Phải làm**: gọi `backfill_tags()` mỗi lần mở app sau khi thư viện mở xong; không có gì thì xong ngay (đặc tả 2.3 bước 5).
- **Toast import**: `ImportReport.tally.added`, `removed.len()`. Nên báo thêm khi `root_offline` (gốc rỗng mà còn bài, không xoá gì), khi `failed_dirs` không rỗng, khi `tally.failed` không rỗng. `Err(IndexError::RootUnavailable)` là thư mục không đọc được.
- **Toast rescan**: `tally.read()`, `tally.missing.len()`; `tally.failed` không phải là thiếu.
- **Sự kiện**: `Progress { kind: Import | Rescan | TagBackfill, done, total }` mỗi lô, xong `Synced`. Nhận `Progress` là đã đọc được lô đó. Import toàn tệp không đổi có `total = 0`, không có `Progress`, chỉ có `Synced`.
- **N9**: bỏ gốc đang chưa có `excluded_paths`; import lại vẫn thêm gốc qua `add_watched_root`.

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Playlist**: `Library::{create_playlist, delete_playlist, add_song_to_playlist, remove_song_from_playlist}`, tự phát `PlaylistsChanged`. Lỗi tên: `LibraryError::Query(QueryError::{EmptyName, DuplicatePlaylist})`, hiển thị bằng i18n, không in lỗi thô.
- **Bìa tự chọn**: `queries::set_album_cover(.., custom = true)`, `set_folder_cover`, `upsert_cover`.
- **N9 chưa đủ dữ liệu**: `queries::{delete_folder, delete_album, add_watched_root, remove_watched_root}` có sẵn nhưng schema chưa có `excluded_paths`, phải thêm cột hoặc chọn cách của đặc tả (bỏ gốc, đổi nhãn nút).

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 17. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N9, S2, S3 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).