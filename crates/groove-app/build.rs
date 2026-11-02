//! groove-gd1_27: nhúng icon vào exe Windows. OS khác lấy icon từ gói (`packaging/`).

fn main() {
    println!("cargo:rerun-if-changed=assets/app/groove.rc");
    println!("cargo:rerun-if-changed=assets/app/groove.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/app/groove.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("compile the Windows icon resource");
    }
}
