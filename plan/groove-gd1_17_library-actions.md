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