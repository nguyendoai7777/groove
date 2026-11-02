# groove-gd1_16 — song-list

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~700 dòng |
| Phụ thuộc | groove-gd1_03, groove-gd1_13 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{song_list,song_row}.rs`, `i18n/*/song_list.toml` |

## 0. Bàn giao từ groove-gd1_13 (02/11/2026)

<!-- handoff: groove-gd1_13 -->

- **Đã có sẵn để dùng**: `crate::player::player(cx) -> Entity<Player>`. Đọc `.read(cx)`, thao tác `.update(cx, |p, cx| p.next(cx))`, nghe `cx.observe` (mọi đổi trạng thái, 4 lần mỗi giây khi phát) và `cx.subscribe` (`PlayerEvent`). Danh sách API: mục 9 của `plan/groove-gd1_13_player-controller.md`.
- **Test view**: `player::install` dùng thiết bị và tệp thật. Test dùng `player::install_for_test(PlayerDeps { library, output: Box::new(FakeOutput::new(48_000, 2)), options, session_file }, cx)` rồi `player::player(cx)` như thường, kèm `cx.executor().allow_parking()` vì engine, thư viện đánh thức từ luồng khác; cài đặt của nó là bản cục bộ.
- **Danh sách bài**: bấm dòng `play_list(track, list, cx)`, bấm đúng bài đang phát tự giữ tiếng (S2). Dòng đang phát so `current_track()`; bars theo `is_playing()`.

## 0. Bàn giao từ groove-gd1_15 (02/11/2026)

<!-- handoff: groove-gd1_15 -->

- **Đã có sẵn để dùng**: `views/library_page.rs` (`LibraryPage`) đã ở `Slot::Library`; vào danh sách bằng `LibraryPageEvent::Open` → `app::navigate(Route::Category(ref))`. `app::library(cx)` trả `Library` dùng chung (`songs(CategoryRef)`, `category(CategoryRef)` cho đầu danh sách: bìa, accent, tên, `song_count`). Chi tiết: `plan/groove-gd1_15_library-grid.md` mục 9.
- **Không gắn `SongList` vào slot ở task này**: nhánh `Route::Category(..)` trong `LibraryPage::render` đang là chữ giữ chỗ và là việc của task 17 thay bằng view `SongList` bạn xuất (task 15 không sửa được tệp của 16, 16 không sửa `library_page.rs`). Xuất `SongList::new(reference, cx)` (hoặc tương đương) thật gọn để một dòng là gắn được.
- **Dùng lại**: `views::cover_image::{CoverImage, CoverKind, parse_color}`, `views::category_card::{accent_of, cover_path}` cho bìa 160 / 80 px và quầng accent ở đầu danh sách.
- **Bẫy đã gặp**: `occlude()` làm `group_hover` tắt; test view với thư viện thật cần `cx.executor().allow_parking()`.

## 0. Bàn giao từ groove-gd1_03 (02/11/2026)

<!-- handoff: groove-gd1_03 -->

- **Chỗ đặt**: danh sách bài của một category nằm trong `Slot::Library` cùng task 15 (đặc tả 1.2); task 15 sở hữu `library_page.rs` và là bên gắn `SongList` vào slot, task này chỉ cần xuất view `SongList` nhận `CategoryRef`. Điều hướng vào / ra: `app::navigate(Route::Category(..), cx)`, `app::go_back(cx)`.
- **Chữ ký ở `app::`** (chi tiết `plan/groove-gd1_03_app-shell.md` mục 9): `Slot::{PlayerBar, Library, NowPlaying}`, `install_slot(slot, view, cx)`, `navigate(Route, cx)`, `current_route(cx)`, `settings(cx)`, `update_settings(cx, |s| …)`, `shell(cx)` phát `ShellEvent`.
- **Icon sẵn**: `GrooveIcon::{Play, Artist, Song, Edit, LyricsBubble, Delete}`.
- **Bẫy**: `#[derive(IntoElement)]` ở bản không test cần `use materiant::gpui_kit as gpui;`.

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 16. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 8. Nợ / việc cho task khác

- **Task 17**: gắn `SongList` vào `LibraryPage::render` (thay chữ giữ chỗ `category_pending`, `library_page.rs` không thuộc task 16), nghe `SongListEvent`, vẽ menu bài (`song_menu.rs`), xoá khỏi playlist, toast "đã thêm". Task 16 không sửa `library_page.rs`, `app.rs`, `player.rs`.
- Không có chỗ nào phải quyết định nghiệp vụ ngoài mục 9.

## 9. Bàn giao thực tế (02/11/2026)

**API cho task 17** (`views/song_list.rs`):

- `SongList::new(reference: CategoryRef, cx: &mut Context<SongList>) -> SongList`, dùng `app::try_library` + `CoverStore::default_location`; gắn bằng `cx.new(|cx| SongList::new(reference, cx))`. Cần `player::install` và `app::set_library` chạy trước. Test dùng `SongList::with_deps(library, covers, reference, cx)`.
- `set_reference(reference, cx)` đổi category trong cùng view (đặt lại cuộn, đầu danh sách mở); `reference()`; `focus(window, cx)` cho ↑ ↓ Enter.
- `SongListEvent` (`cx.subscribe(&list, ..)`): `ContextMenu { reference, song: Box<Song>, position }` (chuột phải; dòng đó đã thành dòng chọn), `CategoryGone(reference)` (mục không còn: nên `app::go_back`).
- Nút Play all của đầu danh sách do view tự làm (`player.play_list(bài đầu, cả danh sách)`; rỗng thì toast `song_list.empty_play.message`, nút còn bị khoá khi rỗng). Bấm dòng: bài đang chọn thì `toggle_play`, bài khác thì `play_list(bài, cả danh sách)`; Enter giống; ↑ ↓ chỉ di con trỏ.
- `views/song_row.rs`: `SongRow`, `RowPlay`, `header_row`, `display_title`, `display_artist`, `format_duration`, `show_artist`; dùng được cho view khác (hàng đợi task 14 / 21 nếu cần).
- i18n `song_list.*` đủ vi + en (loại, nhãn đầu, cột, đang tải, rỗng, lỗi, mục biến mất, toast rỗng).

**Opus (Sonnet) chọn 02/11/2026**: (1) bấm dòng dùng `on_select` của `VirtualList` kèm `window.last_input_was_keyboard()` để tách chuột với ↑ ↓; nhấp đúp (`on_activate`) bỏ qua khi từ chuột, tránh đảo phát / dừng hai lần. (2) Đầu danh sách là khối đứng ngoài `VirtualList` có cao cố định, thu / mở tức thì (không nội suy); thu chỉ khi sau khi thu vị trí cuộn còn > 15 px (danh sách ngắn không thu, tránh nhấp nháy). (3) Playlist không hiện dòng phụ (chỉ "n bài"), album là nghệ sĩ, thư mục là đường dẫn. (4) Bìa đầu danh sách dùng bản 256 px cho cả hai cỡ (bản 512 px không chắc có tệp). (5) Cột nghệ sĩ ẩn khi cửa sổ < 768 px (breakpoint `md` của bản cũ, tính theo cửa sổ, không theo vùng nội dung). Ghi vào `plan/confirm.md`.

**Case** (`cargo test -p groove-app song_`, 20 test, xanh, chạy 3 lần liên tiếp không flake): C1 nạp theo thứ tự tên tệp + đầu danh sách (`loads_songs_in_filename_order`); C2 bấm dòng phát với cả danh sách; C3 ↓ không phát, Enter phát; C4 Play all, rỗng không đổi hàng đợi; C5 chuột phải phát `ContextMenu`; C6 3000 bài chỉ dựng < 40 dòng; C7 thu / mở đầu danh sách theo cuộn; C8 nạp lại khi thư viện đổi, `CategoryGone` khi mục biến mất; thêm test thuần cho hai ngưỡng, tiêu đề / nghệ sĩ / thời lượng, khoá i18n. Dùng thư viện tạm + `FakeOutput`, không phát tiếng, không mở DB thật.

**Soát**: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (xanh trừ một lần `player::tests::volume_and_mute_reach_the_engine` đỏ khi máy tải nặng ở lần chạy đầu, hai lần sau xanh; test của task 13, không đụng tới), `cargo deny check licenses` xanh. Không thêm phụ thuộc.

**Dùng lại của task 14**: `PlayingBars` và `GrooveIcon::Pause` (đã có trên `master` lúc gộp).

**Chờ kiểm tay**: `[owner-qa]` nhìn danh sách thật (ghi ở `plan/_backlog.md`); máy dev chỉ một màn nên không mở cửa sổ.

**Soi cải tiến**: bảo mật không áp (chỉ đọc; đường dẫn bìa đi qua `cover_path` đã lọc hex). RAM: bìa đầu danh sách cache 4 ô, dòng chỉ dựng trong khung, player báo 4 lần mỗi giây nhưng view chỉ vẽ lại khi bài hoặc trạng thái phát đổi. UX: dùng hết bằng bàn phím (Tab vào danh sách, ↑ ↓ PageUp PageDown Home End Enter); chuột phải chưa có phím tương đương (menu phím Menu / Shift+F10 là việc của task 17, ghi ở bàn giao). Pro: không áp.
