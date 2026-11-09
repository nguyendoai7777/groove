# groove — trình phát nhạc Groovex

Repo git riêng trong workspace `xdesktop` (luật chung, quyết định D-* / G-*: `../CLAUDE.md`, `../docs/decisions.md`).
Đã viết lại từ Tauri + Vue sang Rust + GPUI + materiant (G-1): mọi task GĐ1 đã gộp, trừ task 28 (đọc localStorage cũ, tuỳ
chọn, chưa làm). Lộ trình: `ROADMAP.md`.
Repo không có `docs/` (D-7): đặc tả viết lại và bảng so bản cũ đã làm xong, đọc qua lịch sử git —
`git show 1a2480f:docs/rewrite-spec.md`, `git show 1a2480f:docs/parity.md`. Comment `đặc tả <mục>` và mã lỗi `S<n>`, `N<n>`,
`M<n>` trong mã, task trỏ vào đặc tả đó (mục 3 là danh sách lỗi). Task: `plan/groove-gd1_*.md` (skill `run-task`), việc treo `plan/_backlog.md`.

**Bản Rust chưa được chủ dự án nhìn trên màn thật.** Cửa sổ thật mới chạy trong desktop ẩn (`tools/measure-hidden.ps1`), nên
bản Tauri vẫn là bản chạy được duy nhất đã kiểm:

- Tag `tauri-final` là commit cuối còn bản Tauri. Đặc tả trỏ `src/…`, `src-tauri/…`: đọc bằng
  `git -C groove show tauri-final:<đường dẫn>` (hoặc thẳng thư mục khi nó còn).
- Nhánh `claude/remove-tauri` gỡ `src/`, `src-tauri/`, Node, `public/`, `scripts/`, chưa gộp. Chủ dự án gộp sau khi đã xem app
  thật (`plan/_backlog.md`, dòng `[owner-qa]` đầu tiên). Trước lúc đó `master` vẫn còn các thư mục này: chỉ đọc đối chiếu,
  không sửa, không debug (G-1); `src-tauri` nằm ngoài cargo workspace (`exclude`).

## Bố cục (cargo workspace)

| Crate | Loại | Việc | Không được mang |
| --- | --- | --- | --- |
| `crates/groove-core` | lib | `paths`, `i18n`, `logging`, `model`, `settings`, `session`, `queue`, `timeline`, `keymap`, `cli` | UI, âm thanh, SQLite |
| `crates/groove-library` | lib | `db/{schema,migrate,queries}`, `actor`, `store`, `tags`, `fixtures`, `covers`, `scanner`, `indexer`, `watcher`, `reconcile`, `metadata_ops`, `legacy_ls` | UI |
| `crates/groove-audio` | lib | `engine`, `decoder`, `output`, `dsp`, `export` (Save as new: cắt / xuất; MP3 / AAC qua Media Foundation trên Windows — gd2_02; M4R trên Linux / macOS bằng `filmcraft-aac` + `filmcraft-isobmff` thuần Rust, `export::aac`) | UI, SQLite |
| `crates/groove-app` | bin `groove` | `app`, `route`, `icons`, `player`, `session_sync`, `library_ctl`, `commands`, `instance`, `open_paths`, `window_state`, `dev`, `platform/{media_session,taskbar_windows,always_on_top,global_keys}`, `views/*` | — |

Viết tắt trong task: `core`, `library`, `audio`, `app`, `views` = `crates/groove-core/src`, `crates/groove-library/src`,
`crates/groove-audio/src`, `crates/groove-app/src`, `crates/groove-app/src/views`.

## Nối dây

- Khởi tạo app: chuỗi lời gọi `install` trong `app::run` (thứ tự có nghĩa: thư viện → player → phím → view). View thêm vào
  `Shell` qua `Slot`; hộp thoại tự vẽ lớp phủ (`Dialog`) nằm trong cây của view chủ.
- `groove-app` không còn `#![allow(dead_code)]` (gỡ ở task 29). Hàm chỉ test dùng thì `#[cfg(test)]`; mã chết nghĩa là
  tính năng chưa nối, nối chứ đừng xoá cho qua. Crate lib dùng `pub` nên lint không bắt: soát bằng tay khi thêm API.
- Mỗi tệp `.rs` mở đầu bằng dòng ghi task tạo ra nó, để tra mục 9 của task đó.

## Luật riêng repo này

- **Chữ hiển thị**: `groove_core::t!("<vùng>.<khoá>")` trả `&'static str`, `t!("<vùng>.<khoá>", name = giá_trị)` trả
  `String`. Catalog `i18n/{vi,en}/<vùng>.toml`, khoá viết **không tiền tố vùng**, bảng lồng được, biến `{name}`. Mỗi task sở
  hữu một vùng: `common` (01), `shell` (03), `player` (14), `library` (15), `song_list` (16), `actions` (17), `metadata` (19),
  `song_dialogs` (20), `now_playing` (21), `palette` (23), `settings` (24), `platform` (26), `save_as` (gd2_03). Có test: hai ngôn ngữ cùng bộ
  khoá và biến, không chữ toàn hoa (D-13). Task 02 gọi `i18n::set_language` theo cài đặt; `i18n::Language` serde `vi`, `en`.
- **Đường dẫn**: `groove_core::paths`: `data_dir` (`<data>/Groove X`), `cache_dir`, `library_db`, `settings_file`,
  `session_file`, `covers_dir`, `logs_dir`, `legacy_db`; so sánh `normalize`, `is_under`. Không tự ghép thư mục OS.
  `GROOVE_DATA_DIR` chỉ bản debug (M1), kéo theo cả cache và log.
- **Log**: macro của crate `log`; `main` đã gọi `groove_core::logging::init("groove")`. Mức `GROOVE_LOG=debug|info|warn|error`;
  bản release ghi stderr khi có `GROOVE_LOG_STDERR`.
- **JSON** khoá camelCase, enum snake_case; trường mới có `#[serde(default)]`; ghi bằng tệp tạm + đổi tên.
- **UI thread không chạm đĩa** (N7). `LibraryActor` là luồng ghi duy nhất, đọc thẻ ở worker ngoài transaction.
- **Không `panic = "abort"`** (S25): decoder và đọc thẻ bọc `catch_unwind`, tệp độc thành `Error`, không sập app.
- **GPUI** đi qua `materiant::gpui_kit`, không ghim lần hai (D-6). Test view: dev-dep `gpui` (`gpui-pre` với `test-support`).
  Lưới, danh sách dùng `materiant::virtual_grid`, `materiant::list`.
- **`libasound` (LGPL) đã chấp nhận, chỉ link động** (G-2): ở Linux `cpal` sau feature `groove-audio/linux-alsa`, bật mặc
  định; không link tĩnh, không kèm vào bản cài, gói `.deb` khai là phụ thuộc. Build Linux cần `libasound2-dev`.
- Mã chỉ một OS ⇒ `#[cfg(target_os = …)]` trong `app/platform`, không rải khắp nơi.
- **DB cũ của bản Tauri** (`paths::legacy_db`, `<data>/com.mrdxd.groovex/groovex.db`): chép sang `library.db` rồi migrate bản
  chép trong một transaction, không sửa tệp cũ; giữ nguyên `id` của `songs`, `playlists`, `albums`, `folders`; `user_version`
  lớn hơn bản đang hỗ trợ thì từ chối mở và báo, không ghi đè. Thẻ tệp nhạc tương thích hai chiều, `TXXX:TIMELINE` giữ tên.
- **Lệch có chủ đích so với bản web** (GPUI: `div` không có `scale`, `transform`, `backdrop-filter`, transition khai báo):
  khối nổi nền đặc (D-10, sửa 03/11/2026) thay `backdrop-blur`, chỉ cột trái mờ cả cửa sổ (`Blurred`; Linux ngoài KWin không có mờ ⇒ cột trái alpha 0,94, `platform::backdrop`); bỏ `scale` khi hover, đổi
  nền / viền; bỏ nhấp nháy số giờ; thanh phát cập nhật tối đa 4 lần / s và chỉ khi đang phát. Nền `Blurred` làm GPUI tắt
  ClearType, chữ cả app khử răng cưa xám — chờ chủ dự án xem trên màn thật.
- **Màu nhấn** theo bảng màu đang chọn (Settings → Chung; mặc định xanh Material `#005cbb` của materiant — G-4, đổi 03/11/2026): `app::apply_theme` = `set_palette(preset)` + bản vá groove (nền, font, độ đậm), không đặt `primary`; nền và bảng màu lưu riêng (`theme`, `themePreset`); view lấy
  `cx.theme().colors().primary` (chữ trên nền nhạt: `primary_ink`), không mã màu nhấn cứng. Playlist chưa có màu dùng màu nhấn.

## Soát UI riêng của groove

Danh sách chung: `materiant/CLAUDE.md` mục "Soát trước khi xong một component / màn". Thêm cho groove:

- **Một độ đậm** (G-6): `app::groove_theme` đặt `label_weight` / `title_weight` của theme materiant về normal; view không gọi
  độ đậm khác `NORMAL` (test `tests/font_weight.rs` quét `src/**`), phân cấp bằng cỡ chữ và màu.
- **Màu nhấn** từ theme (G-4, mục trên); không thêm mã màu nhấn trong view.
- **Font**: Mona Sans (mặc định) hoặc Be Vietnam Pro nhúng (D-20, feature `font-mona-sans`, `font-be-vietnam-pro` của materiant),
  chọn ở Settings → Chung (`Settings::font`, `UiFont`); `groove_theme` đặt `font_family`, gốc `Shell` gắn
  `.font_family(cx.theme().font_family())`. Cửa sổ / gốc mới phải gắn như vậy, không thì rơi về font hệ thống (Linux: Ubuntu Sans biến
  thiên, lệch dấu tiếng Việt). Trang giấy phép kèm OFL của cả hai họ (`packaging/licenses/gen-licenses.ps1`).
- **Không cắt chữ "…"** (chủ dự án, 2026-11-03): chữ của dữ liệu (tên bài, nghệ sĩ, album, đoạn timeline, đường dẫn) xuống
  dòng như Apple Music — không `truncate` / `text_ellipsis`. Danh sách ảo dòng khác cao: `VirtualList::measured_rows()` + gọi
  `remeasure()` khi chữ đổi mà số dòng giữ nguyên; danh sách ngắn trong hộp thoại: `.fit_content().max_h(…)`. Chỗ cao cố định
  không xuống dòng được (thanh phát) thì chạy chữ `Marquee`. Chỗ còn cắt: lịch sử git (`plan/confirm.md` cũ, mục `wrap`).
- **Space là phím toàn cục** (phát / dừng): vòng focus đi qua `materiant::style::focus_visible`, kể cả nút / hàng tự dựng.
- **Hoạt ảnh lặp** (chữ chạy `Marquee`, thanh nhảy `PlayingBars`): chỉ chạy khi cửa sổ active, dừng tại chỗ khi cửa sổ nền / thu
  nhỏ. Mỗi khung GPUI dựng lại view đang chạy hoạt ảnh và **mọi view tổ tiên**; view nhúng `cached` không bẩn thì dùng lại cả
  cây (số đo: lịch sử git, `plan/confirm.md` cũ mục `cpu`). Giữ đúng bố cục `views::island`:
  - hoạt ảnh lặp là entity riêng (`MarqueeView`, `BarsView`), khung của nó chỉ `notify` chính nó — không `use_keyed_state` làm
    trạng thái trực tiếp (GPUI `notify` luôn view chủ), không `request_animation_frame` trong `render` của view chủ;
  - đường từ gốc tới hoạt ảnh (Shell → thanh phát / Now Playing) nhúng thường; nhánh nặng bên cạnh nhúng `cached`: cột trái, thanh
    trên (`Island<Shell>`), trang thư viện, bìa / cụm điều khiển / âm lượng của thanh phát (`Island<PlayerBar>`), ba khung
    của Now Playing, bìa / cụm nút của PiP (`Island<PipView>`, `views::pip`). View `cached` mà bẩn thì GPUI dựng lại CẢ cây con (kể cả `cached` bên trong);
  - view `cached` chỉ dựng lại khi chính nó (hay view con) `notify`: thứ nó đọc trong `render` phải có `observe` → `notify`
    (`Island` nghe chủ, chủ nghe `Player`…). Theme, ngôn ngữ, đổi cỡ, rê chuột, focus thì GPUI tự làm mới cả cửa sổ;
  - cần cỡ xác định: `.cached(style)` bố cục theo `style`, không đo nội dung — gốc của view con lấp đúng khung (`size_full`).
  - thanh nhảy đang chạy nằm TRONG view `cached` (danh sách bài, hàng đợi, header dòng thời gian) thì đặt
    `bars_overlay::anchor`, không `PlayingBars::new(true)`: lớp phủ cấp nội dung của Shell vẽ nó, trang không bẩn theo khung.
    View chứa anchor gọi `bars_overlay::forget(cx.entity_id(), cx)` đầu `render`;
  - nhịp vị trí khi phát là `Player::clock()` (`PlaybackClock`), không phải `notify` của `Player`: chỉ view vẽ vị trí nghe nó
    (thanh tua `cached` riêng, TimelineBox), thưa còn mỗi giây khi cửa sổ nền (`PlaybackClock::due`). View nghe `Player` chỉ
    `notify` khi đúng thứ nó vẽ đổi (`BarState` của thanh phát, `Body` của lời bài…).
  - chuyển ngắn (banner danh sách bài thu / nở 300 ms): `views::banner::Banner` — entity riêng xin khung chỉ khi đang chuyển,
    phủ lên nội dung `cached` có khung cố định (khung chuyển không dựng lại bảng); test
    `song_list::tests::banner_transition_runs_briefly_without_rebuilding_the_table`.
  Test mẫu: `app::tests::animation_frames_reuse_the_cached_page_but_data_changes_still_show`,
  `app::tests::bars_in_a_cached_page_animate_on_the_overlay_without_rebuilding_the_page`,
  `player::tests::position_ticks_notify_the_clock_not_the_player`, `views::island::tests`.
- **Ảnh bìa**: `views::cover_image::CoverImage` cho cả bìa thư viện lẫn bìa nhúng (`.embedded`), `.radius(…)` bằng bo góc của khung
  chứa (không phóng khi rê); lớp phủ trên bìa tự bo cùng bán kính. Bìa nhúng là `Arc<RenderImage>` đã thu 512 px của
  `Player::external_cover` — không `Image::from_bytes` + `use_render_image` (bộ đệm tài nguyên của GPUI không bao giờ nhả);
  ảnh tự giải mã thì nhả bằng `cx.drop_image` khi bỏ.
- **Xem bằng mắt ở desktop ẩn**: `tools/measure-hidden.ps1 -Seed <seed_data.exe>` có sẵn một bài tên dài đang mở (không phát) để
  thanh phát không khoá và tên chạy; `-Songs <n>` thêm n bài vào thư viện tạm (đo với thư viện có bài); `-Active` mở cửa sổ ở
  trạng thái active; dòng mốc in cả `cpu=` (% một nhân). Không gõ `space` trong `-Keys` (phát ra loa). CPU so trước / sau thì
  đo hai bản trong cùng một lượt: cùng exe mà khác lượt lệch tới 2–3 lần (xung nhịp máy).

## Lệnh

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check licenses          # soát cả cây của Windows, macOS, Linux theo deny.toml
cargo fmt --all
cargo run -p groove-app -- --display secondary --background   # mở cửa sổ ở màn phụ, không giành focus
bash start.sh [--no-watch]          # = bash ../start.sh groove: bản debug `groove.dev`, theo dõi mã (watch-run.mjs), scope systemd + mục "Groovex (dev)" trên Linux; chạy được từ worktree (build ../groove/target/w<N>)
bash build.sh [-f]                  # = bash ../build.sh --project groove: bản cài máy đang chạy vào ../release/groove (Windows: pwsh build.ps1); groove chưa có plan (`--plans` rỗng)
pwsh tools/run.ps1 -Keys "ctrl-k wait@300 shot@palette"       # build trước: cargo build -p groove-app
pwsh packaging/windows/build-installer.ps1                    # bản cài NSIS ở dist/; cần NSIS, cargo-about
pwsh packaging/windows/installer-e2e.ps1 -Setup <setup.exe>   # soát cài, liên kết, gỡ; ghi HKCU rồi dọn, không mở app
bash packaging/macos/build-macos.sh                           # .app, .dmg, ký ad-hoc; chạy trên Mac
bash packaging/linux/build-deb.sh                             # .deb, libasound là phụ thuộc (G-2); chạy trên Ubuntu
```

`pwsh tools/measure-hidden.ps1 -Exe <groove.exe> [-Keys …] [-Shot ảnh.png] [-Seed <exe>] [-Active] [-Clicks min,max,close]`: chạy app thật trong desktop Win32 ẩn, không hiện ở
màn nào, không giành focus, dữ liệu tạm; in RAM, số cửa sổ, tiêu đề, dòng WARN / ERROR. Từ chối exe không đọc
`GROOVE_DATA_DIR` (bản phát hành thật sẽ mở thư viện thật); đo gần bản phát hành thì build release với
`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`. Không có nền mờ, con trỏ, taskbar thật; xem ảnh xong xoá. `-Clicks` bấm nút cửa sổ
bằng thông điệp Win32 như chuột thật, in hit-test và `IsIconic` / `IsZoomed` thật sau mỗi lần bấm (`-Hover <ms>` rê trước khi bấm); `hit@x:y` chỉ in hit-test ở
điểm đó, `dbl@x:y` nhấp đúp (vùng kéo). Muốn view `cached` được dùng lại như lúc chạy thật thì thêm `-Seed … -Active` (chữ chạy). `-Settings <json>` ghi
`settings.json` của dữ liệu tạm trước khi mở (vd `{"theme":"light"}` để chụp theme sáng). Kéo chuột trong `-Clicks`: `down@x:y`,
`move@x:y`, `up@x:y` (đi đường `WM_NC*` khi hit-test không phải `HTCLIENT`, như chuột thật), `shot@<tệp.png>` chụp giữa các bước;
kéo slider của materiant cần vài bước `move`. `-Pip` chạy kèm `--pip`, bấm / chụp cửa sổ trình phát thu nhỏ. `nudge@n` dời cửa sổ
n lần 1 px mỗi 16 ms giữa `WM_ENTERSIZEMOVE` / `WM_EXITSIZEMOVE` như lúc kéo và in CPU lúc dời (nền mờ không có ở desktop ẩn);
`settings` in `settings.json` của dữ liệu tạm.

`tools/run.ps1`: màn phụ, không focus, phím đưa thẳng vào GPUI, chụp đúng cửa sổ, in RAM, `foreground_unchanged`; dữ liệu ở
thư mục tạm; máy chỉ một màn thì từ chối (không đè màn đang dùng), muốn thử thật truyền `-Display primary`. `--display` chọn
màn không có thì thoát, không rơi về màn mặc định. Xem ảnh xong xoá thư mục `-Out`.

## Bẫy

- **Test cần thư mục dùng `groove_core::test_dir::TestDir`, không `tempfile`** (đã bỏ khỏi workspace): trên Windows SQLite
  còn mở lúc `TempDir` drop ⇒ xoá thất bại im lặng, mỗi lượt `cargo test` sót ~180 thư mục `.tmp*` trong `%TEMP%` (từng gom
  37 nghìn thư mục, đăng nhập Windows chậm hẳn). `TestDir::new()` trỏ `target/test-tmp/<crate>/<tệp>-<tên test>[-n]`, cố
  định giữa các lượt, xoá bản cũ lúc tạo ⇒ số thư mục có trần bằng số test. Crate mới cần nó: dev-dep `groove-core` với
  feature `test-support`. Script cũng vậy: dữ liệu chạy thử ở thư mục cố định trong `target/`, dùng lại mọi lượt, không
  GUID trong `%TEMP%` — `measure-hidden.ps1` ở `<target>/hidden-data` cạnh exe (xoá sạch đầu lượt, giữ sau lượt để soát log),
  `run.ps1` ở `target/run`, `installer-e2e.ps1` ở `target/installer-e2e`, `gen-licenses.ps1` ở `target/licenses-raw.html`.
  Lưới chặn còn lại: `.cargo/config.toml` đặt `TMP` / `TEMP` của mọi tiến trình cargo (test, `cargo run`, build script, trình
  link) vào `.cargo/tmp` của repo / worktree, `clean.*` dọn; Unix giữ `TMPDIR` (socket có trần độ dài đường dẫn).
- **Vùng `WindowControlArea` (kéo cửa sổ) không đặt trong view `cached`**: gpui-pre 0.3.10 dùng lại hitbox, listener của view
  `cached` nhưng không chép `window_control_hitboxes`, vùng kéo mất từ khung đầu tiên view được dùng lại (hit-test thành
  `HTCLIENT`). Vùng kéo của thanh trên, header cột trái là khung bọc ở `Shell::render`, vẽ sau phần `cached`.
- **Phần tử kéo được (thanh tua, slider) nằm trong vùng kéo cửa sổ thì phải `occlude`**: không thì hit-test là `HTCAPTION`,
  GPUI vẫn nhận MouseDown nhưng Windows vào vòng dời cửa sổ và nuốt lần thả chuột — thanh kẹt ở trạng thái kéo, rê là chạy theo
  (PiP, 03/11/2026).
- **Mỗi `WM_MOVE` GPUI vẽ lại cả cây, bỏ qua cache `cached`** (`bounds_changed` → `refresh`): kéo cửa sổ thấp khung, bóng mờ, ~55 %
  CPU (bản debug). `platform::move_loop` giữ `WM_MOVE` trong vòng dời của OS, thả chuột mới báo một lần — cửa sổ mới phải
  `move_loop::install`. Giá trị không vẽ lên đâu (vị trí cửa sổ) ghi bằng `app::store_settings_quietly`, không `update_settings`
  (cái sau `notify` `Shell`). Nền `Blurred` (acrylic) giật khi kéo trên Windows 10 / 11 trước 22H2: `backdrop::smooth_blur` đổi
  sang blur-behind như bản Tauri, vùng mờ khít vùng client (đặt lại mỗi `WM_SIZE`), tắt bóng DWM, bỏ viền đổi cỡ vô hình (accent
  phủ cả khung) và tự hit-test 6 px mép trong; sau khi đổi khung phải cho GPUI đo lại `border_offset` (`WM_SETTINGCHANGE`), không thì
  cỡ cửa sổ lưu lại trôi mỗi lần mở. `-Clicks rects` in khung / client. Kiểm tay:
  `GROOVE_BLUR=acrylic` giữ acrylic, `GROOVE_WINDOW_SHADOW=on` giữ bóng.

- **Bản Tauri để đối soát — không xoá**: `src-tauri/target/x86_64-pc-windows-msvc/release/groovex.exe` (exe tự chứa giao diện) và
  `release/groovex_0.1.0_*` (setup, msi, portable). Dọn `target` không đụng hai chỗ này (`clean.sh` đã bỏ qua).
- **Worktree** cố định (D-18): `<gốc>/.claude/worktrees/groove-w<N>`, build ở `groove/target/w<N>`, xong task giữ cả hai. `materiant = { path = "../materiant" }` nhờ lối nối
  `.claude/worktrees/materiant` đã có. `CARGO_TARGET_DIR` riêng từng worktree (`groove/target/w<N>`), không xoá.
- **AppData ảo hoá MSIX**: exe chạy từ app Claude trên Windows ghi vào `LocalCache` của gói. Không thấy DB cũ ở đó không có
  nghĩa là máy không có DB.
- `notify` dùng bản 8 + `notify-debouncer-full` 0.5, không phải bản `-rc` mà `cargo info` đưa ra.
- **`blocking` ghim 1.6.2 trong `Cargo.lock`** (zbus, smol của GPUI Linux kéo vào): 1.7.0 gộp điều kiện thoát thành `thread_count > thread_limit && timed_out` (trần 500) nên luồng `blocking-N` rảnh không bao giờ thoát, thức 2 lần / s mãi. Đo lúc nghỉ: 1.7.0 có 1–2 luồng × 61 lần thức / 30 s, 1.6.2 không còn luồng nào. `cargo update` đừng nâng lên 1.7.x khi upstream chưa sửa.
- **souvlaki vá ở `vendor/souvlaki`** (`[patch.crates-io]`, 0.9.3, MIT): MPRIS bản `use_zbus` chờ lệnh bằng `loop { recv_timeout(10 ms) }`, luồng `media-session` thức 100 lần / s cả lúc nghỉ; bản vá chờ `recv()` chặn. Nâng souvlaki ⇒ soát lại vòng này rồi mới gỡ bản vá. Test chặn cả hai bẫy này: `platform::media_session::tests::idle_mpris_service_does_not_tick`.
- Dòng ERROR `directx_devices.rs:82 ... 0x887A002D` lúc chạy debug chỉ là thiếu lớp debug DXGI, không phải lỗi.
- Trên desktop ẩn còn `unable to get cursor position: Access is denied` và `taskbar list init: 0x80004001`: do desktop ẩn.
- `GROOVE_DATA_DIR` kéo theo cả chỗ tìm DB Tauri cũ (`paths::legacy_db`): chạy thử không bao giờ đọc DB thật.
- **Số đo RAM** (02/11/2026, desktop ẩn, thư viện trống, không phát; release + `debug-assertions` để `GROOVE_DATA_DIR` còn
  hiệu lực): nghỉ 59,5 MB working set / 66,4 MB private, sau khi đi qua Now Playing, bảng lệnh, Settings 63–64 / 68,5–69 MB,
  48 luồng; debug 76 / 70 MB. Chưa đo thư viện thật, lúc phát, macOS, Linux. Lệnh:
  `$env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = "true"; cargo build --release -p groove-app` rồi
  `pwsh tools/measure-hidden.ps1 -Exe target/release/groove.exe -Seconds 30`.
