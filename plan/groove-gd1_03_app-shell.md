# groove-gd1_03 — app-shell

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | sonnet |
| Cỡ | ~700 dòng |
| Phụ thuộc | materiant-gd5_05, groove-gd1_01, groove-gd1_02 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `app/{app,route,icons}.rs`, `views/{side_nav,app_bar}.rs`, `assets/icons/*`, `i18n/*/shell.toml` |

## 0. Bàn giao từ groove-gd1_02 (02/11/2026)

<!-- handoff: groove-gd1_02 -->

- **Đã có sẵn để dùng**: `groove_core::settings::{Settings, ThemeChoice, StartPage, JsonSaver}`, `groove_core::model::{CategoryRef, TrackRef}`.
- **Nợ chuyển sang đây**: khi mở app, trước khi tạo cửa sổ, gọi `Settings::load()` rồi `apply_language()` (task 02 không gọi được vì `main.rs` không sửa). Đây là chỗ đọc đĩa duy nhất được phép trên UI thread (N7).
- **Dùng khi dựng khung**: `settings.theme` (`ThemeChoice`, cùng giá trị serde với `materiant::ThemeMode`, tự đổi kiểu), `settings.last_active_route` (`StartPage`, đổi sang `Route`; `Route::Category(CategoryRef)` không lưu), `settings.side_nav_collapsed`.
- **Lưu**: một `JsonSaver::spawn(paths::settings_file()?, debounce)`, gọi `.save(&settings)` khi đổi; `Drop` ghi nốt khi thoát.

## 0. Bàn giao từ groove-gd1_01 (02/11/2026)

<!-- handoff: groove-gd1_01 -->

- **Đã có sẵn để dùng**: bố cục, lệnh, luật ở `CLAUDE.md`; `groove_core::{paths, i18n, t!, logging}`.
- **Không sửa**: `Cargo.*`, `lib.rs`, `main.rs`, `mod.rs` (mọi `mod` và phụ thuộc đã khai sẵn).
- **Bẫy đã gặp**: `CARGO_TARGET_DIR` riêng từng worktree, build nguội khoảng 3 phút; `tools/run.ps1` từ chối khi máy chỉ một màn.
- **Nợ chuyển sang đây**: `app/app.rs` đang là cửa sổ trống còn thanh tiêu đề OS, đổi sang không viền; `run(dev: DevArgs)` giữ `display_id`, `focus: !dev.background`, `dev::replay` để `tools/run.ps1` chạy được; chừa chỗ một lời gọi khởi tạo cho từng task sau. Vùng i18n `shell`.

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 03. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).

## 2b. Phân vai

Task `sonnet` (D-14), chạy trọn một session, không giao subagent. Mọi phần là ghép UI từ materiant, không có nghiệp vụ.

| Phần việc | Nhãn |
| --- | --- |
| `route.rs`, `icons.rs` (tách sprite thành 30 tệp SVG), `views/{side_nav,app_bar}.rs`, `app.rs`, `i18n/*/shell.toml` | [sonnet] |
| Test `#[gpui::test]` trong App ảo, `cargo test` | [sonnet-test] |
| Nhìn cửa sổ thật (nền mờ, kéo, nút cửa sổ, hover) | [owner-qa] |

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | Cho tuỳ chọn cửa sổ chính, khi dựng `main_window_options`, thì không `titlebar`, nền `Blurred`, `Client` decorations, `--background` không giành focus | [sonnet-test] `app::tests::main_window_is_frameless_and_blurred` |
| C2 | Cho `last_active_route = playlists`, khi mở app, thì ở Playlists, nút lùi tắt, tiêu đề `Danh sách phát | Groove X` | [sonnet-test] `opens_at_last_active_route` |
| C3 | Cho My Music, khi bấm mục Now Playing rồi lùi, thì đổi màn đúng, phát `RouteChanged`, `last_active_route` đi theo | [sonnet-test] `nav_click_navigates_and_persists`, `route::tests::history_back_follows_pushes` |
| C4 | Cho xem danh sách một playlist, khi đó, thì cột trái tô Playlists, `last_active_route` không nhớ tham số, bấm lại đúng màn đó không thêm bước | [sonnet-test] `category_route_keeps_parent_highlight_and_is_not_saved`, `route::tests::start_page_round_trip_drops_category` |
| C5 | Cho cột trái mở, khi bấm nút thu, thì `side_nav_collapsed = true` | [sonnet-test] `toggling_side_nav_updates_settings` |
| C6 | Khi bấm ô tìm hay nút Settings, thì phát `OpenSearch` / `OpenSettings` | [sonnet-test] `search_and_settings_emit_events` |
| C7 | Mọi `GrooveIcon` có SVG nhúng hợp lệ, không tệp mồ côi, rơi về icon materiant | [sonnet-test] `icons::tests::*` |
| C8 | Slot trống vẽ được, điền view thì giữ view; `Library` chung cho ba màn | [sonnet-test] `slots_start_empty_and_accept_views` |
| C9 | Đổi `theme` trong cài đặt thì theme materiant đổi; `ThemeChoice` và `ThemeMode` cùng giá trị serde | [sonnet-test] `settings_change_applies_theme`, `theme_choice_maps_to_the_same_mode` |
| C10 | Cửa sổ thật ở màn phụ: nền mờ lộ desktop ở cột trái, nội dung đục, kéo thanh trên, ba nút cửa sổ, hover đỏ nút đóng | [owner-qa] |

## 8. Nợ chuyển task khác

- **Task 14, 15, 16**: điền slot bằng `app::install_slot(Slot::…, view, cx)` (một lời gọi trong `run`), xem mục 9.
- **Task 23**: nghe `ShellEvent::OpenSearch`. **Task 24**: nghe `ShellEvent::OpenSettings`. Ô tìm đã vẽ và bắn sự kiện, chưa có bảng lệnh.
- **Task 21**: đặt tiêu đề `<bài> - <nghệ sĩ> | Groove X` bằng `app::set_title_override`; đổi màn thì tự về tên trang.
- **Task 27**: `window_state` ghi / đọc vị trí cửa sổ; `run` hiện luôn mở ở giữa màn, 800 × 600.
- **Chưa có quyết định nghiệp vụ nào bị bỏ lửng.**

## 9. Bàn giao thực tế

- **Kết quả**: nhánh `claude/gd1_03`, `cargo build`, `clippy --all-targets -D warnings`, `cargo test` (19 test app, 58 test core), `cargo deny check licenses` đều xanh. Không thêm phụ thuộc.
- **[owner-qa] C10 — chờ kiểm tay**: máy dev chỉ một màn nên `tools/run.ps1` từ chối, không ép mở cửa sổ ở màn chính. Chủ dự án chạy ở máy hai màn: `cargo build -p groove-app` rồi `pwsh tools/run.ps1 -Keys "wait@500 shot@shell down@300,100 up@300,100 wait@500 shot@now"` (hoặc `cargo run -p groove-app -- --display secondary --background`). Nhìn: cột trái mờ, bấm nút thu (300 ms), bấm ô tìm (chưa có gì mở), ba nút cửa sổ, kéo thanh trên. Đã ghi `plan/_backlog.md`.
- **Đã làm theo khối bàn giao**: `run` gọi `Settings::load()` rồi `apply_language()` trước khi tạo cửa sổ (chỗ đọc đĩa duy nhất của UI thread); cửa sổ không viền (`titlebar: None`, `WindowDecorations::Client`, `translucent_window_options`); `run(dev)` giữ `display_id`, `focus: !dev.background`, `dev::replay`.
- **Chữ ký cho task sau** (tất cả ở `app::`):
  - `enum Slot { PlayerBar, Library, NowPlaying }`; `install_slot(slot, view, cx)` nhận `Entity<V>` hoặc `AnyView`. `Library` vẽ ở cả My Music, Playlists và `Route::Category(..)` (đặc tả 1.2: chung một màn); view đọc `current_route(cx)` và nghe `ShellEvent::RouteChanged`. Slot trống vẽ chữ giữ chỗ `shell.slot.empty`; `PlayerBar` trống vẫn chừa 90 px.
  - `navigate(Route, cx)`, `go_back(cx)`, `current_route(cx)`; `Route::{MyMusic, NowPlaying, Playlists, Category(CategoryRef)}`, `Route::title()`, `nav_section()`, `start_page()`.
  - `shell(cx) -> Entity<Shell>` phát `ShellEvent::{RouteChanged(Route), OpenSearch, OpenSettings}`; `open_search(cx)`, `open_settings(cx)` cho `commands.rs`.
  - `settings(cx) -> &Settings`, `update_settings(cx, |s| …)`: sửa, `sanitize`, `cx.notify`, lưu qua một `JsonSaver` duy nhất (debounce 400 ms, flush khi thoát), tự áp theme và ngôn ngữ khi đổi. Task 13, 24 dùng hàm này, không tạo saver khác.
  - `set_title_override(Option<SharedString>, cx)`.
  - `icons::GrooveIcon` (30 icon, `impl From<GrooveIcon> for IconSource`), `icons::Assets` đã đăng ký ở `run` (rơi về icon của materiant).
  - Thêm icon: tệp `.svg` vào `crates/groove-app/assets/icons/` + một dòng `groove_icons!`.
- **Bố cục**: cột trái 230 / 56 px (nội suy 300 ms qua `transition`, chữ chỉ hiện khi rộng hơn 120 px), đầu 36 px, mục 42 px, vạch trái 3 px; thanh trên 36 px; thanh phát 90 px. Cột trái dùng `translucent_surface` (alpha 0,88 tông tối, tông sáng đục); vùng nội dung và thanh phát `surface` đục.
- **Bẫy đã gặp**: `#[derive(IntoElement)]` ở bản không test cần `use materiant::gpui_kit as gpui;` (xem `views/app_bar.rs`); `on_click` của `Button` / `IconButton` nhận `&ClickEvent`; `IconButton` bọc `.occlude()` bên trong vùng `drag_region()` nếu không bấm nút sẽ kéo cửa sổ.
- **Đã tách sprite**: 30 icon (đặc tả ghi 29): `ArrowsCompress` trong `arrow.svg` viết bằng thẻ `<svg>` lồng thay vì `<symbol>`, vẫn tách được. Các sprite cũ ở `public/` giữ cho bản Tauri tới task 29.
- **Quyết định tự chọn** (ghi `plan/confirm.md`): cỡ nhỏ nhất 560 × 360; nền cột trái dùng `translucent_surface` 0,88 thay 0,4 của bản cũ; tên trang đổi sang "Thư viện / Đang phát / Danh sách phát / Cài đặt"; tiêu đề cửa sổ `<trang> | Groove X`; `Slot::Library` chung ba màn.
- **Soi bốn lăng kính**: bảo mật không áp (không IPC, không tệp ngoài `settings.json` qua `JsonSaver` của task 02); UI/UX: mọi nút là `Button` / `IconButton` của materiant nên Tab + Enter dùng được, có tooltip khi cột thu, nút lùi `disabled` khi hết lịch sử, nút đóng hover đỏ, giảm chuyển động thì cột trái nhảy; RAM: không giữ gì thêm ngoài một `Entity<Settings>`, nội suy chỉ xin khung khi đang chạy; Pro không áp.

## 10. Đề xuất cải tiến

Không có đề xuất ngoài phạm vi.
