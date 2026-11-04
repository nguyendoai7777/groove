//! groove-gd1_27, gd2_01: khai báo đóng gói không được lệch mã. Mọi OS đều chạy được, kể cả phần của OS khác: chỉ đọc tệp.

use std::fs;
use std::path::PathBuf;

use groove_core::model::AUDIO_EXTENSIONS;

fn read(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    // Windows có thể checkout tệp văn bản thành CRLF.
    fs::read_to_string(&full).unwrap_or_else(|err| panic!("{}: {err}", full.display())).replace("\r\n", "\n")
}

/// Đối số trong ngoặc kép của mọi dòng `!insertmacro <tên> "…" "…"`.
fn macro_args<'a>(text: &'a str, name: &str) -> Vec<Vec<&'a str>> {
    let call = format!("!insertmacro {name} ");
    text.lines()
        .filter_map(|line| line.trim().strip_prefix(call.as_str()))
        .map(|args| args.split('"').skip(1).step_by(2).collect())
        .collect()
}

/// Giá trị của `!define NAME "…"`.
fn nsis_define<'a>(nsi: &'a str, name: &str) -> &'a str {
    let prefix = format!("!define {name} \"");
    nsi.lines()
        .find_map(|line| line.trim().strip_prefix(prefix.as_str())?.strip_suffix('"'))
        .unwrap_or_else(|| panic!("!define {name}"))
}

/// Mọi đuôi ghép lúc cài cũng phải được dọn lúc gỡ, và không thừa đuôi lạ; tên loại tệp là đuôi viết hoa.
#[test]
fn nsis_associates_and_forgets_exactly_the_audio_extensions() {
    let nsi = read("packaging/windows/groove.nsi");
    let associate = macro_args(&nsi, "AssociateExtension");
    let found: Vec<&str> = associate.iter().map(|args| args[0]).collect();
    assert_eq!(found, AUDIO_EXTENSIONS, "AssociateExtension");
    for args in &associate {
        assert_eq!(args.len(), 2, "{args:?}");
        assert_eq!(args[1], args[0].to_uppercase(), "tên loại tệp của .{}", args[0]);
    }
    let forget: Vec<&str> = macro_args(&nsi, "ForgetExtension").iter().map(|args| args[0]).collect();
    assert_eq!(forget, AUDIO_EXTENSIONS, "ForgetExtension");
    // Hai danh sách được gọi đúng một lần: lúc cài, lúc gỡ.
    assert_eq!(nsi.matches("  !insertmacro AssociateAll\n").count(), 1);
    assert_eq!(nsi.matches("  !insertmacro ForgetAll\n").count(), 1);
}

/// Đặc tả 5.6: hiện trong "Mở bằng" và Default apps, nhưng không bao giờ tự đặt mặc định.
#[test]
fn nsis_registers_with_default_apps_but_never_takes_the_default() {
    let nsi = read("packaging/windows/groove.nsi");
    assert!(nsi.contains(r#"\OpenWithProgids" "${PROG_PREFIX}${EXT}" """#));
    assert!(nsi.contains(r#"WriteRegStr HKCU "${CAPABILITIES}\FileAssociations" ".${EXT}" "${PROG_PREFIX}${EXT}""#));
    assert!(nsi.contains(r#"WriteRegStr HKCU "${REGISTERED_APPS}" "${APP_NAME}" "${CAPABILITIES}""#));
    assert!(nsi.contains(r#"DeleteRegValue HKCU "${REGISTERED_APPS}" "${APP_NAME}""#), "gỡ thì bỏ đăng ký");
    assert!(nsi.contains("SHChangeNotify(i 0x08000000"), "báo Explorer đọc lại liên kết");
    // Ghi giá trị mặc định của `.mp3` hay `UserChoice` là cướp liên kết.
    assert!(!nsi.contains(r#"${CLASSES}\.${EXT}" """#));
    let code: Vec<&str> = nsi.lines().map(str::trim).filter(|line| !line.starts_with('#')).collect();
    assert!(!code.iter().any(|line| line.contains("UserChoice")), "không đụng UserChoice");
    assert!(!nsi.contains("HKLM"), "cài theo người dùng");
    for line in nsi.lines().map(str::trim).filter(|line| line.starts_with("WriteReg") || line.starts_with("DeleteReg"))
    {
        assert!(line.contains(" HKCU "), "chỉ chạm HKCU: {line}");
    }
}

/// Lối tắt và tiến trình cùng AppUserModelID; trang Default apps trỏ đúng tên đã đăng ký.
#[test]
fn nsis_and_app_share_the_identity() {
    let nsi = read("packaging/windows/groove.nsi");
    let rust = read("crates/groove-app/src/platform/default_player.rs");
    let aumid = nsis_define(&nsi, "AUMID");
    assert!(rust.contains(&format!("APP_USER_MODEL_ID: &str = \"{aumid}\";")), "{aumid}");
    let uri = nsis_define(&nsi, "DEFAULT_APPS_URI");
    assert!(rust.contains(&format!("DEFAULT_APPS_URI: &str = \"{uri}\";")), "{uri}");
    let name = nsis_define(&nsi, "APP_NAME");
    assert!(rust.contains(&format!("REGISTERED_NAME: &str = \"{name}\";")), "{name}");
    assert!(uri.ends_with(&format!("registeredAppUser={}", name.replace(' ', "%20"))));
    // Lối tắt Start Menu và Desktop đều được gắn ID.
    assert_eq!(nsi.matches("Call SetShortcutAumid").count(), 2);
}

/// Bản soát e2e kiểm đúng các đuôi đó.
#[test]
fn installer_e2e_checks_every_extension() {
    let script = read("packaging/windows/installer-e2e.ps1");
    let line = script.lines().find(|line| line.starts_with("$extensions = [ordered]@{")).expect("$extensions");
    let found: Vec<&str> =
        line.split(['{', ';']).skip(1).filter_map(|pair| Some(pair.split('=').next()?.trim())).collect();
    assert_eq!(found, AUDIO_EXTENSIONS);
}

#[test]
fn info_plist_declares_every_extension_and_the_right_exe() {
    let plist = read("packaging/macos/Info.plist");
    for ext in AUDIO_EXTENSIONS {
        assert!(plist.contains(&format!("<string>{ext}</string>")), "{ext}");
    }
    assert!(plist.contains("<key>CFBundleExecutable</key>\n  <string>groove</string>"));
    assert!(plist.contains("<key>CFBundleTypeRole</key>"));
    let id = plist
        .split("<key>CFBundleIdentifier</key>\n  <string>")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .expect("CFBundleIdentifier");
    let rust = read("crates/groove-app/src/platform/default_player.rs");
    assert!(rust.contains(&format!("BUNDLE_ID: &str = \"{id}\";")), "{id}");
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
    assert!(desktop.lines().any(|l| l == "Categories=AudioVideo;Audio;Player;"));
    assert!(desktop.lines().any(|l| l == "Icon=groove"));
    // dev-app-id: bản phát hành ghép icon theo `app_id` = tên tệp; bản debug mã khác, mục dev của `start.sh` không đè gói.
    let window_state = read("crates/groove-app/src/window_state.rs");
    assert!(desktop.lines().any(|l| l == "StartupWMClass=groove"));
    assert!(window_state.contains("RELEASE_APP_ID: &str = \"groove\";"));
    assert!(window_state.contains("DEV_APP_ID: &str = \"groove.dev\";"));
    // pip: mục "Trình phát thu nhỏ" ở menu chuột phải của dock; `--pip` đi qua kênh một phiên (`instance.rs`).
    assert!(desktop.lines().any(|l| l == "Actions=pip;"));
    assert!(
        desktop.contains(
            "\n[Desktop Action pip]\nName=Picture in picture\nName[vi]=Trình phát thu nhỏ\nExec=groove --pip\n"
        )
    );
}

/// Icon hicolor đủ cỡ cho menu, "Mở bằng", thanh tác vụ.
#[test]
fn linux_icons_cover_the_usual_sizes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packaging/linux/icons");
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let path = dir.join(format!("{size}x{size}.png"));
        let bytes = fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{}", path.display());
        // IHDR: rộng, cao big-endian ở byte 16..24.
        let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        assert_eq!((width, height), (size, size), "{}", path.display());
    }
    assert!(read("packaging/linux/build-deb.sh").contains("packaging/linux/icons/*x*.png"));
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

/// file-icon: tệp nhạc đã liên kết dùng icon thứ hai của exe (`groove.exe,1`, `assets/app/groove-file.ico`) — cùng hình icon app,
/// nhỏ hơn, có lề; đủ khung cho ô 16 px (Details) tới 256 px (Extra large), mỗi khung PNG.
#[test]
fn audio_files_use_the_padded_file_icon() {
    let nsi = read("packaging/windows/groove.nsi");
    assert!(nsi.contains(r#"${PROG_PREFIX}${EXT}\DefaultIcon" "" "$INSTDIR\groove.exe,1""#));
    assert!(nsi.contains(r#"Applications\groove.exe\DefaultIcon" "" "$INSTDIR\groove.exe,0""#), "app vẫn icon app");
    assert!(read("packaging/windows/installer-e2e.ps1").contains(r#"-eq "$exe,1""#));
    let build = read("crates/groove-app/build.rs");
    assert!(
        build.contains(r#"1 ICON "{icon}""#) && build.contains(r#"2 ICON "{file_icon}""#),
        "thứ tự id = chỉ số icon"
    );

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/app/groove-file.ico");
    let ico = fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    assert_eq!(&ico[..4], &[0, 0, 1, 0], "ICONDIR kiểu icon");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let sizes: Vec<u32> = (0..count)
        .map(|i| {
            let entry = &ico[6 + i * 16..6 + (i + 1) * 16];
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            assert_eq!(&ico[offset..offset + 4], b"\x89PNG", "khung {i} là PNG");
            if entry[0] == 0 { 256 } else { u32::from(entry[0]) }
        })
        .collect();
    assert_eq!(sizes, [16, 20, 24, 32, 48, 64, 256]);
}

/// deb-metainfo: App Center lấy nhà phát hành / giấy phép từ AppStream; metainfo trỏ đúng tệp `.desktop`, icon, gói.
#[test]
fn appstream_metainfo_matches_the_desktop_entry() {
    let metainfo = read("packaging/linux/groove-x.metainfo.xml");
    for needle in [
        "<launchable type=\"desktop-id\">groove.desktop</launchable>",
        "<icon type=\"stock\">groove</icon>",
        "<pkgname>groove-x</pkgname>",
        "<binary>groove</binary>",
        "<project_license>LicenseRef-proprietary</project_license>",
        "<developer id=",
        "@VERSION@",
    ] {
        assert!(metainfo.contains(needle), "{needle}");
    }
    let build = read("packaging/linux/build-deb.sh");
    assert!(build.contains("packaging/linux/groove-x.metainfo.xml"));
    assert!(!build.contains("noreply@groove-x.invalid"), "Maintainer thật, không phải địa chỉ giả");
}
