# groove — trình phát nhạc Groove X

Repo git riêng trong workspace `xdesktop` (luật chung, quyết định D-* / G-*: `../CLAUDE.md`, `../docs/decisions.md`).
Đang viết lại từ Tauri + Vue sang Rust + GPUI + materiant (G-1). Đặc tả, lỗi cần sửa, bảng task và làn song song:
`docs/rewrite-spec.md`. Task: `plan/groove-gd1_*.md` (làm bằng skill `run-task`), việc treo `plan/_backlog.md`, việc đã tự
chọn `plan/confirm.md`.

**Bản Tauri (`src/`, `src-tauri/`, Node) vẫn còn đến task 29**: chỉ để đọc đối chiếu, không sửa, không debug (G-1). `src-tauri`
nằm ngoài cargo workspace (`exclude`), vẫn build riêng được.

## Bố cục (cargo workspace)

| Crate | Loại | Việc | Không được mang |
| --- | --- | --- | --- |
| `crates/groove-core` | lib | `paths`, `i18n`, `logging`, `model`, `settings`, `session`, `queue`, `timeline`, `keymap`, `cli` | UI, âm thanh, SQLite |
| `crates/groove-library` | lib | `db/{schema,migrate,queries}`, `actor`, `store`, `tags`, `fixtures`, `covers`, `scanner`, `indexer`, `watcher`, `reconcile`, `metadata_ops`, `legacy_ls` | UI |
| `crates/groove-audio` | lib | `engine`, `decoder`, `output`, `dsp` | UI, SQLite |
| `crates/groove-app` | bin `groove` | `app`, `route`, `icons`, `player`, `session_sync`, `library_ctl`, `commands`, `instance`, `open_paths`, `window_state`, `dev`, `platform/{media_session,taskbar_windows}`, `views/*` | — |

Viết tắt trong task: `core`, `library`, `audio`, `app`, `views` = `crates/groove-core/src`, `crates/groove-library/src`,
`crates/groove-audio/src`, `crates/groove-app/src`, `crates/groove-app/src/views`.

## Luật song song (đặc tả mục 6)

- Mọi crate, `mod`, phụ thuộc đã khai sẵn. Task sau **không sửa** `Cargo.toml`, `Cargo.lock`, `deny.toml`, `lib.rs`,
  `main.rs`, `mod.rs`. Mỗi tệp `.rs` mở đầu bằng dòng ghi task sở hữu; chỉ sửa tệp của mình, thiếu API từ làn khác thì ghi
  mục 8 của task.
- Thật sự thiếu phụ thuộc ⇒ soát giấy phép, thêm đúng một dòng, ghi mục 9 và `plan/confirm.md`; khả năng xung đột `Cargo.lock`
  thì lúc gộp `cargo build` sinh lại.
- **Ngoại lệ có chủ ý: `app/app.rs`.** Task 03 sở hữu; task sau cần khởi tạo lúc mở app (13, 17, 22, 25, 26, 27) được thêm
  đúng một lời gọi vào `run`, mọi thứ khác nằm ở tệp của mình. Task 25 viết thân `instance::forward_to_primary` (chữ ký có
  sẵn, `main` đã gọi).
- `views/mod.rs` khai sẵn; task 03 dựng slot trống cho từng view.
- `#![allow(dead_code)]` ở `main.rs` để view có trước khi nối được; gỡ ở task 29. Crate lib dùng `pub`, không cần.

## Luật riêng repo này

- **Chữ hiển thị**: `groove_core::t!("<vùng>.<khoá>")` trả `&'static str`, `t!("<vùng>.<khoá>", name = giá_trị)` trả
  `String`. Catalog `i18n/{vi,en}/<vùng>.toml`, khoá viết **không tiền tố vùng**, bảng lồng được, biến `{name}`. Mỗi task sở
  hữu một vùng: `common` (01), `shell` (03), `player` (14), `library` (15), `song_list` (16), `actions` (17), `metadata` (19),
  `song_dialogs` (20), `now_playing` (21), `palette` (23), `settings` (24), `platform` (26). Có test: hai ngôn ngữ cùng bộ
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
- **`libasound` (LGPL) chưa được chấp nhận**: đang chờ chủ dự án (đặc tả 5.1), không ghi là đã chấp nhận. Ở Linux `cpal` là
  optional sau feature `groove-audio/linux-alsa`, tắt mặc định; Windows, macOS luôn có. Mã đụng `cpal` đặt sau
  `#[cfg(any(not(target_os = "linux"), feature = "linux-alsa"))]`, nhánh còn lại trả `Error`, build Linux mặc định vẫn phải
  qua. Bản Linux mặc định vì vậy chưa có tiếng cho tới khi chủ dự án quyết.
- Mã chỉ một OS ⇒ `#[cfg(target_os = …)]` trong `app/platform`, không rải khắp nơi.

## Lệnh

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check licenses          # soát cả cây của Windows, macOS, Linux theo deny.toml
cargo fmt --all
cargo run -p groove-app -- --display secondary --background   # mở cửa sổ ở màn phụ, không giành focus
pwsh tools/run.ps1 -Keys "ctrl-k wait@300 shot@palette"       # build trước: cargo build -p groove-app
```

`tools/run.ps1`: màn phụ, không focus, phím đưa thẳng vào GPUI, chụp đúng cửa sổ, in RAM, `foreground_unchanged`; dữ liệu ở
thư mục tạm; máy chỉ một màn thì từ chối (không đè màn đang dùng), muốn thử thật truyền `-Display primary`. `--display` chọn
màn không có thì thoát, không rơi về màn mặc định. Xem ảnh xong xoá thư mục `-Out`.

## Bẫy

- **Worktree**: đặt ở `<gốc>/.claude/worktrees/groove-<task>`. `materiant = { path = "../materiant" }` nhờ lối nối
  `.claude/worktrees/materiant` đã có. `CARGO_TARGET_DIR` riêng từng worktree (`groove/target/<task>`), xoá khi xong.
- **AppData ảo hoá MSIX**: exe chạy từ app Claude trên Windows ghi vào `LocalCache` của gói. Không thấy DB cũ ở đó không có
  nghĩa là máy không có DB (đặc tả 2.3).
- `notify` dùng bản 8 + `notify-debouncer-full` 0.5, không phải bản `-rc` mà `cargo info` đưa ra.
- Dòng ERROR `directx_devices.rs:82 ... 0x887A002D` lúc chạy debug chỉ là thiếu lớp debug DXGI, không phải lỗi.
