# groove-gd1_19 — metadata-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~850 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_18, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/metadata/{dialog,fields,cover_pane,raw_frames}.rs`, `i18n/*/metadata.toml` |

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `player::install_for_test(PlayerDeps { library, output: Box::new(FakeOutput::new(48_000, 2)), options, session_file }, cx)` rồi `player::player(cx)` như thường, kèm `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác; cài đặt của nó là bản cục bộ.
- **Sau khi lưu**: bài đang phát thì gọi `refresh_current(cx)` nếu đường ghi không phát `LibraryEvent::Synced`; bài có `updated > 0` thì player tự làm. Tệp ngoài: `current_song().track` là `External`, bìa ở `external_cover()`, sửa thẻ ghi tệp, không có phần DB (N6).

## 0. Bàn giao từ groove-gd1_18 (02/11/2026)

<!-- handoff: groove-gd1_18 -->

- **Đã có sẵn để dùng**: `metadata_ops::MetadataOps`: `read(TrackRef)` cho dialog (đủ `fields`, `cover`, `raw_frames`, `filename`), `save(track, edit, mode, new_filename)`, `save_bulk(category, GroupEdit, mode)`. Gọi trên UI rồi `await`. API đủ: `plan/groove-gd1_18_metadata-ops.md` mục 9.
- **Tên tệp**: truyền tên đủ kèm đuôi; `None` là không đổi tên. `validate_file_name(raw, current)` không chạm đĩa, gọi khi gõ để báo sớm. Tệp trùng tên chỉ biết lúc lưu: `MetadataError::TargetExists`.
- **Áp cho nhóm**: lưu bài đang mở bằng `save`, rồi `save_bulk` với `GroupEdit` (sáu trường của đặc tả). `Clean` tự hạ ở lô. Đổi bìa nhóm chỉ qua đây; `save` một bài chỉ gieo khi nhóm chưa có bìa (S8).
- **Báo kết quả lô**: `BulkReport { done, failed, not_started, group_cover_failed }`, nêu tên từng bài trong `failed`. Tiến độ: `LibraryEvent::Progress { kind: MetadataBulk, .. }`.
- **Lỗi cần câu riêng trong `metadata.toml`**: từng biến thể `FileNameError`, `TargetExists`, `RenameFailed` (thẻ đã lưu, tên chưa đổi), `DatabaseAfterWrite` (thẻ đã lưu, thư viện sẽ cập nhật sau), cùng các `TagError` ở bàn giao 06. `error.file_written()` để biết thẻ đã ghi chưa.
- **Làm mới**: `done == total` của `Progress` kind `MetadataBulk` là tín hiệu nạp lại danh sách. Sửa trong app không phát `Synced`, không toast "N bài cập nhật". Đổi tên phát thêm `Moved`.
- **Soát lại khi làm**: `cargo test -p groove-library --test metadata_ops`.

## 0. Bàn giao từ groove-gd1_06 (02/11/2026)

<!-- handoff: groove-gd1_06 -->

- **Đổi so với bản cũ**: `RawFrame { id, description, value, known }` không còn `name`: nhãn khung tra theo `id` qua i18n vùng `metadata`, `description` là mô tả của `TXXX`. Số khung rác là số dòng `known == false`; `TXXX:TIMELINE` có `known == true`.
- **Bìa**: dùng `CoverEdit { Keep, Remove, Set(Vec<u8>) }`, không còn `data:` URI.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 19. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).
