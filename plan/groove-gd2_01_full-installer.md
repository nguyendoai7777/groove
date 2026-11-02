# groove-gd2_01 — full-installer

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ✅ (02/11/2026) |
| Repo | groove |
| Model | opus |
| Cỡ | M — ~600 dòng (NSIS, `.desktop`, `Info.plist`, mã nút "đặt mặc định", test) |
| Phụ thuộc | groove-gd1_27, groove-gd1_29 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1, G-2 |
| Phụ thuộc mới | Không dự kiến; cần thì soát giấy phép, ghi mục 9 |
| File đụng tới | `packaging/windows/*`, `packaging/linux/*`, `packaging/macos/*`, `crates/groove-app/build.rs`, `crates/groove-app/src/platform/*` (MỚI: phần đặt mặc định), `crates/groove-app/src/views/settings/*` (một mục), `i18n/{vi,en}/settings.toml`, `crates/groove-app/tests/packaging.rs`, `../build.ps1`, `../build.sh` nếu cần |

Chủ dự án (02/11/2026): app chạy thật đã ổn ~8/10 (UI làm sau). Muốn `build` ra **bộ cài hoàn chỉnh như một app nghe nhạc
chính thức**, dùng được làm **trình phát mặc định**. Task 27 mới có ProgID + `OpenWithProgids` (chỉ hiện trong "Mở bằng").

## 1. Làm gì

- **Windows (NSIS, cài theo người dùng, không cần admin):**
  - Lối tắt Start Menu (và Desktop là tuỳ chọn trên trang chọn thành phần), gắn AppUserModelID trùng với ID app dùng
    cho media session / taskbar (task 26), để Windows gom đúng một biểu tượng.
  - Đăng ký **Default Programs**: `Capabilities` (tên, mô tả, icon, `FileAssociations` cho mọi đuôi trong
    `AUDIO_EXTENSIONS`), `RegisteredApplications`; ProgID có icon tệp, tên loại tệp đọc được, verb `open` (+ verb "Thêm vào
    hàng đợi" chỉ khi app thật sự hỗ trợ). Sau khi ghi gọi `SHChangeNotify`. Groove X phải hiện trong **Settings → Default
    apps** và chọn được cho từng đuôi.
  - Trang cuối: "Mở Groove X" và "Đặt Groove X làm trình phát mặc định" — mở đúng trang Default apps của app (Win11:
    `ms-settings:defaultapps?registeredAppUser=…`; Win10: trang tương ứng). Windows **không cho tự đặt mặc định im lặng**:
    không ghi đè `UserChoice`, không dùng mẹo băm.
  - Thư mục: mục chuột phải "Phát bằng Groove X" nếu `open_paths` nhận thư mục (kiểm, không có thì ghi mục 8).
  - Version info trong exe (ProductName, FileDescription, CompanyName, phiên bản từ `Cargo.toml`), mục Gỡ cài đặt có icon,
    phiên bản, cỡ, nhà phát hành. Cài đè bản cũ giữ nguyên dữ liệu; gỡ thì hỏi có xoá thư viện / cài đặt không (mặc định giữ),
    dọn sạch registry, lối tắt.
- **Trong app:** một mục ở Settings "Đặt làm trình phát mặc định": cho biết app đang là mặc định cho mp3 hay chưa và mở trang
  hệ thống tương ứng (Windows: trang Default apps; Linux: `xdg-mime default` cho người dùng hiện tại sau khi bấm; macOS:
  đặt handler sau khi bấm). Không bao giờ tự đổi khi người dùng chưa bấm.
- **Linux `.deb`:** `.desktop` khai đủ `MimeType` khớp `AUDIO_EXTENSIONS`, `Categories=AudioVideo;Audio;Player;`, icon
  hicolor nhiều cỡ; app hiện trong "Mở bằng" và chọn làm mặc định được.
- **macOS `.app` / `.dmg`:** `CFBundleDocumentTypes` đủ đuôi; app chọn được làm mặc định qua Get Info → Change All.
- `pwsh ./build.ps1 -Project groove` / `bash build.sh --project groove` (repo gốc) ra đúng bản cài hoàn chỉnh của OS đang chạy.

## 2. Không làm trong task này

- Ký code, notarize, chứng chỉ (dịch vụ trả phí). Ghi rõ cảnh báo SmartScreen / Gatekeeper trong tài liệu.
- Tự đặt mặc định không qua người dùng; thêm định dạng mà decoder chưa phát được; sửa UI cho giống web (làm sau).

## 2b. Phân vai

Task `opus` (D-14), không tách subagent: registry Windows, COM của lối tắt, FFI LaunchServices, luồng nền của mục Settings đều cần soát kỹ.
`[opus]` toàn bộ `packaging/*`, `build.rs`, `platform/default_player.rs`, `views/settings/default_player.rs`, i18n, test. `[owner-qa]`:
cài thật, chọn trong Default apps, nhấp đúp; `.deb` trên Ubuntu có màn; `.dmg` trên Mac.

## 6. Nghiệm thu

| Case | Cho / Khi / Thì | Phủ |
| --- | --- | --- |
| C1 | `.nsi`, `.desktop`, `Info.plist`, `installer-e2e.ps1` ⇒ cùng đúng `AUDIO_EXTENSIONS`; ghép nào cũng có gỡ; tên loại tệp là đuôi viết hoa | `[opus-unit]` `tests/packaging.rs` |
| C2 | `.nsi` ⇒ ghi `Capabilities\FileAssociations`, `RegisteredApplications`, gỡ thì bỏ; chỉ HKCU; không ghi giá trị mặc định của `.<đuôi>`, không có `UserChoice` | `[opus-unit]` `nsis_registers_with_default_apps_but_never_takes_the_default` |
| C3 | AUMID, URI Default apps, tên đăng ký trong `.nsi` ⇒ trùng hằng trong `platform/default_player.rs`; bundle id `Info.plist` trùng mã macOS | `[opus-unit]` `nsis_and_app_share_the_identity`, `info_plist_…` |
| C4 | Exe Windows ⇒ ProductName, FileDescription, CompanyName "Groove X", phiên bản = `Cargo.toml` | `[opus-run]` `installer-e2e.ps1` |
| C5 | Mục Settings: mở, hỏi lại nhiều lần ⇒ không đặt gì; bấm ⇒ gọi đặt đúng một lần, báo kết quả; lỗi ⇒ báo lỗi | `[opus-unit]` `views::settings::default_player::tests` |
| C6 | Bản cài thật, cài im lặng vào thư mục tạm ⇒ lối tắt Start Menu mang AUMID, ProgID mỗi đuôi, `OpenWithProgids`, `Capabilities`, `RegisteredApplications`, mục gỡ đủ icon, phiên bản, cỡ, nhà phát hành; Windows liệt kê Groove X trong "Mở bằng" và `QueryAppIsDefault` nhận app; `UserChoice` không đổi | `[opus-run]` `installer-e2e.ps1` |
| C7 | Exe bị giữ ⇒ cài đè, gỡ dừng mã 2; cài đè sau khi nhả ⇒ 0, AUMID còn; gỡ im lặng ⇒ registry, lối tắt y như trước khi cài | `[opus-run]` `installer-e2e.ps1` |
| C8 | `.deb` dựng trong `ubuntu:24.04`, cài bằng `apt` ⇒ `desktop-file-validate` sạch, MimeType đủ, icon hicolor 16…256, `xdg-mime query default audio/mpeg` = `groove.desktop` sau lệnh đặt của app | `[opus-run]` Docker |
| C9 | `Info.plist` đúng cú pháp plist | `[opus-run]` (không có Mac: chỉ cú pháp) |
| C10 | Cài thật, Default apps chọn Groove X, nhấp đúp `.mp3`; trang cuối bộ cài; `.deb` trên Ubuntu có màn; `.dmg` | `[owner-qa]` |

## 8. Nợ chuyển task khác

- Mục chuột phải "Phát bằng Groove X" cho **thư mục**: chưa làm vì `open_paths::resolve` chỉ nhận tệp có đuôi nhạc (bỏ thư mục).
  Cần: `open_paths` nhận thư mục, quét ở luồng nền (không chạm đĩa trên UI thread), rồi thêm verb `Directory\shell\GrooveX.Play`
  vào `groove.nsi` + gỡ. Chưa có task; ghi ở `ROADMAP.md` mục "Sau GĐ2".
- Verb "Thêm vào hàng đợi": app chưa có cờ dòng lệnh xếp hàng (`instance` chỉ chuyển "mở"); thêm cờ rồi mới thêm verb.

## 9. Bàn giao thực tế

- **Windows, bộ cài** (`packaging/windows/groove.nsi`): trang chọn thành phần (Groove X bắt buộc, lối tắt Desktop tuỳ chọn, mặc
  định không tích, cài đè giữ lựa chọn cũ) → thư mục → cài → trang cuối "Mở Groove X" + "Đặt Groove X làm trình phát nhạc mặc
  định" (mở `ms-settings:defaultapps?registeredAppUser=Groove%20X`). Mỗi đuôi một ProgID `GrooveX.<đuôi>` ("Audio file (MP3)" /
  "Tệp nhạc (MP3)", icon, `MultiSelectModel=Player`, `AppUserModelID`), `OpenWithProgids`, `Applications\groove.exe`,
  `HKCU\Software\Clients\Media\Groove X\Capabilities` + `RegisteredApplications`, rồi `SHChangeNotify`. ProgID chung
  `GrooveX.AudioFile` của gd1_27 tự dọn khi cài đè. Lối tắt Start Menu / Desktop được gắn `System.AppUserModel.ID =
  GrooveX.Player` bằng COM qua plugin System (`SetShortcutAumid`); `platform::default_player::set_process_identity` đặt cùng ID
  cho tiến trình ở đầu `app::run`. Gỡ: hỏi xoá `%APPDATA%\Groove X` + `%LOCALAPPDATA%\Groove X` (mặc định No, im lặng thì giữ),
  dọn registry, lối tắt. Mục gỡ có icon, phiên bản, cỡ, nhà phát hành, mô tả. Không ghi `UserChoice`, không HKLM (test soát).
- **Exe**: `build.rs` sinh `groove.rc` trong `OUT_DIR` với ICON + VERSIONINFO (ProductName, FileDescription, CompanyName "Groove X",
  phiên bản từ `Cargo.toml`); bỏ `assets/app/groove.rc`.
- **Trong app**: Settings → Cài đặt chung → "Trình phát mặc định" (`views/settings/default_player.rs`, logic
  `platform/default_player.rs`). Hỏi trạng thái ở luồng nền khi mở hộp và mỗi lần cửa sổ được kích hoạt lại. Windows:
  `AssocQueryStringW(ASSOCSTR_EXECUTABLE, ".mp3")` so với exe đang chạy; chưa có giá trị `RegisteredApplications\Groove X` thì
  "chưa cài", khoá nút. Nút mở trang Default apps bằng `ShellExecuteW`. Linux: tìm `groove.desktop` theo XDG, `xdg-mime query`,
  bấm thì `xdg-mime default groove.desktop <MimeType của tệp đã cài>`. macOS: `LSCopyDefaultRoleHandlerForContentType` /
  `LSSetDefaultRoleHandlerForContentType` theo UTI của từng đuôi, chỉ khi chạy từ `Groove X.app`. Chỉ đổi khi bấm (test C5).
- **Linux**: icon hicolor 16, 24, 32, 48, 64, 128, 256 ở `packaging/linux/icons/` (16, 24, 48 tách từ `groove.ico`; 32, 64, 128 từ
  `tauri-final:src-tauri/icons`; 256 là `groove.png` cũ). `build-deb.sh` bỏ CR của `.desktop`, chạy `desktop-file-validate` nếu có,
  đọc được version từ `Cargo.toml` CRLF (lỗi gặp khi dựng từ checkout Windows: `Version` rỗng, sửa luôn, cả `build-macos.sh`).
- **macOS**: `Info.plist` thêm `LSApplicationCategoryType` (music), `NSHumanReadableCopyright`; giữ `CFBundleTypeExtensions` +
  `LSHandlerRank=Alternate` (chọn được qua Get Info → Change All, không giành mặc định).
- **Repo gốc**: không sửa `build.ps1`, `build.sh`. `build-installer.ps1` tìm `makensis` theo checkout chính của repo
  (`git rev-parse --git-common-dir`), để chạy được từ worktree.
- **Đã chạy (Windows, cây đã gộp `master`)**: `cargo build`; `cargo clippy --all-targets -- -D warnings` (Rust 1.99.1);
  `cargo test --workspace --no-fail-fast` xanh lần đầu, 709 test (3 + 1 ignored); `cargo fmt --all --check`;
  `cargo deny check licenses`. Bản cài dựng từ worktree (`build-installer.ps1`, 7,2 MB) chạy `installer-e2e.ps1` xanh 76 mục (lần
  đầu đỏ một mục do script e2e hỏi tên app sai cách, đã sửa script; lần đó cũng thấy bước dọn của e2e chạy lại bộ gỡ thừa, đã
  sửa). Sau khi gộp (`389c7ba`): `pwsh ./build.ps1 -Project groove` ở repo gốc ra `release/groove/groove-x-0.2.0-setup.exe` 7,2 MB; `installer-e2e.ps1` trên bản đó xanh 76 mục, chạy hai lần liền (lần hai qua được bước từ chối máy đã cài, tức lần một dọn sạch); soát tay sau đó: không còn khoá gỡ, `ClientsMediaGroove X`, giá trị `RegisteredApplications`, ProgID `GrooveX*`, `Applicationsgroove.exe`, lối tắt.
- **Đã chạy (Linux, Docker `ubuntu:24.04`, gói cài bằng `init.sh groove`)**: `build-deb.sh --draft` ra
  `groove-x_0.2.0_amd64-draft.deb` 13 MB (bản `-draft` vì container không có `pwsh` + `cargo-about`); `apt install` gói;
  `desktop-file-validate` sạch; `mimeinfo.cache` có `groove.desktop` cho 10 MIME; 7 icon hicolor đúng cỡ; exe link động
  `libasound.so.2`; `gio mime audio/mpeg` liệt kê Groove X; đặt `other.desktop` làm mặc định rồi chạy mã thật của app
  (`cargo test -p groove-app linux_make_default -- --ignored`): trước NotDefault, sau Default, `xdg-mime query default` trả
  `groove.desktop` cho cả 10 MIME; `apt remove` dọn `.desktop`, icon, cache. `cargo clippy -p groove-app --all-targets -- -D
  warnings` và test `default_player` xanh trên Linux. `Info.plist` qua `plistutil` (chuyển sang dạng nhị phân được).
- **Chưa chạy**: `build-macos.sh`, `.dmg`, mã macOS của `default_player` chưa biên dịch (`cargo check --target
  aarch64-apple-darwin` dừng ở `libsqlite3-sys` vì máy Windows không có `cc` cho Apple); bộ cài trên màn thật (trang chọn thành
  phần, trang cuối, hộp hỏi xoá dữ liệu lúc gỡ có giao diện); chọn Groove X trong Default apps; nhấp đúp tệp; cửa sổ app thật
  (máy một màn); `.deb` trên Ubuntu có màn.
- **Soi cải tiến**: bảo mật: chỉ HKCU, lệnh mở bọc `"%1"`, `xdg-mime` gọi trực tiếp không qua shell, MIME lấy từ tệp `.desktop`
  của hệ thống chứ không từ đầu vào ngoài; e2e có bước dọn khi dừng giữa chừng. UI/UX: nút là `Button` của materiant (bàn phím
  được), chữ nói rõ vì sao bản dev không đặt được, Windows nói rõ người dùng phải chọn ở trang vừa mở. RAM: không phụ thuộc mới,
  không luồng thường trú; hỏi trạng thái chỉ khi mở hộp và khi cửa sổ được kích hoạt. Pro: không áp.
- **Opus chọn 02/11/2026**: các dòng `groove-gd2_01` ở `plan/confirm.md`.

## 10. Đề xuất cải tiến

Không có đề xuất ngoài phạm vi đáng ghi (mục chuột phải cho thư mục, verb xếp hàng: mục 8).
