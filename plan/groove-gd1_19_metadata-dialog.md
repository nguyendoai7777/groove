# groove-gd1_19 — metadata-dialog

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
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

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 19. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 8. Nợ / việc cho task khác

Chỗ cần quyết định nghiệp vụ, Sonnet chọn phương án an toàn nhất và ghi lại để Opus soát (cũng có ở `plan/confirm.md`):

- **Áp cho nhóm chỉ gửi trường người dùng đã đổi** (so với lúc mở), không gửi cả sáu trường như bản cũ `:574-590`. Lý do: `GroupEdit` ghi đè mọi bài trong nhóm; gửi cả trường không đụng sẽ xoá thể loại / năm riêng của bài khác. Hệ quả: Fix grouping vẫn đi (album artist, tuyển tập đổi nên được gửi).
- **Bài đang mở lưu bằng chế độ người dùng chọn (kể cả `Clean`), bài khác `Clean` hạ thành `clean_keep`** (đã làm ở `save_bulk`, dialog cũng hạ sẵn). Đúng đặc tả 1.8.
- **Ô áp cho nhóm chỉ hiện khi `GroupScope.songs > 1`**; chủ (task 17) quyết `scope` nào truyền: album / thư mục có `songs` thật. Playlist: `apply_group_cover` bỏ qua, các trường vẫn áp; task 17 nên truyền `None` cho playlist nếu không muốn.
- **Dán ảnh**: Ctrl+V (Cmd+V) chỉ dán ảnh khi chuột đang trên ô bìa **và** clipboard có ảnh hoặc tệp ảnh; còn lại ô nhập tự dán chữ.
- **Nhãn chế độ `merge` là "Gộp vào thẻ hiện có"**, vì form gửi mọi trường (đặc tả) nên "chỉ sửa trường đã đổi" là sai nghĩa.
- **Bài `not_started` chỉ hiện số lượng**, không tên: `BulkReport.not_started` chỉ có id, tra tên cần đọc DB mà thư viện đang đóng.
- **Không kiểm số ở Track / Disc / Year / BPM**: đặc tả không nêu luật; `tags` ghi chuỗi nguyên văn. Nếu muốn lọc chữ số thì đổi sang `NumberField`.
- **Chủ cần làm**: task 17 mở dialog từ menu bài (`MetadataDialog::open`); nơi nào mở thì tự giữ `MetadataOps` (`MetadataOps::new(library, CoverStore)`, một bản dùng chung, `Clone`).

## 9. Bàn giao thực tế (02/11/2026)

**Kiểm** (cwd worktree, `CARGO_TARGET_DIR=groove/target/gd1_19`): `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (33 test của `views::metadata`, cả workspace xanh), `cargo deny check licenses` xanh. Không phụ thuộc mới. Test dùng thư mục tạm, `FakeOutput`, thư viện tạm: không phát tiếng, không mở DB thật, không ghi nhạc thật.

**API đã khoá** (`views::metadata`):

- `dialog::MetadataDialog::open(ops: MetadataOps, track: TrackRef, scope: Option<GroupScope>, window, cx) -> Entity<MetadataDialog>`. Chủ **giữ entity và vẽ nó làm con** trong cây của mình (`Dialog` của materiant là khai báo, mở ngay lúc `open`); nghe `cx.subscribe(&entity, ..)`:
  - `MetadataDialogEvent::Saved(Box<SavedTrack>)`: bài đang mở đã ghi xong, nạp lại danh sách (`SavedTrack.song`, `.path`, `.previous_path`).
  - `MetadataDialogEvent::Closed`: phát đúng một lần khi đóng bằng mọi đường (Esc, nút, lưu xong); chủ bỏ entity.
- `GroupScope { category: CategoryRef, songs: usize }`: truyền khi mở từ mục nhiều bài; `songs > 1` mới hiện ô áp cho nhóm.
- Đọc / điều khiển cho test và chủ: `is_ready`, `load_error`, `can_save`, `save`, `error`, `bulk_report`, `set_mode`, `set_apply_group`, `fix_grouping`, `set_field`, `type_filename`, `close`.
- Hàm thuần (đã test): `error_text(&MetadataError)` (câu riêng cho mọi biến thể), `describe_bulk(&BulkReport)`, `fields::{split_file_name, strip_known_extension, new_file_name, group_edit, edit_for_track, needs_grouping_fix}`, `raw_frames::{frame_label, junk_count}`, `cover_pane::{sniff_format, check_cover_bytes, read_cover_file}`.
- Cần có `player::player(cx)` đã cài khi lưu thành công (gọi `refresh_current`). Dialog chỉ gọi khi ghi tệp xong (kể cả `RenameFailed` / `DatabaseAfterWrite`).
- i18n `metadata.*` đủ vi + en: tiêu đề, tab, nhãn trường, bìa, cảnh báo grouping, chế độ ghi, ô nhóm, khung thô (`frame.<ID>`), từng `FileNameError` (`name_error.*`), từng lỗi (`error.*`), báo cáo lô (`bulk.*`).

**Opus soát**: lỗi thật bắt được nhờ test và đã sửa: ô ở tab chưa vẽ không nhận `default_value` (chỉ áp lúc vẽ khung), nên lưu từ tab Chung xoá sạch trường của tab Chi tiết; đổi sang `set_value` ngay khi tạo ô (test `applies_only_changed_fields_to_the_rest_of_the_group` giữ). Bảo mật: tệp bìa kiểm cỡ (32 MB) và chữ ký trước khi nhận, đọc ở luồng nền; không có tên bài trong log. RAM: asset ảnh gỡ khi đổi ảnh và khi đóng ô bìa. UX: tên tệp báo lỗi ngay khi gõ; Esc bị chặn khi đang lưu; nền bấm không đóng (tránh mất bản sửa); thân dialog cuộn được khi cửa sổ thấp. Pro: không áp.

**Chờ kiểm tay** (`[owner-qa]`, `plan/_backlog.md`): nhìn dialog thật, bố cục 900 px, kéo thả và Ctrl+V bìa, chọn tệp bằng hộp thoại OS, cuộn thân khi cửa sổ thấp, rê chuột tô khung bìa.

**Chưa làm**: nối mở dialog từ menu bài / thẻ / thanh phát (task 17, 21). Kéo thả tệp từ OS, hộp thoại chọn tệp, hover thật không kiểm được bằng `#[gpui::test]`.
