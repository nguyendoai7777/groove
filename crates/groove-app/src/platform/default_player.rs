//! groove-gd2_01: trình phát mặc định (đặc tả 5.6). Hỏi app có đang mở tệp `.mp3` không, và đưa người dùng tới chỗ đặt.
//!
//! **Không bao giờ tự đổi khi người dùng chưa bấm.** Windows không cho app tự đặt mặc định: không ghi `UserChoice`, không mẹo
//! băm; nút chỉ mở trang Default apps, người dùng tự chọn. Linux ghi `xdg-mime default` cho tài khoản hiện tại, macOS đặt
//! handler qua LaunchServices, cả hai chỉ sau khi bấm.
//!
//! Mọi hàm ở đây chặn (registry, tiến trình con, LaunchServices): gọi từ luồng nền, UI không chạm đĩa (N7).
//!
//! Windows còn đặt AppUserModelID của tiến trình (`set_process_identity`), trùng với lối tắt bộ cài tạo, để taskbar gom cửa sổ
//! với lối tắt đã ghim và media session hiện đúng tên, icon.

/// Đuôi dùng để hỏi trạng thái: phổ biến nhất trong `AUDIO_EXTENSIONS`. Linux hỏi theo MIME của nó (`sys::PROBE_MIME`).
#[cfg(any(windows, target_os = "macos", test))]
pub const PROBE_EXTENSION: &str = "mp3";

/// App có đang là trình mở mặc định của `PROBE_EXTENSION` không.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Default,
    NotDefault,
    /// Bản chạy không qua bộ cài (bản dev, exe trần): OS không biết app, không đặt được.
    NotInstalled,
    /// Không hỏi được OS (thiếu `xdg-mime`…).
    Unknown,
}

/// Bấm nút xong thì đã làm gì.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Đã mở trang của hệ thống, người dùng tự chọn ở đó (Windows).
    #[cfg_attr(
        all(not(windows), not(test)),
        expect(dead_code, reason = "Linux, macOS đặt luôn, test view dựng giá trị này")
    )]
    OpenedSystemSettings,
    /// Đã đặt cho tài khoản này (Linux, macOS).
    #[cfg_attr(
        all(windows, not(test)),
        expect(dead_code, reason = "Windows chỉ mở trang hệ thống, test view dựng giá trị này")
    )]
    Applied,
}

/// Nút sẽ mở trang hệ thống (Windows) hay đặt luôn (Linux, macOS): để chữ trên nút nói đúng việc.
pub const OPENS_SYSTEM_SETTINGS: bool = cfg!(windows);

/// Hỏi OS. Chặn.
pub fn status() -> Status {
    sys::status()
}

/// Người dùng vừa bấm "Đặt làm mặc định". Chặn.
pub fn make_default() -> Result<Outcome, String> {
    sys::make_default()
}

/// Gọi một lần, trước khi mở cửa sổ đầu tiên.
pub fn set_process_identity() {
    #[cfg(windows)]
    sys::set_process_identity();
}

/// Giá trị của dòng `MimeType=` trong tệp `.desktop`, bỏ mục rỗng.
#[cfg(any(target_os = "linux", test))]
pub fn desktop_mime_types(desktop: &str) -> Vec<&str> {
    desktop
        .lines()
        .find_map(|line| line.trim().strip_prefix("MimeType="))
        .map(|list| list.split(';').map(str::trim).filter(|mime| !mime.is_empty()).collect())
        .unwrap_or_default()
}

#[cfg(windows)]
mod sys {
    use std::path::PathBuf;

    use groove_core::paths::normalize_path;
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows_sys::Win32::UI::Shell::{
        ASSOCF_INIT_IGNOREUNKNOWN, ASSOCF_NOTRUNCATE, ASSOCSTR_EXECUTABLE, AssocQueryStringW,
        SetCurrentProcessExplicitAppUserModelID, ShellExecuteW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    use super::{Outcome, PROBE_EXTENSION, Status};

    /// Trùng `AUMID` trong `packaging/windows/groove.nsi` (lối tắt Start Menu, Desktop): `tests/packaging.rs` soát.
    pub const APP_USER_MODEL_ID: &str = "GrooveX.Player";
    /// Tên giá trị trong `HKCU\Software\RegisteredApplications`, bộ cài ghi.
    const REGISTERED_NAME: &str = "Groove X";
    /// Windows 11 mở thẳng trang của app; Windows 10 bỏ qua tham số và mở trang Default apps chung (ô Trình phát nhạc).
    pub const DEFAULT_APPS_URI: &str = "ms-settings:defaultapps?registeredAppUser=Groove%20X";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub fn set_process_identity() {
        let id = wide(APP_USER_MODEL_ID);
        // SAFETY: chuỗi kết thúc NUL, sống tới hết lời gọi; hàm chép giá trị.
        let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(id.as_ptr()) };
        if hr < 0 {
            log::warn!("default player: cannot set the AppUserModelID: {hr:#x}");
        }
    }

    /// Bộ cài đã đăng ký app với Default apps.
    fn registered() -> bool {
        let key = wide(r"Software\RegisteredApplications");
        let name = wide(REGISTERED_NAME);
        // SAFETY: chuỗi kết thúc NUL; không xin dữ liệu, chỉ hỏi có không.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        status == 0
    }

    /// Exe đang mở `.<ext>` theo Explorer (đã tính `UserChoice`), không có thì `None`.
    fn default_executable(ext: &str) -> Option<PathBuf> {
        let assoc = wide(&format!(".{ext}"));
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` đủ `len` ký tự; hàm ghi kèm NUL và trả độ dài đã ghi vào `len`.
        let hr = unsafe {
            AssocQueryStringW(
                ASSOCF_INIT_IGNOREUNKNOWN | ASSOCF_NOTRUNCATE,
                ASSOCSTR_EXECUTABLE,
                assoc.as_ptr(),
                std::ptr::null(),
                buf.as_mut_ptr(),
                &mut len,
            )
        };
        if hr != 0 {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..end])))
    }

    pub fn status() -> Status {
        if !registered() {
            return Status::NotInstalled;
        }
        let Ok(me) = std::env::current_exe() else { return Status::Unknown };
        match default_executable(PROBE_EXTENSION) {
            Some(exe) if normalize_path(&exe) == normalize_path(&me) => Status::Default,
            _ => Status::NotDefault,
        }
    }

    pub fn make_default() -> Result<Outcome, String> {
        let (verb, uri) = (wide("open"), wide(DEFAULT_APPS_URI));
        // SAFETY: chuỗi kết thúc NUL; không cửa sổ cha.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                uri.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute trả số > 32 khi thành công.
        if result as isize > 32 {
            Ok(Outcome::OpenedSystemSettings)
        } else {
            Err(format!("ShellExecuteW {DEFAULT_APPS_URI}: {}", result as isize))
        }
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::path::PathBuf;
    use std::process::Command;

    use super::{Outcome, Status, desktop_mime_types};

    /// Tên tệp `.desktop` gói `.deb` cài (`packaging/linux/groove.desktop`).
    const DESKTOP_ID: &str = "groove.desktop";
    /// MIME của `PROBE_EXTENSION`.
    const PROBE_MIME: &str = "audio/mpeg";

    /// Tệp `.desktop` đã cài, tìm theo XDG Base Directory: tài khoản trước, hệ thống sau.
    fn desktop_file() -> Option<PathBuf> {
        let home = std::env::var_os("XDG_DATA_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".local/share")));
        let system = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|dirs| !dirs.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
        home.into_iter()
            .chain(system.split(':').filter(|dir| !dir.is_empty()).map(PathBuf::from))
            .map(|dir| dir.join("applications").join(DESKTOP_ID))
            .find(|path| path.is_file())
    }

    pub fn status() -> Status {
        if desktop_file().is_none() {
            return Status::NotInstalled;
        }
        match Command::new("xdg-mime").args(["query", "default", PROBE_MIME]).output() {
            Ok(out) if out.status.success() => {
                if String::from_utf8_lossy(&out.stdout).trim() == DESKTOP_ID {
                    Status::Default
                } else {
                    Status::NotDefault
                }
            }
            Ok(out) => {
                log::warn!("default player: xdg-mime query failed: {}", out.status);
                Status::Unknown
            }
            Err(err) => {
                log::warn!("default player: cannot run xdg-mime: {err}");
                Status::Unknown
            }
        }
    }

    /// Đặt cho đúng các MIME tệp `.desktop` đã cài khai, để không lệch gói.
    pub fn make_default() -> Result<Outcome, String> {
        let path = desktop_file().ok_or_else(|| format!("{DESKTOP_ID} is not installed"))?;
        let text = std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        let mimes = desktop_mime_types(&text);
        if mimes.is_empty() {
            return Err(format!("{}: no MimeType", path.display()));
        }
        let status = Command::new("xdg-mime")
            .arg("default")
            .arg(DESKTOP_ID)
            .args(&mimes)
            .status()
            .map_err(|err| format!("xdg-mime: {err}"))?;
        if status.success() { Ok(Outcome::Applied) } else { Err(format!("xdg-mime default: {status}")) }
    }
}

#[cfg(target_os = "macos")]
mod sys {
    use std::ffi::c_void;

    use groove_core::model::AUDIO_EXTENSIONS;
    use objc2::rc::Retained;
    use objc2_foundation::{NSBundle, NSString};

    use super::{Outcome, PROBE_EXTENSION, Status};

    /// Trùng `CFBundleIdentifier` của `packaging/macos/Info.plist`.
    const BUNDLE_ID: &str = "com.mrdxd.groovex";
    /// `kLSRolesAll`.
    const ROLES_ALL: u32 = 0xFFFF_FFFF;

    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn UTTypeCreatePreferredIdentifierForTag(
            tag_class: *const c_void,
            tag: *const c_void,
            conforming_to: *const c_void,
        ) -> *mut c_void;
        fn LSCopyDefaultRoleHandlerForContentType(content_type: *const c_void, role: u32) -> *mut c_void;
        fn LSSetDefaultRoleHandlerForContentType(content_type: *const c_void, role: u32, handler: *const c_void)
        -> i32;
    }

    /// `NSString` và `CFStringRef` cầu nối miễn phí (toll-free bridged).
    fn cf(text: &NSString) -> *const c_void {
        (text as *const NSString).cast()
    }

    /// Hàm CF `Create`, `Copy` trả +1: `Retained` nhận quyền nhả.
    ///
    /// # Safety
    /// `ptr` là null hoặc một `CFStringRef` +1.
    unsafe fn owned(ptr: *mut c_void) -> Option<Retained<NSString>> {
        // SAFETY: theo hợp đồng của hàm.
        unsafe { Retained::from_raw(ptr.cast::<NSString>()) }
    }

    fn uti(ext: &str) -> Option<Retained<NSString>> {
        let class = NSString::from_str("public.filename-extension");
        let tag = NSString::from_str(ext);
        // SAFETY: hai chuỗi sống tới hết lời gọi; kết quả là bản `Create`.
        unsafe { owned(UTTypeCreatePreferredIdentifierForTag(cf(&class), cf(&tag), std::ptr::null())) }
    }

    /// Bundle của app khi chạy từ `Groove X.app`; exe trần không có.
    fn own_bundle_id() -> Option<Retained<NSString>> {
        NSBundle::mainBundle().bundleIdentifier().filter(|id| id.to_string() == BUNDLE_ID)
    }

    pub fn status() -> Status {
        let Some(me) = own_bundle_id() else { return Status::NotInstalled };
        let Some(uti) = uti(PROBE_EXTENSION) else { return Status::Unknown };
        // SAFETY: `uti` sống tới hết lời gọi; kết quả là bản `Copy`.
        match unsafe { owned(LSCopyDefaultRoleHandlerForContentType(cf(&uti), ROLES_ALL)) } {
            Some(handler) if handler.to_string().eq_ignore_ascii_case(&me.to_string()) => Status::Default,
            _ => Status::NotDefault,
        }
    }

    pub fn make_default() -> Result<Outcome, String> {
        let me = own_bundle_id().ok_or_else(|| "not running from Groove X.app".to_owned())?;
        for ext in AUDIO_EXTENSIONS {
            let uti = uti(ext).ok_or_else(|| format!("no type for .{ext}"))?;
            // SAFETY: hai chuỗi sống tới hết lời gọi.
            let status = unsafe { LSSetDefaultRoleHandlerForContentType(cf(&uti), ROLES_ALL, cf(&me)) };
            if status != 0 {
                return Err(format!(".{ext}: OSStatus {status}"));
            }
        }
        Ok(Outcome::Applied)
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod sys {
    use super::{Outcome, Status};

    pub fn status() -> Status {
        Status::Unknown
    }

    pub fn make_default() -> Result<Outcome, String> {
        Err("unsupported platform".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_list_comes_from_the_desktop_entry() {
        let entry = "[Desktop Entry]\nName=Groove X\nMimeType=audio/mpeg;audio/flac; ;\nExec=groove %F\n";
        assert_eq!(desktop_mime_types(entry), ["audio/mpeg", "audio/flac"]);
        assert!(desktop_mime_types("[Desktop Entry]\nName=x\n").is_empty());
    }

    #[test]
    fn packaged_desktop_entry_declares_the_probe_type() {
        let entry = include_str!("../../../../packaging/linux/groove.desktop");
        assert!(desktop_mime_types(entry).contains(&"audio/mpeg"));
        assert!(groove_core::model::AUDIO_EXTENSIONS.contains(&PROBE_EXTENSION));
    }

    /// Không giả định máy chạy test đã cài hay chưa: chỉ không được treo, không được panic.
    #[test]
    fn status_answers_without_installing_anything() {
        let status = status();
        assert!(matches!(status, Status::Default | Status::NotDefault | Status::NotInstalled | Status::Unknown));
    }

    #[cfg(windows)]
    #[test]
    fn windows_uri_names_the_registered_app() {
        assert!(sys::DEFAULT_APPS_URI.ends_with("registeredAppUser=Groove%20X"));
        assert_eq!(sys::APP_USER_MODEL_ID, "GrooveX.Player");
    }
}
