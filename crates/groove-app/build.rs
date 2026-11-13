//! groove-gd1_27: nhúng icon vào exe Windows. OS khác lấy icon từ gói (`packaging/`).
//! groove-gd2_01: thêm VERSIONINFO (tên sản phẩm, mô tả, nhà phát hành, phiên bản lấy từ `Cargo.toml`): Explorer, Task Manager,
//! "Mở bằng" hiện "Groovex" thay vì "groove.exe".
//! groove-gd2_05: nhúng release note của bản đang build (`release-notes/v<version>.md`, D-22) cho hộp "Có gì mới"; chưa có tệp
//! (version chưa tag) ⇒ `None`.
//! file-icon (04/11/2026): icon thứ hai `groove-file.ico` (`groove.exe,1`) cho tệp nhạc đã liên kết: cùng hình, nhỏ hơn, có lề
//! trong suốt (`tools/make-file-icon.ps1`) — icon app lấp kín ô 16 px của Explorer, trông to hơn icon tệp khác.

use std::path::PathBuf;

const PRODUCT: &str = "Groovex";
/// D-21: nhà phát hành, bản quyền chung mọi app (cùng `packaging/windows/groove.nsi`, `tests/packaging.rs` soát).
const COMPANY: &str = "z0 team, sandtechs jsc.";
const COPYRIGHT: &str = "Copyright © 2026 Sandtechs";

fn main() {
    release_note();
    println!("cargo:rerun-if-changed=assets/app/groove.ico");
    println!("cargo:rerun-if-changed=assets/app/groove-file.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let icon = manifest.join("assets/app/groove.ico");
    let file_icon = manifest.join("assets/app/groove-file.ico");
    let rc = out.join("groove.rc");
    let script = resource_script(&icon.to_string_lossy(), &file_icon.to_string_lossy(), &version());
    std::fs::write(&rc, script).expect("write groove.rc");
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().expect("compile the Windows resources");
}

/// `(major, minor, patch, chuỗi)` của crate.
fn version() -> (u16, u16, u16, String) {
    let part = |name: &str| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    (
        part("CARGO_PKG_VERSION_MAJOR"),
        part("CARGO_PKG_VERSION_MINOR"),
        part("CARGO_PKG_VERSION_PATCH"),
        std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
    )
}

/// id nhỏ nhất là icon Explorer, taskbar hiện cho exe; id 2 là icon tệp (chỉ số 1 trong `DefaultIcon`, đếm theo thứ tự id).
/// `code_page(65001)`: tệp ghi UTF-8, `©` của bản quyền không thành ký tự lạ. Chuỗi trong `.rc` thoát `\` và `"`.
fn resource_script(icon: &str, file_icon: &str, (major, minor, patch, text): &(u16, u16, u16, String)) -> String {
    let escape = |path: &str| path.replace('\\', "\\\\").replace('"', "\"\"");
    let (icon, file_icon) = (escape(icon), escape(file_icon));
    format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"
2 ICON "{file_icon}"

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "{COMPANY}"
      VALUE "FileDescription", "{PRODUCT}"
      VALUE "FileVersion", "{text}"
      VALUE "InternalName", "groove"
      VALUE "LegalCopyright", "{COPYRIGHT}"
      VALUE "OriginalFilename", "groove.exe"
      VALUE "ProductName", "{PRODUCT}"
      VALUE "ProductVersion", "{text}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}

fn release_note() {
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("../../release-notes");
    let note = dir.join(format!("v{version}.md"));
    // Theo dõi cả thư mục: tệp của version đang build có thể được tạo sau lần build trước.
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed={}", note.display());
    let value = match note.canonicalize() {
        Ok(path) => format!("Some(include_str!({:?}))", path.display().to_string()),
        Err(_) => "None".to_owned(),
    };
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("release_note.rs");
    std::fs::write(out, format!("pub const RELEASE_NOTE: Option<&str> = {value};\n")).expect("write release_note.rs");
}
