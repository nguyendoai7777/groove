# groove-gd1_18 — metadata-ops

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~600 dòng |
| Phụ thuộc | groove-gd1_05, groove-gd1_06 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/metadata_ops.rs` (N2, N3, S5, S7, S8) |

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Hàm cho task này**: `Library::song_text(id)`, `song_ids(CategoryRef)`; `queries::{set_song_lyrics, set_song_timeline, set_song_file_state, move_song, upsert_song}` qua `writer().transaction`. Ghi tệp xong mới gửi transaction, không ghi tệp trong closure (N7).
- **Bẫy**: `upsert_song` giữ lyrics, timeline DB khi tệp rỗng, nên muốn xoá thật thì dùng `set_song_lyrics(.., None)`, `set_song_timeline(.., None)`.
## 0. Bàn giao từ groove-gd1_06 (02/11/2026)

<!-- handoff: groove-gd1_06 -->

- **Đã có sẵn để dùng**: `tags::write_tags(path, &MetadataEdit, WriteMode) -> FileStamp` đã ghi qua `<tên>.groove-tmp`, đọc lại kiểm tra, thay tệp, xoá tạm khi lỗi; `read_track`, `read_raw_frames`. Chi tiết: mục 9 của `groove-gd1_06`.
- **Quy ước sửa**: `None` không đụng, `Some("")` xoá. `CoverEdit::Keep` giữ bìa ở mọi chế độ. `TIMELINE` tự được giữ ở mọi chế độ.
- **Còn lại ở task này (N2, N3)**:
  - `Clean` không có lyrics trong phần sửa là mất lyrics: phải đọc từ DB (tệp ngoài thư viện: từ `read_track`) rồi truyền vào `fields.lyrics`.
  - DB có timeline mà tệp không có: truyền `fields.timeline` để đẩy ngược vào tệp.
  - Kiểm tra tên mới, đổi tên, cập nhật DB trong transaction chưa làm; `write_tags` chỉ thay nội dung, đường dẫn giữ nguyên.
  - Lưu `FileStamp` trả về vào DB để watcher bỏ qua (S6).
- **Lỗi cho người dùng**: `TagError` hiển thị bằng tiếng Anh để ghi log; toast tra theo biến thể qua i18n. `Replace` là tệp đang bị app khác giữ, `ReadOnly`, `CoverTooLarge`, `UnsupportedCover`, `Verify` nên có câu riêng.
- **Bẫy**: bìa quá 16 MiB trên FLAC là `TagError::Write` (trần định dạng).
- **Soát lại khi làm**: `cargo test -p groove-library tags`.

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 18. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N2, N3, S5, S7, S8 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

| Phần việc | Vai |
| --- | --- |
| `library/metadata_ops.rs`, `tests/metadata_ops.rs` | `[opus]` |

## 6. Nghiệm thu

Đều `[opus-unit]`: unit trong `metadata_ops.rs`, còn lại ở `tests/metadata_ops.rs`, chỉ dùng thư mục tạm.

- C1 (N3). Đổi tên trùng tệp khác, hoặc trùng dòng DB: `TargetExists`, hai tệp nguyên từng byte, thẻ chưa ghi.
- C2 (N3). Tên `..`, `../x`, `..\x`, tuyệt đối, `.mp3`, rỗng, ký tự cấm, tên dành riêng, đổi đuôi: `InvalidName`, không có tệp ngoài thư mục.
- C3 (N3). Đổi tên thành công giữ `id`, playlist; phát `Moved`; dấu tệp khớp; không sót tạm. Chỉ đổi hoa thường được. Tệp đang mở bằng `File::open` vẫn đọc được.
- C4 (N3). Ghi thẻ lỗi: không đổi tên, DB nguyên. DB lỗi sau đổi tên (trigger thật): tệp về tên cũ, DB, playlist nguyên, `file_written()`. Đổi tên lỗi: DB theo thẻ mới ở tên cũ, `RenameFailed`.
- C5 (N2). `Clean` trên 6 định dạng giữ lyrics, timeline từ DB và đẩy vào tệp; mọi chế độ đẩy timeline; tệp ngoài giữ lyrics của nó; `Some("")` xoá cả tệp và DB.
- C6 (S5). Lyrics ghi ID3v2.3, lưu dấu tệp; lỗi ghi trả về, DB giữ bản cũ; không phát `Synced`.
- C7 (S7). `indexer::apply_scanned_track` đủ trường, gieo bìa album mới; sửa lyrics ngoài rồi đọc lại thì DB theo.
- C8 (S8). Sửa một bài không đổi bìa nhóm đã có; nhóm trống thì gieo; custom không bị gieo đè; áp cho nhóm thì đổi; bìa custom theo bài khi Fix grouping làm rỗng album.
- C9. Lô: bài lỗi đúng `id`, đường dẫn; bài khác xong và giữ tiêu đề, timeline riêng; `Clean` bị hạ; tiến độ 1, 2, 3; lỗi DB phân biệt với lỗi tệp; đóng thư viện giữa lô thì mọi bài nằm trong đúng một danh sách.

## 9. Bàn giao thực tế

**Kết quả**: `-p groove-library`: build, clippy `-D warnings`, test xanh (30 test `tests/metadata_ops.rs`, 4 test đơn vị); `cargo deny check licenses` xanh. Không phụ thuộc mới. Chưa thử trên thư viện nhạc thật.

**API đã khoá** (`groove_library::metadata_ops`):

- `MetadataOps::new(Library, CoverStore) -> io::Result<_>`, `Clone`. Mọi hàm trả ngay `OpsReply<T>` (`Future`, `wait`):
  - `read(TrackRef) -> MetadataView`
  - `save(TrackRef, MetadataEdit, WriteMode, Option<String>) -> SavedTrack`, tham số cuối là tên mới kèm đuôi
  - `save_lyrics(TrackRef, String)`, `save_timeline(TrackRef, String) -> SavedTrack`
  - `save_bulk(CategoryRef, GroupEdit, WriteMode) -> BulkReport { done, failed: Vec<BulkFailure { id, path, error }>, not_started, group_cover_failed }`
- `validate_file_name(raw, current) -> Result<String, FileNameError>`, gọi được từ UI để báo sớm, không chạm đĩa.
- `MetadataError::{InvalidName, TargetExists, SongNotFound, Tag, RenameFailed, DatabaseAfterWrite, Library}`, `file_written()`.

**Opus chọn 02/11/2026** (có ở `plan/confirm.md`):

- `save` thêm tham số tên mới so với đặc tả 2.4, vì `MetadataEdit` thuộc `tags`, không có tên tệp.
- Lưu xong phát `Progress { MetadataBulk, done == total }`, không phát `Synced`: `Synced` kéo theo toast của watcher (S5), `actor.rs` đã khoá nên không thêm sự kiện mới.
- Không cho đổi đuôi; cấm ký tự, tên dành riêng của Windows ở mọi OS.
- `Clean` đọc lyrics từ DB trước theo đặc tả, DB trống mới lấy tệp.
- Áp bìa cho nhóm đánh dấu `custom`; bìa custom đi theo bài khi album bị dọn.
- `GroupEdit` chỉ có sáu trường của đặc tả 1.8, kiểu chặn ghi đè tiêu đề cả album.
- Mỗi bài một transaction thay vì lô `BATCH_ROWS`, để báo cáo đúng từng bài.

**Lệch so với plan**: 691 dòng mã (ước 600). Không tách vì task chỉ sở hữu một tệp và `lib.rs` đã khoá. Thêm `tests/metadata_ops.rs`.

**Bẫy**: `indexer::apply_scanned_track` luôn lấy thư mục cha trực tiếp làm `folder_id`, dòng DB cũ có thư mục khác sẽ chuyển theo. Luồng chạy tuần tự, lô dài làm lần lưu sau phải chờ. Lỗi `DatabaseAfterWrite` để dấu tệp lệch DB có chủ ý, để watcher đọc lại.

**Soi cải tiến**: bảo mật: đường dẫn kiểm trước khi chạm đĩa, `create_new` chặn đè kể cả khi đích vừa xuất hiện, log không chứa nội dung thẻ. RAM: một luồng ngủ, lô dùng lại một bản bìa, album nhiều bài chỉ giải mã bìa một lần. UI/UX: lỗi tách theo biến thể; chữ là việc của 19, 20. Pro: không áp. Không có đề xuất treo.

**Việc chưa làm**: nối UI (19, 20). `set_category_cover` thuộc `LibraryCtl` (17). Watcher bỏ qua theo dấu tệp là của 09.
