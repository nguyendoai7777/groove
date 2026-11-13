# groove-gd2_05 — auto-update

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (13/11/2026) |
| Repo | groove (+ materiant: crate `updater`; cloud: đăng ký app `groove`) |
| Model | opus |
| Cỡ | M — ~650 dòng groove (update, whats_new, about, app_menu, nút thanh trên, i18n) + crate `updater` ~900 dòng dùng chung |
| Phụ thuộc | groove-gd2_01 (bộ cài), fastcast gd8_02 (server phát bản), gd8_03 (mẫu client) |
| Quyết định áp dụng | D-2, D-9, D-13, D-21, D-22, D-27, G-6, N7 |
| Phụ thuộc mới | `updater` (path `../materiant/updater`): `ureq` 3 + rustls `ring` + `rustls-platform-verifier`, `ed25519-dalek` 2, `sha2` 0.10 — MIT / Apache-2.0 / ISC / BSD, cùng bộ fastcast đã soát |
| File đụng tới | MỚI `crates/groove-app/src/update.rs`, `update/whats_new.rs`, `app_menu.rs`, `views/settings/about.rs`, `assets/icons/update.svg`, `i18n/{vi,en}/update.toml`; SỬA `app.rs`, `commands.rs`, `views/app_bar.rs`, `views/settings/dialog.rs`, `icons.rs`, `main.rs`, `build.rs`, `groove-core/src/i18n.rs`, `packaging/macos/build-macos.sh`, `packaging/windows/{groove.nsi,build-installer.ps1}`, `Cargo.toml` |

## 1. Làm gì

- Soát bản mới 10 s sau khi mở rồi mỗi 6 giờ (`GET /apps/groove/releases/latest`, manifest ký Ed25519 bằng khoá cập nhật riêng của groove).
- Có bản patch / minor ⇒ thanh trên hiện nút "Cập nhật lên v<x>" (rê: "· ⌘D"); bấm hoặc ⌘D / Ctrl+D ⇒ tải, soát sha256 + cỡ, cài,
  app thoát, bản mới tự bật với `--updated`. Major / bản không tự cài được ⇒ mở trang tải.
- Bản mới bật ⇒ hộp "Có gì mới" (note server gửi kèm lúc cài, không có thì `release-notes/v<version>.md` nhúng lúc build); Enter / Esc / OK đóng.
- Settings → Giới thiệu: phiên bản, trạng thái, Kiểm tra cập nhật / Cập nhật. macOS: menu "Groovex" (Giới thiệu, Kiểm tra cập nhật…, Cài đặt…, Ẩn ⌘H, Thoát ⌘Q).
- Build: macOS thêm `groovex_<ver>_macos_<arch>.app.zip` + `.sha256` và in lệnh `publish-release.sh`; NSIS `/UPDATE` bật lại app; `build-installer.ps1` in lệnh publish.

## 2. Không làm trong task này

- Linux tự cài (`.deb` ở `/usr` cần root, chưa có dạng gói thư mục người dùng — như fastcast F-16): Linux chỉ báo + mở trang tải.
- Chuyển fastcast sang crate `updater` (fastcast có bản riêng cùng hợp đồng, đang chạy thật).
- Đăng bản thật lên server, deploy khoá cập nhật prod lên staging, DNS `cloud.groovex.sandtechs.com` (chủ dự án).

## 3. Kiểu dữ liệu · IPC · cấu hình

- `updater::AppSpec` hằng `update::SPEC` (id `groove`, khoá prod, `dev_build = cfg!(debug_assertions)`, gói `Groovex.app`, exe `groove`).
- Sổ `<data>/update/state.json` (`lastCheck`, thân + chữ ký manifest cuối), `<data>/update/notes.json` (note chờ hiện), `<data>/last-version`.
- `ShellEvent::OpenAbout`, `Slot::WhatsNew`, `SettingsTab::About`. Biến dev (chỉ bản debug-assertions): `GROOVE_CLOUD_URL`, `GROOVE_WHATS_NEW=1`.

## 6. Nghiệm thu

- C1 có bản mới ⇒ nút trên thanh trên, không có ⇒ không (`app::tests::update_button_shows_only_when_a_new_version_is_available`).
- C2 manifest ký sai / khác app / khác nền / không mới hơn ⇒ bỏ (`updater` `manifest::tests`, `check::server_tests`).
- C3 sha256 / cỡ sai ⇒ không cài, bản đang dùng còn nguyên (`updater` `install::tests`).
- C4 hộp "Có gì mới" Enter / Esc đóng (`app::tests::whats_new_closes_with_enter_and_escape`), hiện một lần (`updater` `notes::tests`).
- C5 E2E macOS arm64 với server giả: 0.2.0 thấy 0.2.1, ⌘D ⇒ tải, thay `Groovex.app`, bật lại 0.2.1, hộp hiện — mục 9.
- C6 [owner-qa] Windows: `_setup.exe /S /UPDATE` chờ app thoát rồi bật lại; nhìn nút / thẻ Giới thiệu sáng / tối, vi / en.

## 7. Ghi chú UI

- Nút: viền màu nhấn mờ, chữ + icon màu nhấn (G-4), một độ đậm (G-6), cao 26, đứng trước cụm nút cửa sổ; đang tải "Đang tải n%", không bấm được.
- Khoá i18n vùng `update` (`button`, `tooltip`, `about.*`, `whats_new.*`, `menu.*`).

## 9. Bàn giao thực tế (điền khi xong)

**Đã làm** (nhánh `claude/groove-gd2_05_update`):
- Crate chung `materiant/updater` (không GPUI, ferry dùng cả ở daemon khay): `AppSpec`, soát manifest, sổ + lịch soát, tải + sha256,
  cài macOS / Windows / Linux, `notes` cho hộp "Có gì mới", server giả `updater/tools/fake-server.mjs`. Server `cloud`: app `groove`
  đăng ký (`migrations/groove/0001_register.sql`, `deploy/groove/`, fixture khoá dev); khoá cập nhật prod sinh ở
  `cloud/secrets/groove/update-prod-key.json` (ngoài git), khoá công khai nhúng ở `update::PROD_UPDATE_KEY`.
- groove: `update.rs` (global `Updater`, `check_now`, `start_install`, `badge`), `update/whats_new.rs` (`Slot::WhatsNew`),
  `views/settings/about.rs` (`SettingsTab::About`), `app_menu.rs` (menu macOS), nút trên thanh trên, ⌘D / Ctrl+D ở `commands::on_keystroke`.

**Opus chọn 13/11/2026** (D-9):
- Crate chung đặt trong repo materiant (như `devtool`, D-25) vì groove / ferry đã trỏ `../materiant`; không dựa GPUI để daemon khay
  của ferry dùng được. fastcast giữ bản riêng (đã chạy thật, cùng hợp đồng) — chuyển sang crate chung là việc sau.
- Server `https://cloud.groovex.sandtechs.com` theo D-27 (mỗi app một tên miền; dời server chỉ đổi DNS). **Chủ dự án cần thêm DNS
  + khối Caddy cho tên này** (cùng container `fastcast-cloud`) và `bash cloud/deploy/groove/deploy.sh` để khoá cập nhật lên server;
  tới lúc đó app soát lỗi mạng, chỉ ghi log.
- Nút cập nhật là nút icon tròn (như fastcast), không phải nút chữ: ô tìm canh giữa 40 % thanh, cửa sổ ~800 pt thì nút chữ
  "Cập nhật lên v0.2.1" đè lên ô tìm (thấy ở E2E). Chữ + phím ở tooltip, đang tải hiện "n%".
- `dev_build = cfg!(debug_assertions)`: bản thử `.app` build với `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` (đã là cách đo RAM
  của repo) ⇒ tin khoá dev, đọc `GROOVE_DATA_DIR` / `GROOVE_CLOUD_URL`; không thêm feature.
- macOS bật lại qua `open -a` (LaunchServices) không mang biến môi trường ⇒ `updater` bản dev chuyển `<APP>_*` bằng `open --env`
  (không thì bản thử bật lại dùng dữ liệu thật `~/Library/Application Support/Groove X` của máy).
- `main.rs` ghi "task sau không sửa": thêm hai dòng `mod app_menu; mod update;` (không có chỗ khác khai mod mới).

**E2E macOS arm64, 13/11/2026** (server giả `fake-server.mjs` ký khoá dev, `.app` trong scratchpad, dữ liệu `GROOVE_DATA_DIR` tạm):
```
15:49:39 update: 0.2.1 available (Patch)                       # bản 0.2.0, GET …/latest?os=macos&arch=arm64&plan=free&current=0.2.0
15:49:45 update: 0.2.1 pending from the last check             # mở lại: đọc sổ, không hỏi mạng
KEY cmd-d                                                      # GET …/files/groovex_0.2.1_macos_arm64_draft.app.zip
15:50:02 update: 0.2.1 installed, quitting so it can start
15:50:04 whats new: 0.2.1 (1 notes)                            # tiến trình mới: …/Groovex.app/Contents/MacOS/groove --updated
```
`Info.plist` của gói sau cài: 0.2.1; `last-version` 0.2.1; hộp "Có gì mới" + nút icon soát bằng ảnh chụp đúng cửa sổ app.
Bẫy: `--display secondary` trên máy một màn ⇒ app thoát ngay; cửa sổ `--background` bị che thì macOS không vẽ, ảnh chụp là khung cũ.

**Chưa làm / chờ** (`[owner-qa]`):
- Windows: `_setup.exe /S /UPDATE` chưa chạy thật (NSIS chờ exe hết khoá rồi bật `groove.exe --updated`).
- Linux: chỉ mở trang tải (`.deb` ở `/usr`), chưa có `.tar.gz` / cài thư mục người dùng.
- Xem tay thẻ Giới thiệu sáng / tối, vi / en; menu macOS "Kiểm tra cập nhật…".
- DNS + Caddy `cloud.groovex.sandtechs.com`, deploy khoá, đăng bản thật — chủ dự án.
