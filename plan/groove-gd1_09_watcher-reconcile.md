# groove-gd1_09 — watcher-reconcile

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~700 dòng |
| Phụ thuộc | groove-gd1_08 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/{watcher,reconcile,indexer}.rs`, `library/db/{schema,migrate,queries}.rs`, `tests/{watcher_reconcile,indexer_import,migrate_legacy}.rs` (N1, N4, S6) |

## 0. Bàn giao từ groove-gd1_08 (02/11/2026)

<!-- handoff: groove-gd1_08 -->

- **Đã có sẵn để dùng**: `scanner::scan(root) -> ScanOutcome { files (có `stamp`), failed_dirs, failed_files, skipped_links }`, `may_still_exist(path)`, `root_failure(root)`; không theo liên kết thư mục. `Indexer::index_paths(paths) -> IndexJob<IndexTally>` đọc và upsert theo lô, không xoá, không phát `Synced`: tự gộp với phần xoá, đổi tên rồi `emit`. Chi tiết: `plan/groove-gd1_08_indexer-import.md` mục 9.
- **Một điểm vào (S7)**: mọi cập nhật từ tệp đi qua `indexer::{TrackRow, apply_scanned_track}` hoặc `index_paths`, đừng viết đường thứ hai.
- **Đơn vị mtime**: `file_mtime` mới là ms (`FileStamp`). Dòng `tags_read_at IS NULL` từ DB cũ còn là giây: khi so S6 phải coi đó là "chưa có mốc", không phải "đã đổi", nếu không lần mở đầu sẽ báo cả nghìn bài cập nhật. `SongFileState` chưa có `tags_read_at`, xem `known_rows` trong `indexer.rs`.
- **Nợ chuyển sang đây**: `Indexer::import` hiện xoá thật dòng đã xác nhận mất, cascade mất playlist. Thêm `missing_since` thì đổi `import_steps` sang đánh dấu; đổi tên ngoài app (N4) cũng cần nhánh ở đó, vì import coi tệp mới và dòng mất là hai việc riêng.
- **Mẫu xoá an toàn**: `plan_removals` chỉ xoá khi `NotFound` thật, chỗ chứa đã xem hết; gốc 0 tệp mà còn dòng là offline. Dùng lại luật này, có thể cho thành `pub(crate)`.
- **Bẫy**: `Indexer` chạy lần lượt từng việc; reconcile chạy qua `index_paths` sẽ chờ import đang chạy, không cần khoá thêm. Test quyền thật: dùng `icacls /deny *S-1-1-0`, xem `Locked` trong `tests/indexer_import.rs`.
- **Soát lại khi làm**: `cargo test -p groove-library --test indexer_import`.

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Đối chiếu**: `Library::song_file_states()`, `Library::watched_roots()`; ghi qua `writer().transaction` với `queries::{move_song, delete_song_by_path, set_song_file_state, upsert_song}`. `move_song` giữ `id`, đích là bài khác thì `QueryError::PathConflict`. Phát `LibraryEvent::Moved` sau commit.
- **Thiếu trong schema**: N1 cần `missing_since`, N9 cần `excluded_paths`, `schema.rs` (04) ở `user_version = 1` chưa có. Task này cần `missing_since` thì phải thêm cột và cách nâng phiên bản; `migrate::open_existing` hiện chỉ nhận đúng các phiên bản đã biết.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 09. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N1, N4, S6 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Toàn bộ `[opus]`, không giao subagent.

## 6. Nghiệm thu

Tất cả `[opus-unit]` trên tệp thật trong thư mục tạm, `crates/groove-library/tests/watcher_reconcile.rs` (26 test) và unit test của `db/{migrate,queries}`, `reconcile`, `watcher`.

- C1 (N1): tệp vắng khi mở app thì đánh dấu, playlist giữ dòng; tệp về thì hiện lại đúng vị trí, không đọc lại thẻ.
- C2 (N1): thư mục mất quyền thật (`icacls`): bài trong đó không bị đánh dấu, tệp mất thật ở chỗ khác vẫn bị đánh dấu.
- C3 (N1): gốc rỗng hoặc biến mất: không đụng gì, kể cả quá hạn nhiều lần.
- C4 (N1): chưa tới 30 ngày không dọn; quá hạn và xác nhận không có thì dọn, kèm album rỗng; bài vừa về trong lượt đó không bị dọn.
- C5 (N1): `forget_missing` chỉ xoá dòng đang đánh dấu.
- C6 (N4): đổi tên tệp, đổi tên thư mục khi app tắt giữ `id`, playlist, timeline, album đi theo thư mục mới.
- C7 (N4): cặp đổi tên của watcher được tin cả khi thẻ đổi cùng lúc; đổi tên đè bài khác không gộp dòng; bản sao không phân biệt được thì không ghép.
- C8 (S6): tệp có dấu `(mtime, size)` khớp DB bị bỏ qua ở mọi lô, không phát sự kiện; sửa ngoài app đọc đúng một lần; dòng DB cũ không báo đổi.
- C9: nâng DB bản 1 giữ dữ liệu; lỗi giữa chừng để nguyên bản 1; schema nâng và schema mới giống hệt.
- C10: watcher thật: đổi tên tệp, thư mục, sửa, thêm, xoá; gốc vắng lúc mở rồi quay lại; `refresh_roots`.

## 8. Nợ chuyển task khác

- **groove-gd1_17**: gọi `LibraryWatcher::start` lúc mở app, `refresh_roots` sau import, bỏ gốc; N9 dùng `queries::add_excluded_path`; hộp thoại xác nhận dọn bài mất nếu muốn. Đã ghi mục 0 của task đó.
- **groove-gd1_13**: `SyncReport.removed` gồm cả bài vừa đánh dấu mất. Đã ghi mục 0 của task đó.

## 9. Bàn giao thực tế

**API đã khoá**:

- `watcher::LibraryWatcher::{start(Indexer), start_with(Indexer, WatcherOptions), refresh_roots(&self), stop(self)}`, thả giá trị cũng dừng. `start` gắn watcher rồi tự `reconcile` trên luồng nền. `watcher::{ChangeBatch { dirty, renames, need_rescan }, apply_batch(&Indexer, ChangeBatch) -> IndexJob<SyncOutcome>, DEBOUNCE, ROOT_RETRY}`.
- `reconcile::{reconcile(&Indexer) -> IndexJob<ReconcileReport { sync, roots, failed_dirs }>, SyncOutcome { tally, unchanged, moved, restored, marked_missing, purged }, RootState, MISSING_GRACE_MS, missing_songs(&Library), forget_missing(&Library, Vec<i64>)}`.
- `queries::{mark_song_missing, clear_song_missing, delete_missing_song, missing_songs, MissingSong, add_excluded_path, remove_excluded_path, remove_exclusions_covering, excluded_paths, is_excluded}`. `schema::{SCHEMA_VERSION = 2, upgrade_schema}`.
- `ImportReport.removed` đổi thành `marked_missing`, thêm `moved`, `restored`. Import không còn xoá dòng nào.

**Quyết định** (đã ghi `plan/confirm.md`):

- Bài có `missing_since` ẩn khỏi lưới, danh sách, tìm kiếm, `song`, `songs_for_tracks`, `track_for_path`, kể cả playlist (dòng `playlist_songs` còn nguyên, về thì hiện đúng chỗ). `song_by_id`, `song_by_path` còn thấy, nên `metadata_ops` không đổi. `groove-core::Song` không có cờ "mất" nên không hiện dòng xám được.
- `Synced.removed` gồm cả bài vừa đánh dấu mất: với hàng đợi, bài đó không phát được nữa (N5).
- `move_song` lấy tên của một dòng đang mất thì dòng đó bị xoá, vì `track_for_path` coi tên đó là trống nên `metadata_ops` cho đổi tên vào.
- Dọn 30 ngày chỉ khi gốc vừa quét được, chỗ chứa xem hết, tệp vẫn `NotFound`.
- Dòng DB cũ chưa có `file_size` ghép đổi tên chỉ khi cùng tên tệp, có tiêu đề, lệch thời lượng dưới 1 giây.
- Hai bản sao không phân biệt được: không ghép, thành dòng mới và dòng mất.
- `excluded_paths` có `path_key` để so kiểu M2. Chọn lại đúng chỗ đang bị che thì import gỡ dòng che; import gốc cha vẫn tôn trọng.

**Kết quả kiểm**: `cargo clippy -p groove-library --all-targets -- -D warnings` xanh, `cargo test -p groove-library` xanh (lib 110, watcher_reconcile 26 chạy lặp 4 lần không rụng, indexer_import 22, metadata_ops 30, migrate_legacy 20), `cargo build --workspace` xanh, `cargo deny check licenses` ok. Không thêm phụ thuộc. Test mất quyền đã chạy thật trên Windows, không bị bỏ qua.

**Soi cải tiến**: Bảo mật: đường dẫn chỉ từ watcher và DB, không có IPC mới; log chỉ ghi đường dẫn nhạc của chính người dùng. UI/UX: chưa có màn cho bài đang mất, xem mục 8. RAM: thêm một luồng và debouncer theo số gốc, `known_rows` đọc cả bảng mỗi lô rồi nhả; chưa đo vì chưa nối vào app. Pro: không áp.

**Giới hạn đã biết**:

- Chuyển tệp sang gốc khác mà hai sự kiện rơi vào hai lô, tạo trước xoá sau, thì tệp mới đã có dòng riêng khi dòng cũ bị đánh dấu nên không ghép. Debounce 2 giây gom gần hết trường hợp, dòng cũ vẫn giữ playlist 30 ngày.
- Điểm gắn Linux mount sau khi đã theo dõi thư mục rỗng không có sự kiện, lần mở app kế mới thấy.
- Chưa nối vào app: chưa chạy trên thư viện OneDrive thật. Chờ task 17.
