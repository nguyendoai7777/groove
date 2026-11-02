//! groove-gd1_27: khai báo đóng gói không được lệch mã. Mọi OS đều chạy được, kể cả phần của OS khác: chỉ đọc tệp.

use std::fs;
use std::path::PathBuf;

use groove_core::model::AUDIO_EXTENSIONS;

fn read(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    // Windows có thể checkout tệp văn bản thành CRLF.
    fs::read_to_string(&full).unwrap_or_else(|err| panic!("{}: {err}", full.display())).replace("\r\n", "\n")
}

/// Mọi đuôi ghép lúc cài cũng phải được dọn lúc gỡ, và không thừa đuôi lạ.
#[test]
fn nsis_associates_and_forgets_exactly_the_audio_extensions() {
    let nsi = read("packaging/windows/groove.nsi");
    for macro_name in ["AssociateExtension", "ForgetExtension"] {
        let call = format!("!insertmacro {macro_name} \"");
        let found: Vec<&str> =
            nsi.lines().filter_map(|l| l.trim().strip_prefix(call.as_str())?.strip_suffix('"')).collect();
        assert_eq!(found, AUDIO_EXTENSIONS, "{macro_name}");
    }
}

#[test]
fn nsis_only_registers_open_with_never_the_default() {
    let nsi = read("packaging/windows/groove.nsi");
    assert!(nsi.contains("\\OpenWithProgids\" \"${PROG_ID}\""));
    // Ghi giá trị mặc định của `.mp3` là cướp liên kết, đặc tả 5.6 không cho.
    assert!(!nsi.contains("${CLASSES}\\.${EXT}\" \"\""));
    assert!(!nsi.contains("HKLM"), "cài theo người dùng");
}

#[test]
fn info_plist_declares_every_extension_and_the_right_exe() {
    let plist = read("packaging/macos/Info.plist");
    for ext in AUDIO_EXTENSIONS {
        assert!(plist.contains(&format!("<string>{ext}</string>")), "{ext}");
    }
    assert!(plist.contains("<key>CFBundleExecutable</key>\n  <string>groove</string>"));
    assert!(plist.contains("<key>CFBundleTypeRole</key>"));
}

#[test]
fn desktop_entry_opens_many_files_and_covers_each_format() {
    let desktop = read("packaging/linux/groove.desktop");
    assert!(desktop.lines().any(|l| l == "Exec=groove %F"));
    let mime = desktop.lines().find_map(|l| l.strip_prefix("MimeType=")).expect("MimeType");
    for (ext, kind) in [
        ("mp3", "audio/mpeg"),
        ("flac", "audio/flac"),
        ("wav", "audio/wav"),
        ("m4a", "audio/mp4"),
        ("m4r", "audio/x-m4r"),
        ("ogg", "audio/ogg"),
    ] {
        assert!(AUDIO_EXTENSIONS.contains(&ext), "{ext}");
        assert!(mime.split(';').any(|m| m == kind), "{kind}");
    }
    assert_eq!(AUDIO_EXTENSIONS.len(), 6, "đuôi mới thì thêm MIME ở đây và trong `.desktop`");
    assert!(mime.ends_with(';'), "chuẩn desktop entry đòi `;` cuối danh sách");
}

/// G-2: khai phụ thuộc, không kèm, có trên trang giấy phép.
#[test]
fn libasound_follows_g2() {
    let deb = read("packaging/linux/build-deb.sh");
    assert!(deb.contains("depends=\"libasound2 | libasound2t64"));
    assert!(deb.contains("Depends: $depends"));
    assert!(deb.contains("NEEDED.*libasound"), "soát exe link động");
    assert!(deb.contains("libasound không được nằm trong gói"), "soát không kèm");
    let licenses = read("packaging/licenses/gen-licenses.ps1");
    assert!(licenses.contains("Name = 'ALSA library (libasound)'"));
    let manifest = read("crates/groove-audio/Cargo.toml");
    assert!(manifest.contains("linux-alsa"));
}

/// `about.toml` và `deny.toml` phải soát cùng các target.
#[test]
fn license_tools_cover_the_same_targets() {
    let targets = |text: String| -> Vec<String> {
        let start = text.find("targets = [").expect("targets");
        let block = &text[start..];
        let end = block.find(']').expect("]");
        let mut found: Vec<String> =
            block[..end].lines().filter_map(|l| Some(l.split('"').nth(1)?.to_owned())).collect();
        found.sort();
        found
    };
    assert_eq!(targets(read("packaging/licenses/about.toml")), targets(read("deny.toml")));
}
