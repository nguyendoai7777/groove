# groove-gd1_05 — library-store

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
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
Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 05. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).