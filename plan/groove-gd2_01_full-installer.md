# groove-gd2_01 — full-installer

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | 🔄 |
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

## 6. Nghiệm thu (gạch, run-task viết Case test)

- Test đóng gói đọc `.nsi`, `.desktop`, `Info.plist` và so đúng `AUDIO_EXTENSIONS` (mở rộng `tests/packaging.rs`).
- `installer-e2e.ps1` mở rộng: cài im lặng vào thư mục tạm, soát khoá `Capabilities`, `RegisteredApplications`, ProgID,
  lối tắt + AppUserModelID, mục gỡ; gỡ xong không sót gì. Không đổi `UserChoice` thật, không đụng liên kết mặc định đang
  dùng của máy; máy đã cài Groove X thật thì từ chối như cũ.
- Dựng bản cài thật bằng `build.ps1 -Project groove`.
- `[owner-qa]`: cài thật, chọn Groove X trong Default apps, nhấp đúp `.mp3` mở đúng app; `.deb` trên Ubuntu; `.dmg` trên Mac.
