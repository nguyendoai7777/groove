# groove-gd1_20 — lyrics-timeline-dialogs

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~300 dòng |
| Phụ thuộc | groove-gd1_18, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{lyrics_dialog,timeline_dialog}.rs`, `i18n/*/song_dialogs.toml` |

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `player::install_for_test(PlayerDeps { library, output: Box::new(FakeOutput::new(48_000, 2)), options, session_file }, cx)` rồi `player::player(cx)` như thường, kèm `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác; cài đặt của nó là bản cục bộ.
- **Lyrics, timeline bài đang phát**: `lyrics()`, `timeline()` đã parse theo thời lượng thật; lưu xong gọi `refresh_current(cx)` để nạp lại. Tệp ngoài lấy từ thẻ.

## 0. Bàn giao từ groove-gd1_18 (02/11/2026)

<!-- handoff: groove-gd1_18 -->

- **Đã có sẵn để dùng**: `metadata_ops::MetadataOps`: `read(track)` nạp `fields.lyrics`, `fields.timeline` (từ tệp, tệp không có thì DB); `save_lyrics(track, text)`, `save_timeline(track, text)`. Gọi trên UI rồi `await`. API: `plan/groove-gd1_18_metadata-ops.md` mục 9.
- **Quy ước**: chuỗi rỗng là xoá cả tệp và DB. Timeline lưu văn bản nguyên gốc.
- **Lỗi (S5)**: `await` phải toast khi `Err`, không chỉ log; ghi tệp lỗi thì DB giữ bản cũ, nên đừng đóng dialog làm mất chữ đang gõ. Câu riêng theo biến thể trong `song_dialogs.toml`.
- **Làm mới**: nghe `Progress` kind `MetadataBulk`, `done == total`; không có `Synced`.
- **Soát lại khi làm**: `cargo test -p groove-library --test metadata_ops`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 20. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 8. Thiếu / chờ quyết định

- **Không có chỗ giữ `MetadataOps` chung ở app**: `app.rs` chỉ có `library()`; `CoverStore::default_location()` nằm riêng ở `player_bar`. Hộp thoại nhận `MetadataOps` qua tham số, chủ view (task 21, 19) tự dựng một bản `MetadataOps::new(app::library().clone(), CoverStore::default_location()?)` rồi chia sẻ (Clone). Mỗi `MetadataOps` là một luồng nên nên có đúng một bản; chỗ đặt (Global ở `app.rs`) là việc của Opus.
- **Ô timeline chưa mono**: đặc tả 1.6 ghi "ô mono", nhưng `materiant::field::TextArea` chưa có tuỳ chọn font. Giữ font mặc định, hướng dẫn khuôn dạng nằm dưới ô; thêm `font_family` cho `TextArea` là việc materiant.
- **Esc khi đang gõ**: giữ mặc định của `Dialog` (Esc và bấm nền đều đóng, mất chữ chưa lưu) như bản Tauri. Muốn chặn thì `close_on_escape(false)` + hỏi xác nhận: quyết định UX của chủ dự án.

## 9. Bàn giao thực tế

**Kết quả**: `cargo build`, `clippy --all-targets -D warnings`, `cargo test` (groove-app 86 test, 6 của `lyrics_dialog`), `cargo deny check licenses` xanh. Không phụ thuộc mới.

**API** (`views::lyrics_dialog`, dựng timeline ở `views::timeline_dialog`):

- `TextDialog::lyrics(ops, window, cx)`, `TextDialog::new(TextKind::{Lyrics, Timeline}, ops, window, cx)`, `timeline_dialog::timeline(ops, window, cx)`. Là `Render` entity: view gốc giữ `Entity<TextDialog>` và vẽ nó (Dialog tự trống khi đóng).
- `open(track, window, cx)` (đọc nền, ô khoá tới khi có chữ), `save`, `close`, `set_text`, `text(cx)`, `is_open`, `is_busy`, `kind`.
- `error_message(&MetadataError) -> &'static str`: một câu riêng mỗi biến thể (catalog `song_dialogs.error.*`).
- Hộp 500 px (`Dialog::max_width`), `TextArea` 12 dòng, nút Huỷ, Lưu, nút ✕.

**Hành vi chốt**: lưu OK ⇒ `Player::refresh_current` rồi đóng. Lỗi (S5) ⇒ toast Danger theo biến thể, hộp mở nguyên, giữ chữ, mở khoá ô. Lỗi đọc ⇒ toast rồi đóng. `TrackChanged` mà bài hiện tại khác bài đang sửa ⇒ đóng (cùng bài, như sau `refresh_current`, thì không). Mỗi lần mở / đóng tăng `generation` để kết quả cũ không đè lần mở mới. Chuỗi rỗng là xoá. Không toast thành công.

**Case đã phủ** (`#[gpui::test]`, DB và tệp tạm, `FakeOutput`): C1 mở nạp từ tệp, lưu ghi tệp + DB, `refresh_current` làm `Player::lyrics()` đổi; C2 lưu rỗng xoá lyrics; C3 ghi lỗi (tệp bị xoá) giữ hộp mở và chữ; C4 đổi bài đóng hộp; C5 timeline lưu nguyên văn, player parse; C6 mỗi biến thể lỗi một câu i18n. Test i18n sẵn có kiểm hai ngôn ngữ cùng khoá, không chữ hoa toàn.

**Soi cải tiến**: bảo mật: không log nội dung lời, chỉ lỗi; ghi qua `MetadataOps` (bản tạm, đổi tên). UI/UX: Esc, bấm nền đóng mất chữ (mục 8); nút Lưu loading khi ghi; toast có cách gỡ. RAM: hộp chỉ vẽ khi mở. Pro: không áp. Không có đề xuất mục 10.

**Chờ kiểm tay**: `[owner-qa]` ở `_backlog.md`: nhìn hộp thật.
