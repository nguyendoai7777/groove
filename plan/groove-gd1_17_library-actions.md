# groove-gd1_17 — library-actions

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | ~750 dòng |
| Phụ thuộc | groove-gd1_08, groove-gd1_15, groove-gd1_16 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/library_ctl.rs`, `views/{song_menu,playlist_dialog,card_menu}.rs`, `i18n/*/actions.toml` (N9, S2, S3) |

## 0. Bàn giao từ groove-gd1_19 (02/11/2026)

<!-- handoff: groove-gd1_19 -->

- **Đã có sẵn để dùng**: `views::metadata::dialog::MetadataDialog::open(ops: MetadataOps, track: TrackRef, scope: Option<GroupScope>, window, cx) -> Entity<MetadataDialog>`. Menu bài, menu thẻ, thanh phát mở nó cho mục "Sửa thông tin". Chi tiết: mục 9 của `plan/groove-gd1_19_metadata-dialog.md`.
- **Cách gắn**: dialog của materiant là khai báo ⇒ chủ giữ `Option<Entity<MetadataDialog>>` trong view của mình, vẽ nó làm con (`div().children(dialog.clone())`), `cx.subscribe` hai sự kiện: `Saved(Box<SavedTrack>)` để nạp lại danh sách, `Closed` để gán `None`. Cửa sổ phải bọc `Root`.
- **`MetadataOps`**: tạo một lần `MetadataOps::new(library, CoverStore)` (`Clone`), đặt vào global hoặc `LibraryCtl`; dialog không tự tạo.
- **`GroupScope`**: truyền `Some(GroupScope { category, songs })` khi mở từ album / thư mục có nhiều bài (ô áp cho cả mục chỉ hiện khi `songs > 1`); mở từ dòng bài trong danh sách playlist thì truyền `None` (bìa playlist không có, task 19 chưa lọc).
- **Sau khi lưu**: dialog tự gọi `refresh_current` của player; danh sách thì chủ nạp lại từ `Saved`. Lô nhóm phát `Progress { MetadataBulk }`, không phát `Synced`.
- **Việc của task này**: `set_category_cover` của `LibraryCtl` vẫn thuộc task này; đổi bìa nhóm chỉ đi qua dialog của 19.

## 0. Bàn giao từ groove-gd1_16 (02/11/2026)

<!-- handoff: groove-gd1_16 -->

- **Đã có sẵn để dùng**: `views::song_list::{SongList, SongListEvent}`: `cx.new(|cx| SongList::new(reference, cx))`, đổi mục bằng `set_reference(reference, cx)`. Chi tiết API: `plan/groove-gd1_16_song-list.md` mục 9.
- **Phải nối**: trong `LibraryPage::render` thay nhánh `Route::Category(..)` (chữ giữ chỗ `category_pending`) bằng view `SongList`; tạo một lần, đổi mục bằng `set_reference`, gọi `focus` khi vào. Nghe `SongListEvent::CategoryGone` thì `app::go_back`.
- **Menu bài**: `SongListEvent::ContextMenu { reference, song: Box<Song>, position }` đã chọn sẵn dòng; vẽ `song_menu.rs` tại `position`; `reference` là `CategoryRef::Playlist(_)` thì thêm "Xoá khỏi danh sách phát" (chữ đỏ). Danh sách chưa có phím mở menu bằng bàn phím (Menu / Shift+F10).
- **Đã làm sẵn, đừng làm lại**: Play all của đầu danh sách (kèm toast khi rỗng), bấm dòng phát / đảo phát dừng, Enter, nạp lại khi `Synced` / `PlaylistsChanged` / `Moved`.
- **Bẫy đã gặp**: `VirtualList::on_select` bắn cả khi ↑ ↓ lẫn khi bấm chuột; tách bằng `window.last_input_was_keyboard()`. Test view cần `cx.executor().allow_parking()` và `player::install_for_test`; dòng đã có `debug_selector` `song-row-<số thứ tự từ 1>`.

## 0. Bàn giao từ groove-gd1_09 (02/11/2026)

<!-- handoff: groove-gd1_09 -->

- **Phải nối**: giữ `watcher::LibraryWatcher::start(indexer)` suốt phiên trong `library_ctl`, trước `Library::shutdown` gọi `stop`. `start` tự gắn watcher rồi tự so đĩa với DB, đừng gọi `reconcile` thêm. Sau import, bỏ gốc: `refresh_roots()`.
- **N9 đã có dữ liệu**: schema bản 2 có `excluded_paths`. `queries::{add_excluded_path, remove_excluded_path, excluded_paths, is_excluded}`. Quét, watcher, import gốc cha đều tự bỏ qua chỗ đã thêm; hàm chỉ ghi dòng, không xoá bài: xoá bài dưới đó trong cùng transaction.
- **Import đổi**: `ImportReport.removed` thành `marked_missing`, thêm `moved`, `restored`. Toast "đã xoá" giờ nghĩa là "không còn thấy tệp"; playlist vẫn giữ.
- **Bài đang mất** ẩn khỏi mọi danh sách, 30 ngày mới dọn. Muốn cho người dùng dọn ngay: `reconcile::{missing_songs, forget_missing}`.
- **Sự kiện mới phải nghe**: `LibraryEvent::Moved` tới cùng `Synced` khi đổi tên ngoài app. Mở app không đổi gì thì không có sự kiện, không toast.
- **Soát lại khi làm**: `cargo test -p groove-library --test watcher_reconcile`.
- Chi tiết: `plan/groove-gd1_09_watcher-reconcile.md` mục 9.

## 0. Bàn giao từ groove-gd1_15 (02/11/2026)

<!-- handoff: groove-gd1_15 -->

- **Đã có sẵn để dùng**: màn lưới My Music / Playlists ở `views/library_page.rs` (`LibraryPage`, đã gắn vào `Slot::Library` trong `app::run` bằng `library_page::install(cx)`, hàm trả `Entity<LibraryPage>`); `views::{category_card,cover_image}`; global thư viện `app::{set_library, library, try_library}`. Chi tiết: `plan/groove-gd1_15_library-grid.md` mục 9.
- **Nợ chuyển sang đây**: nhánh `Route::Category(..)` trong `LibraryPage::render` đang là chữ giữ chỗ; thay bằng `SongList` của task 16 (task 16 không sở hữu `library_page.rs`).
- **Sự kiện view phát** (`LibraryPageEvent`, nghe bằng `cx.subscribe(&page, ..)`): `Open(CategoryRef)` (đã nối thành `app::navigate`), `PlayAll(CategoryRef)`, `Delete(CategoryRef)`, `Import`, `Rescan`, `CreatePlaylist`. Task này nghe `PlayAll` / `Delete` / `Import` / `Rescan` / `CreatePlaylist`, làm hộp thoại và nghiệp vụ, rồi báo ngược `set_importing(bool, cx)`, `set_rescanning(bool, cx)`; view tự nạp lại lưới khi nhận `LibraryEvent::{Synced, PlaylistsChanged}`. Tên lấy lại entity: `library_page::install` đã trả về; hoặc đăng ký nghe trong `install` bằng một lời gọi của task này ở `app::run`.
- **Bẫy đã gặp**: `occlude()` quanh nút trong lớp phủ thẻ làm `group_hover` tắt (nút biến mất dưới con trỏ); dùng `cx.stop_propagation()` trong `on_click`. Test view dùng thư viện thật ở thư mục tạm cần `cx.executor().allow_parking()` (luồng đọc đánh thức future từ thread khác), mẫu ở mục test của `library_page.rs`.
- **Soát lại khi làm**: không mở DB người dùng khi chạy thử hay test, dùng `LibraryConfig` thư mục tạm.

## 0. Bàn giao từ groove-gd1_08 (02/11/2026)

<!-- handoff: groove-gd1_08 -->

- **Đã có sẵn để dùng**: `groove_library::indexer::Indexer::standard(library)`, sao chép rẻ; `import(path)`, `rescan()`, `backfill_tags()` trả về ngay `IndexJob<T>`, `await` trên UI. Chọn thư mục ở UI, `LibraryCtl::import` chỉ nối vào đây. Chi tiết: `plan/groove-gd1_08_indexer-import.md` mục 9.
- **Phải làm**: gọi `backfill_tags()` mỗi lần mở app sau khi thư viện mở xong; không có gì thì xong ngay (đặc tả 2.3 bước 5).
- **Toast import**: `ImportReport.tally.added`, `removed.len()`. Nên báo thêm khi `root_offline` (gốc rỗng mà còn bài, không xoá gì), khi `failed_dirs` không rỗng, khi `tally.failed` không rỗng. `Err(IndexError::RootUnavailable)` là thư mục không đọc được.
- **Toast rescan**: `tally.read()`, `tally.missing.len()`; `tally.failed` không phải là thiếu.
- **Sự kiện**: `Progress { kind: Import | Rescan | TagBackfill, done, total }` mỗi lô, xong `Synced`. Nhận `Progress` là đã đọc được lô đó. Import toàn tệp không đổi có `total = 0`, không có `Progress`, chỉ có `Synced`.
- **N9**: bỏ gốc đang chưa có `excluded_paths`; import lại vẫn thêm gốc qua `add_watched_root`.

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Playlist**: `Library::{create_playlist, delete_playlist, add_song_to_playlist, remove_song_from_playlist}`, tự phát `PlaylistsChanged`. Lỗi tên: `LibraryError::Query(QueryError::{EmptyName, DuplicatePlaylist})`, hiển thị bằng i18n, không in lỗi thô.
- **Bìa tự chọn**: `queries::set_album_cover(.., custom = true)`, `set_folder_cover`, `upsert_cover`.
- **N9 chưa đủ dữ liệu**: `queries::{delete_folder, delete_album, add_watched_root, remove_watched_root}` có sẵn nhưng schema chưa có `excluded_paths`, phải thêm cột hoặc chọn cách của đặc tả (bỏ gốc, đổi nhãn nút).

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 17. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Sửa dứt điểm các lỗi N9, S2, S3 theo mục 3 của đặc tả, mỗi lỗi có test tái hiện.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 8. Nợ / việc cho task khác

- Thanh phát (task 21 hoặc sau) chưa có mục "Sửa thông tin": gọi `library_ctl::ctl(cx)` rồi `open_metadata`.
- Phím Menu của bàn phím chưa gán: materiant chỉ gán Shift+F10 cho `OpenContextMenu`.
- `MenuItem` của materiant không có icon màu tuỳ ý: menu con playlist chưa có chấm màu của đặc tả 1.2; cần thêm ở materiant.

## 9. Bàn giao thực tế (02/11/2026)

**API đã khoá** (`app/library_ctl.rs`):

- `library_ctl::ctl(cx) -> Option<Entity<LibraryCtl>>`; `install(library, covers, &page, cx)` do `library_page::install_with` gọi, `app::run` không thêm dòng nào.
- `LibraryCtl`: `pick_and_import`, `import_path(path)`, `rescan`, `forget_root(path)`, `play_category(reference)`, `request_delete(reference)` (qua hộp xác nhận), `remove_category(reference)`, `open_create_playlist(Option<song_id>)`, `add_to_playlist(playlist, song)`, `remove_from_playlist(playlist, song)`, `pick_category_cover(reference)`, `set_category_cover(reference, path)`, `open_metadata(track, from)`, `open_category_metadata(reference)`, `open_text_dialog(SongEditKind, track)`, `playlists()`, `is_importing()`, `is_rescanning()`.
- `LibraryCtlEvent::Notice { kind, message }`, `connect` đổi thành toast.
- Hàm thuần: `remove_category_from(&library, reference) -> Removed { songs, root_removed }`, `forget_root_in`, `store_category_cover`, `import_notice`, `rescan_notice`.
- `with_window(cx, f)` luôn hoãn bằng `cx.defer`: `on_select` của menu chạy lúc cửa sổ đang bị mượn, `window.update` trực tiếp sẽ lỗi.
- `app::metadata_ops(cx)`: dùng bản đã có trên `master`; ở đây chỉ thêm `app::set_metadata_ops` cho test.
- `views::card_menu::{build, ConfirmDelete, delete_label}`, `views::song_menu::build`, `views::playlist_dialog::PlaylistDialog`.
- `LibraryPageEvent::CategoryGone(reference)`, `LibraryPage::{set_overlays, add_overlay, set_metadata_dialog, refresh_lists}`, `SongList::refresh`.

**Hành vi**

- N9: bỏ thư mục dưới gốc ghi `excluded_paths`, thư mục đúng là gốc thì bỏ gốc; bỏ album ghi từng tệp, kể cả bài đang mất. Không đụng tệp. Nhãn nút là "Bỏ khỏi thư viện", hộp xác nhận nói rõ tệp còn nguyên. Sau đó phát `Synced { removed }` để hàng đợi bỏ bài.
- S2: `play_category` luôn thay hàng đợi, trùng bài thì giữ tiếng. S3: đi theo `CategoryRef`.
- S13: menu thẻ có "Đổi ảnh bìa" (không có ở playlist) và "Sửa thông tin".
- Mở app: `backfill_tags`, `LibraryWatcher::start`. **Không gọi `reconcile` riêng**: lời giao có nêu reconcile, nhưng bàn giao của 09 ghi `start` đã tự so đĩa với DB; gọi thêm là quét hai lần.
- Thoát: luồng riêng dừng watcher rồi mới `Library::shutdown`; dòng `on_app_quit` cũ ở `library_page` đã chuyển sang đây.
- `importing`, `rescanning` = còn việc của ctl chưa xong, hoặc `Progress` chưa tới đích; `Synced` tắt phần `Progress`. Vì import không đổi gì không có `Progress`, chỉ nghe `Progress` thì ô chờ không hiện.
- Nạp lại: lưới và danh sách nghe `Moved`, `Progress { MetadataBulk, done == total }`; hộp metadata `Saved` gọi `refresh_lists`.
- Menu bài dựng trong `SongList` bằng `context_menu` của materiant, nên Shift+F10 có sẵn. `SongListEvent::ContextMenu` vẫn phát nhưng không còn ai cần để vẽ.

**Tệp ngoài dòng "File đụng tới"**: `views/library_page.rs`, `views/song_list.rs`, `views/category_card.rs`, `app.rs` (`set_metadata_ops` cho test), `groove-app/Cargo.toml` (`rfd`, `rusqlite`), `i18n/*/library.toml` (bỏ `category_pending`).

**Soát**: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check licenses`, `cargo test`, cwd = worktree; kết quả cuối ở báo cáo gộp. Test: thư viện thư mục tạm, `FakeOutput`, `Picker` giả. Chưa mở cửa sổ thật: `[owner-qa]` ở `plan/_backlog.md`. **Hộp chọn `rfd` thật chưa chạy lần nào** vì test dùng `Picker` giả.

**Bẫy**: gọi `window.update` từ `on_select` lỗi im lặng; `entity.update` chính mình trong closure gây panic lease (đã gặp ở `show_metadata`). Test chờ theo DB rồi bấm menu ngay thì menu còn dựng từ danh sách cũ: chờ dòng trên UI đổi đã.
