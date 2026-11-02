# groove-gd1_15 — library-grid

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~650 dòng |
| Phụ thuộc | groove-gd1_03, groove-gd1_05, materiant-gd5_04 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `views/{library_page,category_card,cover_image}.rs`, `i18n/*/library.toml` |

## 0. Bàn giao từ groove-gd1_05 (02/11/2026)

<!-- handoff: groove-gd1_05 -->

- **Đã có sẵn để dùng**: `store::Library` (sao chép rẻ, gửi được giữa luồng); mọi hàm trả về ngay `actor::Reply<T>`, là `Future`, UI `await`. `Reply::wait` chặn, chỉ dùng ở luồng worker. Chi tiết API: `plan/groove-gd1_05_library-store.md` mục 9.
- **Lưới**: `Library::categories()` trả `Categories { albums, folders }`, `playlists()` trả `Vec<CategorySummary>`, đã sắp theo tên không dấu. `CategorySummary`: `reference, name, artist, path, color, song_count, cover_hash, accent_color`; trường không áp cho loại đó là `None`. Bìa: `CoverStore::path(hash, size)`.
- **Trạng thái mở**: `Library::status()`, `OpenStatus::Failed(OpenFailure)` phải báo cho người dùng. `TooNew`: chỉ báo cần cập nhật app. `LegacyFailed`: hai nút `retry_open()` và `start_fresh()`. Lúc `Opening` việc đọc tự chờ, không lỗi.
- **Nạp lại**: nghe `LibraryEvent::{Opened, Synced, PlaylistsChanged}` qua `subscribe()`.

## 0. Bàn giao từ groove-gd1_03 (02/11/2026)

<!-- handoff: groove-gd1_03 -->

- **Chỗ đặt**: `Slot::Library`, vẽ ở My Music, Playlists **và** `Route::Category(..)` (đặc tả 1.2: chung một màn, `mode` lưới / danh sách). View đọc `app::current_route(cx)` và nghe `ShellEvent::RouteChanged` qua `app::shell(cx).subscribe`; vào danh sách thì `app::navigate(Route::Category(CategoryRef::…), cx)`, nút lùi trên thanh trên đã chạy theo lịch sử.
- **Chữ ký ở `app::`** (chi tiết `plan/groove-gd1_03_app-shell.md` mục 9): `Slot::{PlayerBar, Library, NowPlaying}`, `install_slot(slot, view, cx)`, `navigate(Route, cx)`, `current_route(cx)`, `settings(cx)`, `update_settings(cx, |s| …)`, `shell(cx)` phát `ShellEvent`.
- **Vùng nội dung**: đục (`surface`), cao = cửa sổ trừ thanh trên 36 px và thanh phát 90 px; slot nằm trong `div` `flex_1 min_h_0`, view dùng `size_full()`.
- **Icon sẵn**: `GrooveIcon::{DefaultCover, Folder, Delete, Play, Album, Song}`.

Đặc tả (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`, bảng mục 6 dòng 15. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai
Task `sonnet`, làm trọn một phiên, không giao subagent. Mọi phần `[sonnet]`; không có phần `[opus]`. `[owner-qa]`: nhìn thật.

## 6. Nghiệm thu — Case test
Test trong `views/library_page.rs` (`#[gpui::test]`, thư viện thật ở thư mục tạm, không chạm DB người dùng) và `views/category_card.rs`.
- C1 `[sonnet-test]` nạp xong thì có album, thư mục, playlist đúng thứ tự tên; tab Album mặc định, tab Thư mục, route Playlists chọn đúng danh sách: xanh.
- C2 `[sonnet-test]` số cột đo ở khung đầu khớp công thức; `min_cell_width` không bao giờ vượt 6 cột, cửa sổ nhỏ nhất ra 2 cột: xanh.
- C3 `[sonnet-test]` ↓ rồi Enter mở đúng category; ô chờ "Importing..." đứng đầu, không mở gì, các ô thật dịch một: xanh.
- C4 `[sonnet-test]` nhấp thẻ mở danh sách; rê hiện Play all / Delete, nhấp nút chỉ phát sự kiện của nút, không mở thẻ: xanh.
- C5 `[sonnet-test]` tạo playlist (`PlaylistsChanged`) thì lưới nạp lại: xanh.
- C6 `[sonnet-test]` `TooNew` thì không vẽ lưới; `LegacyFailed`: thử lại vẫn hỏng giữ trạng thái lỗi, bắt đầu mới mở thư viện trống: xanh.
- C7 `[sonnet-test]` không có thư viện, route Category, `app::library(cx)`, lùi từ danh sách giữ tab: xanh.
- C8 `[sonnet-test]` mọi khoá chữ có trong catalog; dòng phụ theo loại; màu accent; đường dẫn bìa chặn hash không phải hex: xanh.
- C9 `[owner-qa]` nhìn thật, xem mục 9.

## 8. Việc chưa làm được / chờ task khác
- **`SongList` (task 16) chưa có**: `Route::Category(..)` đang vẽ chữ giữ chỗ (`library.category_pending.title`) trong `library_page.rs`. Task 16 chỉ xuất view, không sửa `library_page.rs`; **task 17 (hoặc ai gộp sau 16) thay nhánh `Route::Category` trong `LibraryPage::render` bằng `SongList`** (một chỗ, có comment).
- **Nghiệp vụ của task 17** (view chỉ phát `LibraryPageEvent`): nhập thư mục, quét lại, hộp tạo playlist, Play all, hộp xác nhận + xoá. Chưa nối; bấm các thẻ hành động hiện chưa làm gì cho tới khi 17 nghe sự kiện.
- **Tự dựng `importing` / `rescanning` từ `LibraryEvent::Progress`** chưa làm: chưa biết sự kiện cuối ("xong") của task 08 ở dạng nào, và bật / tắt trạng thái là việc của `library_ctl` (17).

## 9. Bàn giao thực tế
- Soát 02/11/2026, cwd worktree, sau khi gộp `master`: `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (groove-app 38 test, gồm 14 test của task này), `cargo fmt --all --check`, `cargo deny check licenses` xanh. Không thêm phụ thuộc. Không mở DB thật (mọi test dùng `LibraryConfig` thư mục tạm).
- **API**: `views::library_page::{LibraryPage, LibraryPageEvent, install, install_with, min_cell_width}`; `LibraryPage::{new(library, covers, route, cx), set_route, set_importing, set_rescanning}`; `LibraryPageEvent::{Open(CategoryRef), PlayAll(CategoryRef), Delete(CategoryRef), Import, Rescan, CreatePlaylist}`. `views::category_card::{CategoryCard, CARD_EXTRA_HEIGHT, subtitle, accent_of, cover_path}`. `views::cover_image::{CoverImage, CoverKind, parse_color, DEFAULT_PLAYLIST_COLOR}`. `app::{set_library, library, try_library}`.
- **`app.rs`**: thêm global `LibraryHandle` + ba hàm trên, một `use`, và đúng một lời gọi `library_page::install(cx)` trong `run`. `install` mở `Library::open_standard()` (lời gọi duy nhất trong app), đặt global, dựng view, `install_slot(Slot::Library)`, nối `ShellEvent::RouteChanged` → `set_route`, `LibraryPageEvent::Open` → `app::navigate(Route::Category(..))`, và `shutdown` thư viện lúc thoát app. `open_standard` lỗi (không dựng được luồng nền) thì `library(cx)` không có, màn báo "thư viện chưa sẵn sàng".
- **Trạng thái mở** (`OpenStatus::Failed`): chữ qua i18n, không in lỗi thô (chi tiết vào log). `TooNew`: chỉ báo cập nhật app. `LegacyFailed`: hai nút `retry_open` ("Thử lại") và `start_fresh` ("Bắt đầu mới", chữ nói rõ không chuyển lại dữ liệu cũ, tệp cũ vẫn giữ). `NotLibrary`, `Other`: báo chung.
- **Quyết định (Sonnet chọn 02/11/2026, không phải nghiệp vụ)**:
  - Đầu màn (tiêu đề, thẻ hành động, tab) đứng yên, chỉ lưới cuộn — vì `VirtualGrid` không chứa phần đầu; bản cũ cuộn cả trang. Cửa sổ thấp thì lưới còn ít chỗ (cửa sổ nhỏ nhất 560×360 còn ~1 hàng).
  - Trần 6 cột bằng cách tăng `min_cell_width` theo bề ngang cửa sổ (ước theo cột trái thu 56 px, nên không bao giờ quá 6; cột trái mở thì lưới ít cột hơn mức tối đa, và đang nội suy mở / thu có thể lệch tạm thời), ô tối thiểu 130 px để cửa sổ nhỏ nhất ra 2 cột.
  - Click thẻ vào danh sách ngay (đặc tả "Bấm thẻ vào danh sách"), Enter / nhấp đúp cũng vào; `on_select` chỉ giữ ô chọn.
  - Rescan: bỏ qua nhấp lúc `rescanning` (bản cũ `if (isRescanning) return`).
  - Bìa dùng tệp 256 px (`CoverSize::Small`), `LruImageCache` 192 ảnh (≈ 50 MB trần, gấp 2–3 lần số ô một khung).
  - Playlist luôn có chữ "N bài" (bản cũ ghi `<n> items`); chữ của album không nghệ sĩ: "Chưa rõ nghệ sĩ".
  - Thay `scale(1.05)` + quầng bóng bằng viền accent 2 px khi rê (đặc tả mục 4).
- **Lỗi thật tìm ra bằng test**: `occlude()` quanh nút Play all / Delete chặn hover của thẻ ⇒ `group_hover` tắt, lớp phủ biến mất ngay dưới con trỏ, nút không bấm được. Bỏ `occlude()`, chặn nhấp lan lên thẻ bằng `cx.stop_propagation()` trong `on_click` của nút (test C4 giữ).
- **Soi cải tiến**: bảo mật — `hash` bìa lấy từ DB được ghép vào đường dẫn, đã chặn mọi thứ không phải hex (test); UI/UX — lưới có focus, dùng hết bằng bàn phím, nhưng chưa tự focus lúc mở màn (không giành focus của ô tìm); RAM — bìa qua `LruImageCache`, trần cố định, chưa đo thật (cần DB thật); Pro — không áp.
- **Chờ kiểm tay `[owner-qa]` (C9)**: chưa nhìn bằng mắt vì máy dev chỉ một màn (`tools/run.ps1` từ chối). Máy hai màn: `cargo build -p groove-app` rồi `pwsh tools/run.ps1 -Keys "wait@800 shot@library"`, xoá thư mục `-Out` sau khi xem. Mong đợi: tiêu đề + hai thẻ + tab Album / Thư mục, lưới 2–6 cột, bìa vuông, rê thẻ ra viền accent + hai nút tròn, chữ không bị cắt, thanh cuộn nổi bên phải không đè ô. Dữ liệu ở thư mục tạm nên màn sẽ trống (hiện "Chưa có album"); muốn có thẻ thì trỏ `GROOVE_DATA_DIR` vào thư mục có `library.db` sao chép từ DB thử.

## 10. Đề xuất cải tiến
- [ux] Đầu màn không cuộn cùng lưới — `VirtualGrid` không có chỗ cho phần đầu — thêm `header` vào `VirtualGrid` (hàng ảo đầu tiên cao tuỳ ý) — cửa sổ thấp vẫn thấy nhiều hàng — M — P3, thuộc materiant, chưa làm vì cần đổi API `VirtualGrid` (repo khác, ngoài tệp task).
