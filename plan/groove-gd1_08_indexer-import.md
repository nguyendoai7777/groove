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

## 0. Bàn giao từ groove-gd1_06 (02/11/2026)

<!-- handoff: groove-gd1_06 -->

- **Đã có sẵn để dùng**: `tags::read_track(path) -> ScannedTrack { fields, cover, duration_ms, stamp }`, gom một lần đọc (S14), bọc `catch_unwind`. `fixtures::write_sample` sinh tệp mẫu cho test. Chi tiết: mục 9 của `groove-gd1_06`.
- **Đọc thẻ**: tệp có âm thanh mà thẻ hỏng vẫn `Ok` với trường trống; không phải âm thanh thì `Err`. Không có tệp thì `TagError::NotFound`.
- **Số**: `track_no`, `disc_no` là `Option<String>` như trong thẻ, cột INT tự parse. `FileStamp.mtime_ms` là mili giây; cột `file_mtime` của DB cũ là giây, nên chọn một đơn vị khi so (S6).
- **RAM**: `cover` là byte đầy đủ; tính `cover_hash`, bìa thu nhỏ xong nhả ngay, đừng giữ qua kênh tới actor.
- **Watcher (task 09)**: bỏ qua bản tạm bằng `tags::is_temp_path`; đuôi nhạc ở `groove_core::model`.
- **Bẫy**: mẫu OGG có setup giả, chỉ dùng để thử thẻ.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 08. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).