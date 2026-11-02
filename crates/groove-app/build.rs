//! groove-gd1_27: nhúng icon vào exe Windows. OS khác lấy icon từ gói (`packaging/`).
//! groove-gd2_01: thêm VERSIONINFO (tên sản phẩm, mô tả, nhà phát hành, phiên bản lấy từ `Cargo.toml`): Explorer, Task Manager,
//! "Mở bằng" hiện "Groove X" thay vì "groove.exe".

use std::path::PathBuf;

const PRODUCT: &str = "Groove X";

fn main() {
    println!("cargo:rerun-if-changed=assets/app/groove.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let icon = manifest.join("assets/app/groove.ico");
    let rc = out.join("groove.rc");
    std::fs::write(&rc, resource_script(&icon.to_string_lossy(), &version())).expect("write groove.rc");
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

/// id nhỏ nhất là icon Explorer, taskbar hiện cho exe. Chuỗi trong `.rc` thoát `\` và `"`.
fn resource_script(icon: &str, (major, minor, patch, text): &(u16, u16, u16, String)) -> String {
    let icon = icon.replace('\\', "\\\\").replace('"', "\"\"");
    format!(
        r#"1 ICON "{icon}"

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
      VALUE "CompanyName", "{PRODUCT}"
      VALUE "FileDescription", "{PRODUCT}"
      VALUE "FileVersion", "{text}"
      VALUE "InternalName", "groove"
      VALUE "LegalCopyright", "Copyright (C) {PRODUCT}"
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
