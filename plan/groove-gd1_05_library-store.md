# groove-gd1_05 — library-store

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~750 dòng |
| Phụ thuộc | groove-gd1_04 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/{actor,store}.rs`, `library/db/queries.rs` |

## 0. Bàn giao từ groove-gd1_04 (02/11/2026)

<!-- handoff: groove-gd1_04 -->

- **Đã có sẵn để dùng**: `db::migrate::open_default(&mut dyn CoverSink) -> Result<(Connection, OpenOutcome)>` cho actor; test dùng `open_or_migrate(target, legacy, sink)` hoặc `open_fresh(target)`, cả hai đã đặt pragma cho kết nối đó.
- `db::schema`: `apply_pragmas` (mọi kết nối phụ phải gọi), `normalize_text`, `search_key`, `album_group_key`, `path_key`. Không tự viết lại công thức.
- **Đổi so với lúc viết spec**: `songs.path_key` là UNIQUE, `file_path` không còn UNIQUE. Upsert, tra bài theo đường dẫn phải qua `path_key`.
- `search_key` các trường cách nhau bằng `\n`: truy vấn qua `normalize_text` rồi `instr(search_key, ?)`.
- `playlist_songs.position` tính từ 0, `added_at`, `tags_read_at` là ms từ epoch.
- **Phải xử lý**: `MigrateError::TooNew` (báo, không mở), `LegacyFailed` (`library.db` chưa có, tệp cũ nguyên; báo rồi `open_fresh` hoặc thử lại lần sau), `OpenOutcome::Migrated(report)`.
- **Nợ**: sink thật ở `covers.rs` (07). Khi 07 chưa gộp thì nhận sink từ caller, không tự viết sink bỏ bìa ở đường chạy thật.
- **Bẫy**: `Connection::close` trước khi đổi tên tệp trên Windows. Test dùng closure làm sink được. Thử bằng thư mục tạm, không đụng DB thật.
Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 05. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai
Toàn bộ `[opus]`, không giao subagent.

## 6. Nghiệm thu
- C1 `[opus-unit]` upsert, tra đường dẫn qua `path_key`, giữ `id`, giữ lyrics, timeline DB (M2, N2).
- C2 `[opus-unit]` tìm không dấu, `-` không khớp tất cả, truy vấn rỗng trả rỗng, tiêu đề xếp trước (S18).
- C3 `[opus-unit]` thứ tự album, thư mục, playlist, lưới (S19); album trùng tên tách theo `group_key` (S1).
- C4 `[opus-unit]` đọc không chờ transaction ghi đang mở, thấy bản đã commit (N7).
- C5 `[opus-unit]` `Library::open` trả về ngay khi chuyển đổi đang kẹt; việc đọc đến sớm chờ, không lỗi giả (N7).
- C6 `[opus-unit]` `TooNew`: từ chối, tệp không đổi một byte, cả hai lối ra đều không ghi đè.
- C7 `[opus-unit]` `LegacyFailed`: không tạo `library.db`, tệp cũ nguyên; thử lại được, bỏ qua được.
- C8 `[opus-unit]` transaction lỗi hoặc panic thì rollback, luồng ghi sống tiếp (S9, S25).

## 8. Nợ chuyển task khác
- 09, 17: schema chưa có `missing_since` (N1), `excluded_paths` (N9). Đặc tả mục 3 có, đặc tả 2.2 và `schema.rs` không. Đã ghi vào khối bàn giao hai task đó.
- 13, 15 hoặc 17: gọi `Library::open_standard()` trong `app.rs` và hiển thị `OpenFailure`; task này không có UI.

## 9. Bàn giao thực tế
- Soát 02/11/2026 trên nhánh sau khi gộp `master`, `-p groove-library`: build, clippy `-D warnings`, test, `cargo fmt --all --check`, `cargo deny check licenses` xanh. Không mở DB thật, chưa đo với DB thật.
- Cỡ: mã `actor.rs` ~460, `store.rs` ~235, `queries.rs` ~590 dòng, vượt ước lượng ~750. Không tách vì ba tệp dùng chung kiểu và đều do task này sở hữu; thêm `tests/library_store.rs`.
- API đã khoá, `groove_library`:
  - `store::Library`: `open_standard`, `open(LibraryConfig, Box<dyn CoverSink + Send>)`, `status`, `retry_open`, `start_fresh`, `subscribe`, `writer`, `read`, `categories`, `playlists`, `category`, `songs`, `song_ids`, `search`, `song`, `songs_for_tracks`, `track_for_path`, `song_text`, `watched_roots`, `song_file_states`, `pending_tag_reads`, `create_playlist`, `delete_playlist`, `add_song_to_playlist`, `remove_song_from_playlist`, `shutdown`. `LibraryConfig`, `SearchResults`.
  - `actor`: `LibraryActor::{transaction, transaction_then_emit, emit}`, `Reply<T>` (`Future`, `wait`), `BATCH_ROWS`, `LibraryError`, `LibraryResult`, `OpenFailure`, `OpenStatus`, `LibraryEvent`, `SyncReport`, `MovedSong`, `Progress`, `ProgressKind`.
  - `db::queries`: hàm thuần trên `&Connection`, `QueryError`, `CategorySummary`, `Categories`, `SongText`, `SongFileState`, `PendingTagRead`, `SongUpsert`, `UpsertOutcome`, `now_ms`.
- Opus chọn 02/11/2026:
  - Luồng đọc riêng cũng là một actor, kết nối `READ_ONLY`: UI thread không chạm đĩa cả khi đọc. Đặc tả chỉ ghi "một kết nối đọc riêng".
  - `LegacyFailed` không tự `open_fresh`: tạo rồi thì không bao giờ chuyển lại. Thư viện ở trạng thái `Failed`, UI cho chọn `retry_open` hoặc `start_fresh`.
  - Ghi nhận closure (`transaction`) thay vì enum lệnh: task 08, 09, 18 không phải sửa `actor.rs`, đúng luật song song.
  - `futures` `oneshot`, `mpsc` cho kết quả và sự kiện, để UI `await` được mà không chặn luồng.
  - `Library::open_standard` dùng `LibraryConfig::standard` và `CoverStore::default_location` thay cho `migrate::open_default`, vì luồng đọc cũng cần đường dẫn; hai đường dẫn giống hệt.
  - Hạt giống `watched_roots` từ `folders` chỉ chạy một lần sau chuyển đổi, không ở lần đọc như bản cũ: chạy ở lần đọc thì người dùng bỏ hết gốc sẽ thấy gốc quay lại (N9).
  - Đặt bìa tự động không đè bìa tự chọn; `position` có lỗ sau khi xoá, chỉ dùng để xếp. Chưa có lệnh sắp lại playlist, đặc tả 2.4 không ghi.

## 10. Đề xuất cải tiến
- Bảo mật: không ghi nội dung lyrics, truy vấn vào log; kết nối đọc không ghi được (có test). Đường dẫn DB có trong log lỗi mở.
- UI/UX: không áp ở tầng này; yêu cầu hiển thị `OpenFailure` đã ghi cho task 15.
- RAM: thêm hai luồng và một kết nối đọc; `sink` thả ngay sau khi mở; chưa đo RAM, đo ở task 29. Channel không giới hạn: worker phải `wait` từng lô, đã ghi cho 08.
- Pro: không áp.
