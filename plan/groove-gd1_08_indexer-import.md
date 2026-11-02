# groove-gd1_08 — indexer-import

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
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

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 08. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Toàn bộ `[opus]`, không giao subagent. Tệp: `library/{scanner,indexer}.rs`, test mới `crates/groove-library/tests/indexer_import.rs`.

## 6. Nghiệm thu

Tất cả `[opus-unit]`, chỉ dùng thư mục tạm.

- C1. Cho cây có đuôi hoa thường lẫn, tệp lạ, bản tạm. Khi `scan`. Thì đúng các tệp nhạc, thứ tự ổn định, có dấu tệp.
- C2. Gốc mất, gốc là tệp, gốc từ chối đọc. Thì `root_failure`; `import` trả `RootUnavailable`, không ghi gì. Thư mục rỗng không phải lỗi.
- C3. Junction / symlink vòng về gốc (S26). Thì quét xong, không theo, dòng sau liên kết không bị xoá.
- C4. Import: thư mục là cha trực tiếp, `track_no` từ `3/12`, `search_key` tìm được không dấu, gốc được ghi, gốc cha nuốt gốc con.
- C5. S1: hai "Greatest Hits" hai nghệ sĩ thành hai album; cùng `album_artist` khác thư mục một album; không có `album_artist` thì theo thư mục, không xé theo nghệ sĩ từng bài.
- C6. Import lại: `(mtime, size)` khớp thì không đọc; tệp đổi thì đọc, giữ `id`.
- C7. Tệp xoá thật thì dòng đi, album và thư mục rỗng được dọn.
- C8. N1: thư mục con bị từ chối đọc bằng quyền thật (`icacls` / `chmod`): bài bên trong và playlist còn, `failed_dirs` báo, tệp mất thật chỗ khác vẫn xoá. `plan_removals`: `PermissionDenied`, `TimedOut`, lỗi lạ không xoá; chỉ `NotFound` mới xoá.
- C9. N1: gốc quét ra 0 tệp mà DB còn dòng: `root_offline`, không xoá.
- C10. Tệp không phải âm thanh: vào `failed`, bài khác vẫn vào, dòng cũ giữ nguyên trường.
- C11. N2: lyrics, timeline trong DB còn sau khi đọc lại tệp không có.
- C12. Bìa: album chung ảnh một dòng `covers`, không tăng số tệp khi đọc lại; cache mất thì có lại; bìa nhóm từ bài đầu tiên có bìa; bìa tự chọn không đổi (S8); ảnh hỏng chỉ mất bìa.
- C13. S7: `rescan` cập nhật tiêu đề, album, lyrics, gieo bìa album mới; tệp thiếu chỉ đếm, không xoá.
- C14. Tác vụ nền (2.3 bước 5): dòng `tags_read_at IS NULL` được điền, gán lại album, mtime thành ms; tệp thiếu, tệp lỗi giữ `NULL`, không lặp vô hạn.
- C15. N7: luồng ghi bị giữ, gọi `import`, `rescan` vẫn trả về ngay, đọc vẫn chạy.
- C16. 250 tệp: `Progress` 100, 200, 250, cuối cùng `Synced`; nhận `Progress` là đọc được lô đó.

## 8. Nợ chuyển task khác

- `groove-gd1_09`: N1 phần `missing_since`, N4 đổi tên. `Indexer::import` hiện xoá thật dòng đã xác nhận mất; thêm cột thì đổi `import_steps` sang đánh dấu.
- `groove-gd1_17`: `LibraryCtl::{import, rescan}` gọi `Indexer`; gọi `backfill_tags` mỗi lần mở app.
- `groove-gd1_18`: S7 yêu cầu một hàm chung, dùng `indexer::{TrackRow, apply_scanned_track}` sau khi ghi thẻ. Task 18 không ghi phụ thuộc 08 nên không nhận khối bàn giao tự động.

## 9. Bàn giao thực tế

**API đã khoá**:

- `groove_library::scanner`: `scan(&Path) -> ScanOutcome { files: Vec<ScannedFile { path, stamp }>, failed_dirs, failed_files, skipped_links }`, `may_still_exist(&Path)`, `root_failure(&Path)`, `ScanFailure { path, message }`.
- `groove_library::indexer`: `Indexer::{new(Library, CoverStore), standard(Library), import(root), rescan(), backfill_tags(), index_paths(Vec<PathBuf>)}`, `Clone`. Mỗi hàm trả về ngay `IndexJob<T>` (`Future`, `wait`). `ImportReport { tally, unchanged, removed, failed_dirs, root_offline }`, `IndexTally { added, updated, missing, failed }` + `read()`, `IndexError::{RootUnavailable, Library}`, `TrackRow::from_scanned`, `apply_scanned_track(&Connection, &TrackRow)`.

**Soát** (`-p groove-library`, trên nhánh sau khi gộp `master`): build, `clippy --all-targets -D warnings`, test xanh (lib 92 có 1 `#[ignore]`, `indexer_import` 22, `library_store` 13, `migrate_legacy` 20, `covers_*` 2), `cargo fmt --all --check`, `cargo deny check licenses` ok. Test quyền thật và junction chạy thật trên Windows, không bị bỏ qua. Không mở DB thật, không đụng thư viện nhạc thật, chưa đo với thư viện thật.

**Cỡ**: `indexer.rs` ~630 dòng mã + 90 test, `scanner.rs` ~100 + 100 test, không có phụ thuộc mới.

**Opus chọn 02/11/2026** (đã ghi `plan/confirm.md`):

- Import bỏ qua tệp `(mtime, size)` khớp và đã đọc thẻ, dù 1.11 ghi "đọc thẻ mọi tệp": S26 yêu cầu không kéo tệp OneDrive khi không đổi. Muốn đọc hết thì dùng `rescan`.
- `file_mtime` mới tính bằng ms theo `FileStamp`. Dòng từ DB cũ còn là giây cho tới khi `tags_read_at` có giá trị.
- Import vẫn xoá thật dòng đã mất (1.11), vì `missing_since` chưa có trong schema (task 09). Điều kiện xoá đủ cả bốn: nằm trong gốc, không thấy lúc quét, chỗ chứa đã xem hết, `symlink_metadata` báo `NotFound`.
- Không theo liên kết thư mục, nhưng ghi vào `skipped_links` và giữ dòng dưới đó: bản cũ có theo, nên DB cũ có thể có bài sau junction. Liên kết tới tệp vẫn nhận.
- Đọc theo từng lô bằng `thread::scope`, tối đa 4 luồng, chờ cả lô rồi ghi: một tệp cloud chậm giữ cả lô, đổi lại không có bộ đệm sắp xếp lại và bìa nhóm luôn từ bài đầu tiên.
- Mọi việc của `Indexer` chạy lần lượt, việc sau chờ trên luồng của nó.
- `backfill_tags` đi theo con trỏ `id`, không theo `pending_tag_reads`: bài đọc lỗi còn `NULL` sẽ bị lấy lại mãi. Bài lỗi thử lại ở lần mở sau.
- Thư mục gốc đĩa không có tên thì lấy đường dẫn làm tên, thay chữ cứng `Unknown Folder` nằm trong DB.

**Lăng kính**: bảo mật: panic, tệp độc đã bọc ở `read_track`, `save`; log chỉ có đường dẫn, không có thẻ. UI/UX: chữ, toast là của task 17; báo cáo đủ số để hiện, kể cả `failed_dirs`, `root_offline`. RAM: tối đa một lô dòng, mỗi lúc một ảnh giải mã, `read_track` giữ tối đa 4 bìa cùng lúc; `known_rows` nạp cả bảng một lần rồi nhả. Chưa đo RSS, đo ở task 29. Pro: không áp.

**Chờ kiểm tay**: import lần đầu trên thư viện OneDrive thật (`plan/_backlog.md`).

**Việc chưa làm**: chưa nối vào UI, thuộc task 17. `missing_since`, phát hiện đổi tên thuộc task 09.
