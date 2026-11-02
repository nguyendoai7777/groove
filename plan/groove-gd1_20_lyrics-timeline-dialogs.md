# groove-gd1_20 — lyrics-timeline-dialogs

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
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
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `Player::spawn(PlayerDeps { library, output: Box::new(FakeOutput), .. }, cx)` với `app::update_settings` đã có, và `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác.
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
