# groove-gd1_10 — audio-engine

| Thuộc tính | Giá trị |
|---|---|
| Trạng thái | ☐ |
| Repo | groove |
| Model | opus |
| Cỡ | ~850 dòng |
| Phụ thuộc | groove-gd1_01 |
| Quyết định áp dụng | D-2, D-9, D-11, D-13, D-14, G-1 |
| Phụ thuộc mới | Theo mục 5 của đặc tả; soát giấy phép ở `groove-gd1_01` |
| File đụng tới | `audio/{engine,decoder,output}.rs` |

Đặc tả: `docs/rewrite-spec.md`, bảng mục 6 dòng 10. Đọc mục liên quan của đặc tả trước khi code; viết tắt đường dẫn ở cuối mục 6.

## 1. Làm gì
- Làm đúng hành vi ở mục 1 và dữ liệu ở mục 2 của đặc tả cho phạm vi "File đụng tới".
- Linux: quyết định `libasound` (LGPL, link động) đang chờ chủ dự án. Làm và kiểm trên Windows, code Linux biên dịch sau `cfg`, chưa coi Linux là đạt.
- Thử HE-AAC với thư viện thật, ghi tỉ lệ tệp không phát được ở mục 9.
- Chỉ sửa tệp mình sở hữu; thiếu API từ task khác thì ghi mục 8, không sửa tệp của làn khác.
- Chữ giao diện qua i18n `vi` + `en`; lưới và danh sách dùng `materiant::virtual_grid` / `materiant::list`.

## 2. Không làm trong task này
- Không thêm tính năng ngoài đặc tả; không sửa bản Tauri + Vue (G-1).