//! groove-gd1_25: một phiên (đặc tả 5.5). `main` gọi [`forward_to_primary`] trước khi dựng GPUI.
//!
//! Phiên sau gửi một dòng JSON `{"paths": [...], "cwd": "..."}` qua local socket, chờ phiên chính báo đã nhận rồi thoát. Không
//! chờ thì thông điệp có thể mất: client đóng pipe trước khi server gọi `ConnectNamedPipe` là dữ liệu bị bỏ (tái hiện khi máy
//! bận). Phiên chính giữ socket từ trước khi có cửa sổ; thông điệp xếp hàng trong kênh, `open_paths` nhận khi player đã sẵn sàng.
//!
//! Chỉ người dùng hiện tại mở được: Windows là pipe instance đầu + DACL một SID, phiên sau còn kiểm exe của server cùng
//! thư mục; Unix là socket 0600 trong thư mục 0700. Dữ liệu nhận vẫn không được tin: có trần cỡ dòng, chỉ tệp qua
//! `open_paths::resolve` mới tới player. Log không ghi đường dẫn.

use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::{Duration, Instant};

use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, ListenerOptions, Name, Stream};
use serde::{Deserialize, Serialize};

use crate::open_paths::{self, OpenRequest};

/// Trần một dòng. Đủ cho vài chục nghìn đường dẫn; dòng dài hơn là client hỏng hoặc độc, không được làm phình RAM.
const MAX_LINE: u64 = 1 << 20;
/// Không gửi được cũng không giành được endpoint (phiên chính đang dựng, pipe đang chuyển instance) thì thử lại trong khoảng này.
const CLAIM_PATIENCE: Duration = Duration::from_secs(5);
const CLAIM_RETRY: Duration = Duration::from_millis(50);
/// Phiên chính treo thì không có báo nhận; phiên sau vẫn phải thoát. Dài vì máy bận cũng không được làm mất tệp.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);
/// Phiên chính báo đã nhận xong một dòng hợp lệ.
const ACK: &[u8; 3] = b"ok
";

/// Một dòng trên socket. Khoá camelCase theo luật JSON; thiếu trường thì coi như rỗng.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardMessage {
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub cwd: PathBuf,
}

impl ForwardMessage {
    /// Dòng để gửi, kèm `\n`. Quá trần thì bỏ các tệp cuối: phát được phần lớn còn hơn không phát gì. Tên không phải Unicode
    /// không đi được qua JSON nên bị bỏ, phiên đầu vẫn mở được chúng từ `argv`.
    fn encode(&self) -> io::Result<Vec<u8>> {
        let mut kept = ForwardMessage { paths: Vec::with_capacity(self.paths.len()), cwd: self.cwd.clone() };
        let mut size = serde_json::to_vec(&kept).map_err(io::Error::other)?.len() as u64 + 1;
        for path in &self.paths {
            let Ok(json) = serde_json::to_vec(path) else { continue };
            size += json.len() as u64 + 1;
            if size > MAX_LINE {
                break;
            }
            kept.paths.push(path.clone());
        }
        if kept.paths.len() < self.paths.len() {
            log::warn!("instance: {} of {} paths not forwarded", self.paths.len() - kept.paths.len(), self.paths.len());
        }
        let mut line = serde_json::to_vec(&kept).map_err(io::Error::other)?;
        line.push(b'\n');
        Ok(line)
    }
}

/// Đọc đúng một dòng, không quá [`MAX_LINE`]. Hỏng thì `InvalidData`: bên nhận bỏ, không focus, không phát.
fn read_message(reader: impl Read) -> io::Result<ForwardMessage> {
    let mut line = String::new();
    let n = BufReader::new(reader.take(MAX_LINE + 1)).read_line(&mut line)?;
    if n as u64 > MAX_LINE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
    }
    if n == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    serde_json::from_str(line.trim_end()).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

/// Địa chỉ của phiên chính.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Endpoint {
    /// Windows: tên pipe. Unix: đường dẫn socket.
    id: String,
    /// Endpoint thật trên Windows: phiên sau kiểm exe của server trước khi gửi đường dẫn.
    check_server: bool,
}

/// Bản debug không đụng phiên thật; có `GROOVE_DATA_DIR` thì mỗi thư mục dữ liệu một phiên, như mọi thứ khác của nó.
fn dev_tag(debug: bool, data_dir: Option<&Path>) -> String {
    if !debug {
        return String::new();
    }
    match data_dir {
        // FNV-1a: chỉ cần ổn định và không chứa đường dẫn.
        Some(dir) => {
            let hash = groove_core::paths::normalize_path(dir)
                .bytes()
                .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
            format!("-dev-{hash:016x}")
        }
        None => "-dev".to_owned(),
    }
}

impl Endpoint {
    /// Endpoint thật cho người dùng hiện tại. Windows theo SID: pipe là toàn máy, tên người dùng thì có thể trùng sau khi lọc.
    fn primary() -> io::Result<Endpoint> {
        // Như `groove_core::paths`: biến này chỉ có nghĩa ở bản debug, bản phát hành không đọc (M1).
        let data_dir =
            if cfg!(debug_assertions) { std::env::var_os("GROOVE_DATA_DIR").map(PathBuf::from) } else { None };
        let tag = dev_tag(cfg!(debug_assertions), data_dir.as_deref());
        #[cfg(windows)]
        {
            Ok(Endpoint { id: format!("groove-x-{}{tag}", sys::current_user_sid()?), check_server: true })
        }
        #[cfg(unix)]
        {
            let dir = match std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
                Some(dir) => PathBuf::from(dir).join(format!("groove-x{tag}")),
                None => groove_core::paths::data_dir()?.join(format!("run{tag}")),
            };
            Ok(Endpoint { id: dir.join("primary.sock").to_string_lossy().into_owned(), check_server: true })
        }
    }

    /// Tên riêng cho test, không đụng phiên thật.
    #[cfg(test)]
    fn for_test(tag: &str) -> Endpoint {
        let unique = format!("groove-x-test-{}-{tag}", std::process::id());
        #[cfg(windows)]
        {
            Endpoint { id: unique, check_server: false }
        }
        #[cfg(unix)]
        {
            let path = std::env::temp_dir().join(unique).join("primary.sock");
            Endpoint { id: path.to_string_lossy().into_owned(), check_server: false }
        }
    }

    fn name(&self) -> io::Result<Name<'static>> {
        #[cfg(windows)]
        {
            use interprocess::local_socket::GenericNamespaced;
            self.id.clone().to_ns_name::<GenericNamespaced>()
        }
        #[cfg(unix)]
        {
            use interprocess::local_socket::GenericFilePath;
            PathBuf::from(&self.id).to_fs_name::<GenericFilePath>()
        }
    }

    /// Giành vai phiên chính. Đã có chủ thì `AddrInUse`, không chạy mù. Unix: khoá trước, socket cũ sau crash chỉ được xoá
    /// khi đã giữ khoá; crash thì OS tự nhả khoá.
    fn claim(&self) -> io::Result<Claimed> {
        #[cfg(unix)]
        let lock = sys::lock_and_clear(&self.id)?;
        let options = ListenerOptions::new().name(self.name()?);
        #[cfg(windows)]
        let options = {
            use interprocess::os::windows::local_socket::ListenerOptionsExt;
            options.security_descriptor(sys::owner_only_descriptor()?)
        };
        let listener = options.create_sync().map_err(sys::listen_error)?;
        #[cfg(unix)]
        sys::restrict_socket(&self.id)?;
        Ok(Claimed {
            listener,
            #[cfg(unix)]
            _lock: lock,
        })
    }

    /// Nối, xin phép cho phiên chính lấy foreground (S22), gửi, chờ bên kia nhận. Có hạn, xem [`SEND_TIMEOUT`].
    fn send(&self, line: Vec<u8>) -> io::Result<()> {
        let endpoint = self.clone();
        let (done, result) = std_mpsc::channel();
        thread::Builder::new().name("instance-send".into()).spawn(move || {
            let _ = done.send(endpoint.send_blocking(&line));
        })?;
        result.recv_timeout(SEND_TIMEOUT).unwrap_or_else(|_| Err(io::ErrorKind::TimedOut.into()))
    }

    fn send_blocking(&self, line: &[u8]) -> io::Result<()> {
        let mut stream = Stream::connect(self.name()?)?;
        #[cfg(windows)]
        sys::prepare_server(&stream, self.check_server)?;
        stream.write_all(line)?;
        stream.flush()?;
        let mut ack = [0; ACK.len()];
        stream.read_exact(&mut ack)?;
        if ack == *ACK { Ok(()) } else { Err(io::ErrorKind::InvalidData.into()) }
    }
}

/// Đã giành được endpoint. Phải giữ sống hết đời tiến trình.
struct Claimed {
    listener: Listener,
    #[cfg(unix)]
    _lock: std::fs::File,
}

enum Outcome {
    /// Có phiên chính, đã chuyển: tiến trình này thoát.
    Forwarded,
    /// Tiến trình này là phiên chính.
    Primary(Claimed),
    /// Không nói được với ai, không giành được tên (tiến trình lạ giữ tên, lỗi OS). App vẫn phải mở được.
    Standalone,
}

/// Vòng thử gửi / giành. Nhiều tiến trình mở cùng lúc (Explorer bật một tiến trình cho mỗi tệp) thì đúng một bên thắng.
fn forward_or_claim(endpoint: &Endpoint, message: &ForwardMessage, patience: Duration) -> Outcome {
    let Ok(line) = message.encode() else {
        return Outcome::Standalone;
    };
    let deadline = Instant::now() + patience;
    loop {
        match endpoint.send(line.clone()) {
            Ok(()) => return Outcome::Forwarded,
            Err(err) if err.kind() == io::ErrorKind::TimedOut => {
                // Có phiên chính nhưng treo: mở phiên thứ hai sẽ giành DB với nó, thoát vẫn đỡ hơn.
                log::warn!("instance: primary did not answer");
                return Outcome::Forwarded;
            }
            Err(err) => log::debug!("instance: forward failed: {err}"),
        }
        match endpoint.claim() {
            Ok(claimed) => return Outcome::Primary(claimed),
            Err(err) if err.kind() == io::ErrorKind::AddrInUse && Instant::now() < deadline => {
                thread::sleep(CLAIM_RETRY);
            }
            Err(err) => {
                log::warn!("instance: running without single instance: {err}");
                return Outcome::Standalone;
            }
        }
    }
}

/// Luồng nhận kết nối, mỗi kết nối một luồng ngắn: client treo không chặn client sau.
fn serve(claimed: Claimed, sink: UnboundedSender<OpenRequest>) -> io::Result<()> {
    thread::Builder::new().name("instance-accept".into()).spawn(move || {
        // Giữ nguyên `Claimed` trong luồng: closure chỉ bắt `claimed.listener` thì khoá Unix bị nhả ngay khi `serve` trả
        // về, phiên sau giành được khoá, xoá socket của phiên đang chạy rồi thành phiên chính thứ hai.
        let claimed = claimed;
        for connection in claimed.listener.incoming() {
            match connection {
                Ok(stream) => {
                    let sink = sink.clone();
                    let spawned = thread::Builder::new().name("instance-conn".into()).spawn(move || {
                        receive(stream, &sink);
                    });
                    if let Err(err) = spawned {
                        log::warn!("instance: cannot spawn connection thread: {err}");
                    }
                }
                Err(err) => {
                    log::warn!("instance: accept failed: {err}");
                    thread::sleep(CLAIM_RETRY);
                }
            }
        }
    })?;
    Ok(())
}

fn receive(stream: Stream, sink: &UnboundedSender<OpenRequest>) {
    match read_message(&stream) {
        Ok(message) => {
            let paths = open_paths::resolve(message.paths, &message.cwd);
            let _ = sink.unbounded_send(OpenRequest { paths, activate: true });
            // Phiên sau đã thoát vì hết hạn thì ghi lỗi, không sao: yêu cầu đã vào kênh.
            let mut writer = &stream;
            let _ = writer.write_all(ACK).and_then(|()| writer.flush());
        }
        Err(err) => log::warn!("instance: bad message ignored: {err}"),
    }
}

type Incoming = (UnboundedSender<OpenRequest>, UnboundedReceiver<OpenRequest>);

/// Kênh của phiên chính, từ lúc giành được endpoint tới lúc `open_paths` nhận.
static INCOMING: Mutex<Option<Incoming>> = Mutex::new(None);

/// Đã có phiên chính thì chuyển `argv` + `cwd` cho nó và trả `true`, tiến trình này thoát luôn.
pub fn forward_to_primary() -> bool {
    let endpoint = match Endpoint::primary() {
        Ok(endpoint) => endpoint,
        Err(err) => {
            log::warn!("instance: no endpoint: {err}");
            return false;
        }
    };
    let args: Vec<OsString> = std::env::args_os().collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    let message = ForwardMessage { paths: open_paths::positional_args(&args), cwd };
    match forward_or_claim(&endpoint, &message, CLAIM_PATIENCE) {
        Outcome::Forwarded => true,
        Outcome::Standalone => false,
        Outcome::Primary(claimed) => {
            let (sender, receiver) = mpsc::unbounded();
            match serve(claimed, sender.clone()) {
                Ok(()) => *INCOMING.lock().unwrap_or_else(|err| err.into_inner()) = Some((sender, receiver)),
                Err(err) => log::warn!("instance: cannot serve: {err}"),
            }
            false
        }
    }
}

/// Kênh mà luồng nhận đang đẩy vào. `None` nếu chạy không có một phiên.
pub fn take_incoming() -> Option<Incoming> {
    INCOMING.lock().unwrap_or_else(|err| err.into_inner()).take()
}

#[cfg(windows)]
mod sys {
    use std::ffi::OsString;
    use std::io;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::Path;
    use std::ptr;

    use interprocess::local_socket::Stream;
    use interprocess::local_socket::prelude::*;
    use interprocess::os::windows::security_descriptor::{
        AsSecurityDescriptorExt, BorrowedSecurityDescriptor, SecurityDescriptor,
    };
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

    /// `CreateNamedPipeW` với `FILE_FLAG_FIRST_PIPE_INSTANCE` khi tên đã có chủ.
    pub fn listen_error(err: io::Error) -> io::Error {
        match err.raw_os_error().map(|code| code as u32) {
            Some(ERROR_ACCESS_DENIED | ERROR_PIPE_BUSY) => {
                io::Error::new(io::ErrorKind::AddrInUse, "endpoint is held by another process")
            }
            _ => err,
        }
    }

    pub fn current_user_sid() -> io::Result<String> {
        let mut token = ptr::null_mut();
        // SAFETY: tham số ra hợp lệ; `OwnedHandle` đóng handle token.
        let token = unsafe {
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedHandle::from_raw_handle(token)
        };
        let mut size = 0;
        // SAFETY: lần đầu chỉ hỏi cỡ (đệm rỗng); lần sau đệm `u64` đủ cỡ, căn hơn `TOKEN_USER`.
        let mut buf = unsafe {
            GetTokenInformation(token.as_raw_handle(), TokenUser, ptr::null_mut(), 0, &mut size);
            vec![0u64; (size as usize).div_ceil(8)]
        };
        // SAFETY: như trên.
        let ok =
            unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, buf.as_mut_ptr().cast(), size, &mut size) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut text = ptr::null_mut();
        // SAFETY: `buf` là `TOKEN_USER` OS vừa ghi; `text` cấp bằng `LocalAlloc`, kết thúc bằng NUL.
        unsafe {
            let sid = (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid;
            if ConvertSidToStringSidW(sid, &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let len = (0..).take_while(|&i| *text.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Ok(sid)
        }
    }

    /// DACL có bảo vệ (không thừa kế), một ACE: toàn quyền cho người dùng hiện tại.
    pub fn owner_only_sddl() -> io::Result<String> {
        Ok(format!("D:P(A;;GA;;;{})", current_user_sid()?))
    }

    pub fn owner_only_descriptor() -> io::Result<SecurityDescriptor> {
        let wide: Vec<u16> = std::ffi::OsStr::new(&owner_only_sddl()?).encode_wide().chain([0]).collect();
        let mut raw = ptr::null_mut();
        // SAFETY: `wide` kết thúc bằng NUL; `raw` cấp bằng `LocalAlloc`, chép xong thì trả.
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut raw,
                ptr::null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let owned = BorrowedSecurityDescriptor::from_ptr(raw).to_owned_sd();
            LocalFree(raw);
            owned
        }
    }

    /// Exe của tiến trình `pid`. Không đọc được ⇒ lỗi: bên gọi từ chối, không bỏ qua.
    fn process_image(pid: u32) -> io::Result<OsString> {
        // SAFETY: handle hợp lệ khi khác null, `OwnedHandle` đóng; đệm + độ dài đúng cặp.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let handle = OwnedHandle::from_raw_handle(handle);
            let mut buf = vec![0u16; 32 * 1024];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(handle.as_raw_handle(), PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(OsString::from_wide(&buf[..len as usize]))
        }
    }

    /// Cùng thư mục sau khi giải đường dẫn thật. Giải không được ⇒ `false`.
    fn same_directory(a: &Path, b: &Path) -> bool {
        match (a.parent().map(Path::canonicalize), b.parent().map(Path::canonicalize)) {
            (Some(Ok(a)), Some(Ok(b))) => a == b,
            _ => false,
        }
    }

    /// Trước khi gửi: tiến trình lạ giành tên pipe không được đọc đường dẫn của người dùng; server đúng thì cho nó lấy
    /// foreground (quyền này phiên sau đang có vì vừa được người dùng mở).
    pub fn prepare_server(stream: &Stream, check: bool) -> io::Result<()> {
        let denied = |message: &str| io::Error::new(io::ErrorKind::PermissionDenied, message.to_owned());
        let pid = stream.peer_creds()?.pid().ok_or_else(|| denied("server has no pid"))?;
        if check {
            let server = process_image(pid).map_err(|_| denied("cannot read server exe"))?;
            if !same_directory(&std::env::current_exe()?, Path::new(&server)) {
                return Err(denied("server runs from another directory"));
            }
        }
        // SAFETY: chỉ truyền số. Thất bại thì cửa sổ nhấp nháy taskbar thay vì lên trước, không phải lỗi.
        unsafe { AllowSetForegroundWindow(pid) };
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn same_directory_ignores_case_and_file_name() {
            let dir = tempfile::tempdir().unwrap();
            let sub = dir.path().join("Sub");
            std::fs::create_dir(&sub).unwrap();
            assert!(same_directory(&sub.join("x.exe"), &dir.path().join("sub").join("y.exe")));
            assert!(!same_directory(&sub.join("x.exe"), &dir.path().join("x.exe")));
        }

        #[test]
        fn descriptor_grants_only_the_current_user() {
            let sddl = owner_only_sddl().unwrap();
            assert!(sddl.starts_with("D:P(A;;GA;;;S-1-"), "{sddl}");
            assert_eq!(sddl.matches('(').count(), 1, "one ACE: {sddl}");
            owner_only_descriptor().unwrap();
        }
    }
}

#[cfg(unix)]
mod sys {
    use std::fs::{self, File, TryLockError};
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::Path;

    /// Thư mục 0700 thuộc người dùng này, khoá độc quyền, rồi mới dọn socket chết của lần crash trước (đặc tả 5.5). Khoá nhả
    /// khi tiến trình chết, kể cả crash.
    pub fn lock_and_clear(socket: &str) -> io::Result<File> {
        let socket = Path::new(socket);
        let Some(dir) = socket.parent() else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "socket path has no parent"));
        };
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        let lock = File::options().create(true).truncate(false).write(true).open(dir.join("primary.lock"))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(io::ErrorKind::AddrInUse, "another primary holds the lock"));
            }
            Err(TryLockError::Error(err)) => return Err(err),
        }
        match fs::remove_file(socket) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(lock),
        }
    }

    /// `ListenerOptionsExt::mode` của `interprocess` không có trên macOS ⇒ chmod sau `bind`.
    pub fn restrict_socket(socket: &str) -> io::Result<()> {
        fs::set_permissions(socket, fs::Permissions::from_mode(0o600))
    }

    pub fn listen_error(err: io::Error) -> io::Error {
        err
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::process::Command;
    use std::sync::Arc;

    use groove_core::model::is_audio_path;

    use super::*;

    const CHILD_TAG: &str = "GROOVE_TEST_INSTANCE_TAG";

    fn serve_channel(endpoint: &Endpoint) -> UnboundedReceiver<OpenRequest> {
        let Ok(claimed) = endpoint.claim() else { panic!("claim") };
        let (sender, receiver) = mpsc::unbounded();
        serve(claimed, sender).unwrap();
        receiver
    }

    fn next_request(receiver: &mut UnboundedReceiver<OpenRequest>) -> OpenRequest {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(request) = receiver.try_recv() {
                return request;
            }
            assert!(Instant::now() < deadline, "no request");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn absolute(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn send_raw(endpoint: &Endpoint, bytes: &[u8]) {
        let mut stream = Stream::connect(endpoint.name().unwrap()).unwrap();
        let _ = stream.write_all(bytes);
        let _ = stream.flush();
    }

    #[test]
    fn wire_format_matches_the_spec() {
        let message = ForwardMessage { paths: vec!["a.mp3".into()], cwd: "dir".into() };
        let line = message.encode().unwrap();
        assert_eq!(line, b"{\"paths\":[\"a.mp3\"],\"cwd\":\"dir\"}\n");
        assert_eq!(read_message(Cursor::new(line)).unwrap(), message);
        assert_eq!(read_message(Cursor::new(b"{}\n".to_vec())).unwrap(), ForwardMessage::default());
    }

    #[test]
    fn oversized_and_junk_lines_are_rejected() {
        let long = vec![b'x'; MAX_LINE as usize + 10];
        assert_eq!(read_message(Cursor::new(long)).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(read_message(Cursor::new(b"not json\n".to_vec())).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(read_message(Cursor::new(Vec::new())).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    /// Quá nhiều tệp vẫn ra một dòng hợp lệ dưới trần, giữ các tệp đầu.
    #[test]
    fn encode_stays_under_the_limit() {
        let paths: Vec<PathBuf> = (0..100_000).map(|i| PathBuf::from(format!("C:/Nhạc/bài số {i}.mp3"))).collect();
        let line = ForwardMessage { paths: paths.clone(), cwd: "C:/".into() }.encode().unwrap();
        assert!(line.len() as u64 <= MAX_LINE);
        let decoded = read_message(Cursor::new(line)).unwrap();
        assert!(decoded.paths.len() > 1_000);
        assert_eq!(decoded.paths[..3], paths[..3]);
    }

    #[test]
    fn dev_builds_never_share_the_real_endpoint() {
        assert_eq!(dev_tag(false, Some(Path::new("/tmp/a"))), "");
        assert_eq!(dev_tag(true, None), "-dev");
        let (a, b) = (dev_tag(true, Some(Path::new("/tmp/a"))), dev_tag(true, Some(Path::new("/tmp/b"))));
        assert_ne!(a, b);
        assert!(!a.contains("tmp"), "no path in the name");
    }

    /// Một bên giữ tên thì bên sau `AddrInUse`, không chạy mù.
    #[test]
    fn second_claim_is_refused() {
        let endpoint = Endpoint::for_test("claim");
        let Ok(first) = endpoint.claim() else { panic!("first claim") };
        let Err(second) = endpoint.claim() else { panic!("second claim") };
        assert_eq!(second.kind(), io::ErrorKind::AddrInUse);
        drop(first);
    }

    /// Không tin dữ liệu nhận: rác và tệp sai đuôi không tới player, kết nối sau vẫn được phục vụ; `BÀI.MP3` qua (S22).
    #[test]
    fn server_filters_input_and_survives_junk() {
        let endpoint = Endpoint::for_test("filter");
        let mut receiver = serve_channel(&endpoint);
        send_raw(&endpoint, b"garbage\n");
        send_raw(&endpoint, &vec![b'x'; MAX_LINE as usize + 10]);
        let cwd = absolute("groove-cwd");
        let paths = vec![PathBuf::from("BÀI.MP3"), absolute("virus.exe"), absolute("noext")];
        let line = ForwardMessage { paths, cwd: cwd.clone() }.encode().unwrap();
        send_raw(&endpoint, &line);
        let request = next_request(&mut receiver);
        assert!(request.activate);
        assert_eq!(request.paths, [cwd.join("BÀI.MP3")]);
        assert!(request.paths.iter().all(|path| is_audio_path(path)));
    }

    /// Mở không có tệp: vẫn đánh thức phiên chính để focus.
    #[test]
    fn empty_open_only_activates() {
        let endpoint = Endpoint::for_test("empty");
        let mut receiver = serve_channel(&endpoint);
        let message = ForwardMessage { paths: Vec::new(), cwd: absolute("x") };
        assert!(matches!(forward_or_claim(&endpoint, &message, CLAIM_PATIENCE), Outcome::Forwarded));
        let request = next_request(&mut receiver);
        assert!(request.activate && request.paths.is_empty());
    }

    /// Explorer bật một tiến trình cho mỗi tệp: nhiều bên đua thì đúng một phiên chính, không mất tệp nào của bên còn lại.
    #[test]
    fn racing_processes_elect_one_primary_and_lose_nothing() {
        const RACERS: usize = 8;
        let endpoint = Arc::new(Endpoint::for_test("race"));
        let start = Arc::new(std::sync::Barrier::new(RACERS));
        let (sender, mut receiver) = mpsc::unbounded();
        let racers: Vec<_> = (0..RACERS)
            .map(|i| {
                let (endpoint, start, sender) = (endpoint.clone(), start.clone(), sender.clone());
                thread::spawn(move || {
                    let message = ForwardMessage { paths: vec![absolute(&format!("{i}.mp3"))], cwd: absolute("x") };
                    start.wait();
                    match forward_or_claim(&endpoint, &message, Duration::from_secs(10)) {
                        Outcome::Forwarded => false,
                        Outcome::Primary(claimed) => {
                            serve(claimed, sender).unwrap();
                            true
                        }
                        Outcome::Standalone => panic!("racer {i} ran standalone"),
                    }
                })
            })
            .collect();
        let primaries = racers.into_iter().map(|racer| racer.join().unwrap()).filter(|won| *won).count();
        assert_eq!(primaries, 1);
        let received: Vec<_> = (1..RACERS).map(|_| next_request(&mut receiver)).collect();
        assert!(received.iter().all(|request| request.paths.len() == 1));
    }

    /// Thân của tiến trình con, chỉ chạy khi test dưới bật lại chính exe này.
    #[test]
    #[ignore = "tiến trình con của second_process_forwards_and_exits"]
    fn child_process_body() {
        let Some(tag) = std::env::var_os(CHILD_TAG) else { return };
        let endpoint = Endpoint { id: tag.into_string().unwrap(), check_server: false };
        let message = ForwardMessage { paths: vec!["con.flac".into()], cwd: absolute("child") };
        assert!(matches!(forward_or_claim(&endpoint, &message, CLAIM_PATIENCE), Outcome::Forwarded));
    }

    /// Hai tiến trình thật: đường dẫn tương đối ghép với `cwd` của phiên sau.
    #[test]
    fn second_process_forwards_and_exits() {
        let endpoint = Endpoint::for_test("process");
        let mut receiver = serve_channel(&endpoint);
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "instance::tests::child_process_body", "--ignored"])
            .env(CHILD_TAG, &endpoint.id)
            .output()
            .unwrap();
        assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stdout));
        assert_eq!(next_request(&mut receiver).paths, [absolute("child").join("con.flac")]);
    }

    /// Crash để lại socket chết: phiên mới dọn được và lên làm chính (đặc tả 5.5). Chạy trên Mac / Ubuntu.
    #[cfg(unix)]
    #[test]
    fn stale_socket_is_cleaned_up() {
        let endpoint = Endpoint::for_test("stale");
        let socket = Path::new(&endpoint.id);
        std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
        std::fs::write(socket, b"").unwrap();
        let message = ForwardMessage::default();
        assert!(matches!(forward_or_claim(&endpoint, &message, CLAIM_PATIENCE), Outcome::Primary(_)));
        std::fs::remove_dir_all(socket.parent().unwrap()).unwrap();
    }

    /// Socket 0600 trong thư mục 0700. Chạy trên Mac / Ubuntu.
    #[cfg(unix)]
    #[test]
    fn socket_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let endpoint = Endpoint::for_test("private");
        let Ok(claimed) = endpoint.claim() else { panic!("claim") };
        let socket = Path::new(&endpoint.id);
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(socket), 0o600);
        assert_eq!(mode(socket.parent().unwrap()), 0o700);
        drop(claimed);
        std::fs::remove_dir_all(socket.parent().unwrap()).unwrap();
    }
}
