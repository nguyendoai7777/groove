# groove-gd1_08 — indexer-import

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_05, groove-gd1_06, groove-gd1_07 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/{scanner,indexer}.rs` |

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Ghi**: `library.writer().transaction(|conn| ...)`, bên trong gọi `db::queries`. Đọc thẻ, giải ảnh làm ở worker, xong mới gửi vào; mỗi transaction tối đa `actor::BATCH_ROWS` (100) dòng. `.wait()` từng lô để có backpressure, channel ghi không giới hạn.
- **Hàm cho task này**: `queries::{upsert_song, SongUpsert, get_or_create_folder, get_or_create_album, delete_empty_albums, upsert_cover, set_album_cover, set_folder_cover, add_watched_root}`; `Library::pending_tag_reads(limit)` cho tác vụ `tags_read_at IS NULL`.
- **`upsert_song`** tra theo `path_key`, tự tính `filename`, `search_key`, tự lấy tên album từ `album_id`. Lyrics, timeline rỗng từ tệp giữ bản DB (N2). Các trường còn lại ghi đè: truyền đủ, nhớ `tags_read_at = queries::now_ms()`.
- **Sự kiện**: sau khi lô commit gọi `writer().emit(LibraryEvent::Progress(..))`, xong hẳn `Synced(SyncReport { added, updated, removed })`, `removed` là `id`.
- **`set_album_cover(.., custom = false)`** không đè bìa người dùng chọn (S8).
## 0. Bàn giao từ groove-gd1_06 (02/11/2026)

<!-- handoff: groove-gd1_06 -->

- **Đã có sẵn để dùng**: `tags::read_track(path) -> ScannedTrack { fields, cover, duration_ms, stamp }`, gom một lần đọc (S14), bọc `catch_unwind`. `fixtures::write_sample` sinh tệp mẫu cho test. Chi tiết: mục 9 của `groove-gd1_06`.
- **Đọc thẻ**: tệp có âm thanh mà thẻ hỏng vẫn `Ok` với trường trống; không phải âm thanh thì `Err`. Không có tệp thì `TagError::NotFound`.
- **Số**: `track_no`, `disc_no` là `Option<String>` như trong thẻ, cột INT tự parse. `FileStamp.mtime_ms` là mili giây; cột `file_mtime` của DB cũ là giây, nên chọn một đơn vị khi so (S6).
- **RAM**: `cover` là byte đầy đủ; tính `cover_hash`, bìa thu nhỏ xong nhả ngay, đừng giữ qua kênh tới actor.
- **Watcher (task 09)**: bỏ qua bản tạm bằng `tags::is_temp_path`; đuôi nhạc ở `groove_core::model`.
- **Bẫy**: mẫu OGG có setup giả, chỉ dùng để thử thẻ.

## 0. Bàn giao từ groove-gd1_07 (02/11/2026)

<!-- handoff: groove-gd1_07 -->

- **Đã có sẵn để dùng**: `groove_library::covers::CoverStore`: `default_location()`, `save(&self, &[u8]) -> Result<CoverRecord, CoverError>`, `has_files(hash)`, `path(hash, CoverSize)`; hàm `cover_hash(&[u8])`. `Clone + Send + Sync`, gọi được từ worker. API đủ ở `plan/groove-gd1_07_covers.md` mục 9.
- **Phải làm**: `save` không chạm DB. Indexer tự `INSERT OR IGNORE INTO covers` từ `CoverRecord` và gán `cover_hash`, ở `LibraryActor` như mọi lệnh ghi.
- **Tránh giải mã lặp**: album 12 bài có 12 bản bìa giống nhau. Tính `cover_hash` trước; đã có dòng `covers` và `has_files(hash)` đúng thì bỏ qua `save`. Cache có thể bị xoá trong khi DB còn dòng, nên vẫn phải soát `has_files`.
- **Giữ RAM**: mỗi `save` đỉnh bằng một lần giải mã, trần 192 MiB. Chỉ cho một luồng giải mã bìa tại một thời điểm, không chạy theo số worker đọc thẻ. Gọi ngoài transaction.
- **Ảnh hỏng**: `save` trả `Err` kể cả khi decoder panic, không cần bọc `catch_unwind`. `Err` chỉ mất bìa, bài vẫn vào thư viện với `cover_hash = NULL`, ghi `log::warn!`.
- **Bìa album**: `save` nhận byte gốc, ảnh nhỏ không phóng to nên `_512.jpg` có thể nhỏ hơn 512 và không vuông.
- **Soát lại khi làm**: `cargo test -p groove-library --test covers_migrate`; nhập lại một thư mục không tăng số tệp ở thư mục bìa.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 08. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
