# groove — lộ trình

Viết lại trình phát nhạc Groovex từ Tauri + Vue sang Rust + GPUI + materiant (G-1). Đặc tả, lỗi cần sửa, bảng task và làn
song song (đã làm xong, đọc qua lịch sử git): `git show 1a2480f:docs/rewrite-spec.md`. Task: `plan/groove-gd1_*.md`, trạng thái ở từng tệp task.

## GĐ1 — viết lại (02/11/2026)

| Task | Trạng thái |
| --- | --- |
| 01-27 | ✅, đã gộp `master` |
| 28 `localstorage-import` (tuỳ chọn) | ☐ chưa làm: bản mới chạy với mặc định, `legacy_ls.rs` là tệp rỗng, phụ thuộc `rusty-leveldb` đã khai |
| 29 `acceptance-remove-tauri` | ☐ (quy ước không có trạng thái "chờ") mã, bảng so bản cũ (`git show 1a2480f:docs/parity.md`), số đo RAM đã gộp; gỡ Tauri nằm ở nhánh `claude/remove-tauri`, chờ chủ dự án xem app thật |

Còn lại trước khi đóng GĐ1:

1. Chủ dự án xem app thật ở máy hai màn: các dòng `[owner-qa]` ở `plan/_backlog.md`, bắt đầu từ dòng đầu tiên.
2. Gộp `claude/remove-tauri` (lệnh ở dòng `[owner-qa]` đầu tiên), đánh dấu task 29 ✅, rồi `close-phase`.
3. macOS, Ubuntu: dựng gói, chạy test một phiên, media session (các dòng `[groove-gd1_25]`, `[groove-gd1_26]`, `[groove-gd1_27]`).

## GĐ2 — bản cài hoàn chỉnh (02/11/2026)

| Task | Trạng thái |
| --- | --- |
| `groove-gd2_01` `full-installer` | ✅ |

### Bàn giao GĐ2

<!-- handoff: groove-gd2_01 -->

- **Đã có**: bộ cài Windows đăng ký Default apps (`Capabilities` ở `HKCUSoftwareClientsMediaGroovex`, ProgID `GrooveX.<đuôi>`),
  lối tắt mang AppUserModelID `Sandtechs.Groovex`; mục Settings "Trình phát mặc định" (`platform/default_player.rs`). Thêm đuôi mới
  ⇒ sửa `AUDIO_EXTENSIONS`, hai danh sách trong `groove.nsi`, `$extensions` của `installer-e2e.ps1`, `MimeType` của `.desktop`,
  `Info.plist`: `tests/packaging.rs` bắt lệch.
- **Bẫy**: `installer-e2e.ps1` ghi HKCU thật rồi dọn và từ chối khi máy đã cài Groovex; đừng thử cài bằng tay trên máy dùng thật.
  LangString rỗng của NSIS bị coi là chưa đặt (`/WX` đỏ). Dựng `.deb` từ checkout Windows phải bỏ CR (đã xử lý trong script).
- **Chờ**: hai dòng `[groove-gd2_01 · owner-qa]` ở `plan/_backlog.md`; mục 8 của `plan/groove-gd2_01_full-installer.md`.

## Sau GĐ2 (chưa lập task)

- Mục chuột phải "Phát bằng Groovex" cho thư mục, verb "Thêm vào hàng đợi" (mục 8 của `groove-gd2_01`).

## Sau GĐ1 (chưa lập task)

- Task 28 nếu chủ dự án muốn giữ EQ, phím tắt của bản Tauri.
- Giao diện xoá bài mất vĩnh viễn trước hạn 30 ngày (`reconcile::forget_missing` đã có, chưa có đường vào từ UI).
- Đo RAM với thư viện thật và khi đang phát (`CLAUDE.md` mục Bẫy, Số đo RAM).
