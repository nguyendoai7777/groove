# groove-gd1_04 — db-migration

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `library/db/{schema,migrate}.rs`, `crates/groove-library/tests/migrate_legacy.rs` (MỚI) |

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Đã có**: `paths::library_db`, `paths::legacy_db`, `paths::normalize` (cột `path_key`, M2). `base64` đã khai cho bìa cũ. Thử bằng `GROOVE_DATA_DIR`, không đụng DB thật.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 04. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 6. Nghiệm thu
Phủ bằng `crates/groove-library/tests/migrate_legacy.rs`, fixture dựng bằng SQL theo `src-tauri/src/db.rs:124-228`, dữ liệu giả. `[opus-unit]`, đều ✅.
- C1. Giữ nguyên `id` của songs, albums, folders, playlists và mọi cột cũ.
- C2. Tệp cũ giống hệt từng byte, không sinh tệp cạnh nó.
- C3. Base64 đi qua `CoverSink`, ảnh trùng lưu một lần, bỏ `thumbnail`, `accent_color`, accent không phải hằng cũ. Ảnh hỏng, sink lỗi, sink panic chỉ mất bìa dòng đó.
- C4. `tags_read_at IS NULL` mọi bài, `search_key` tính ngay.
- C5. `group_key` là `folder:<nhỏ nhất>`, album rỗng `folder:0`; S1 vào được cùng tên khác nhóm.
- C6. `position` theo tên tệp, `added_at` tính bằng ms.
- C7. Trùng `path_key` gộp về `id` nhỏ nhất, lấp lyrics, timeline, playlist theo dòng thắng (M2).
- C8. DB của bản Tauri cũ thiếu cột, thiếu bảng vẫn chuyển được.
- C9. Dòng mồ côi bị bỏ, `foreign_key_check` sạch.
- C10. `user_version > 1` bị từ chối, tệp không đổi; tệp lạ cũng không đụng.
- C11. Chuyển hỏng không để lại gì, tệp cũ nguyên, `open_fresh` vẫn mở được; `.migrating` sót lại bị thay.
- C12. Mở lần hai không chuyển lại.
- C13, C14. Tạo mới đúng schema, pragma, index; bản chuyển và bản mới cùng `sqlite_master`.
- C15. Khoá ngoại SET NULL, CASCADE chạy.
- C16. Base64 không ở lại trong cỡ tệp.
- C17. 1 002 bài chạy 88 ms (debug, không bìa).

## 8. Nợ chuyển task khác
- 07 hiện thực `migrate::CoverSink`.
- 05 gọi `migrate::open_default`, xử lý `LegacyFailed`, `TooNew`.
- 08 làm tác vụ `tags_read_at IS NULL`: gán lại album theo `schema::album_group_key`, điền `file_size`, `cover_hash`.

## 9. Bàn giao thực tế
- Soát 02/11/2026 trên nhánh sau khi gộp `master`, `-p groove-library`: build, clippy `-D warnings`, 28 test (8 đơn vị + 20 tích hợp), `cargo fmt --all --check`, `cargo deny check licenses` đều xanh. Không mở DB thật, chưa chạy với DB thật.
- API đã khoá, `groove_library::db`:
  - `schema`: `SCHEMA_VERSION`, `create_schema`, `apply_pragmas`, `user_version`, `normalize_text`, `search_key`, `album_group_key`, `path_key`, `NO_FOLDER_ID`.
  - `migrate`: `open_default`, `open_or_migrate`, `open_fresh`, `migrate_legacy`, `CoverSink`, `CoverRecord`, `MigrationReport`, `OpenOutcome`, `MigrateError`.
- Opus chọn 02/11/2026:
  - Thêm `songs.path_key` UNIQUE (M2, handoff 01), bỏ UNIQUE trên `file_path` vì thừa. Đặc tả 2.2 chưa ghi cột này.
  - Trùng `path_key` trong DB cũ: gộp về `id` nhỏ nhất, vì nếu không UNIQUE làm hỏng cả chuyển đổi. Đây là ngoại lệ duy nhất của "giữ nguyên `id`".
  - Chụp bằng `VACUUM INTO` từ kết nối chỉ đọc thay vì chép tệp, nhất quán cả khi bản Tauri đang mở. Làm trên `library.db.migrating`, xong mới đổi tên.
  - `position` theo tên tệp, vì bản cũ hiện playlist theo `filename` (`db.rs:536`) và không lưu thứ tự.
  - `CoverSink` thay vì tự ghi tệp bìa: `covers.rs` thuộc task 07. Bìa hỏng không dừng chuyển đổi, tác vụ đọc thẻ lấy lại bìa từ tệp nhạc.
  - DB cũ hỏng trả `LegacyFailed`, không tự tạo thư viện trống; caller chọn `open_fresh`.
  - `search_key` giữ dấu câu, `AC/DC` không còn khớp `acdc` như bản cũ, theo S18.
  - Dòng mồ côi bị bỏ, đếm ở `orphan_rows`.
- Chưa làm: tác vụ đọc lại thẻ (task 08), tệp bìa thật (07), truy vấn (05). Chưa nối vào app.

## 10. Đề xuất cải tiến
- Bảo mật: log chỉ ghi số đếm và lỗi, không ghi base64; tệp cũ mở chỉ đọc.
- UI/UX: không áp, thông báo lỗi do task 05, 17 hiển thị.
- RAM: chỉ chạy một lần; `seen` giữ chuỗi base64 tới hết chuyển đổi, nhả ngay sau đó.
- Pro: không áp.
