//! groove-gd1_26: ITaskbarList3 (đặc tả 1.12, 5.3): Prev, Play / Pause, Next trên ảnh thu nhỏ của taskbar.
//!
//! Ba luồng, không chỗ nào gọi Shell trên UI thread: gọi Win32 ở đó có thể chạy lồng wndproc của GPUI trong lúc `cx` đang mượn App
//! ("RefCell already borrowed", `fastcast/CLAUDE.md`).
//!
//! - **UI thread**: chỉ cài subclass (Win32 bắt buộc cùng luồng với cửa sổ) và đẩy trạng thái khi nó đổi.
//! - **Subclass**: bắt `WM_COMMAND` + `THBN_CLICKED`, `TaskbarButtonCreated`, `WM_DPICHANGED`. Chạy lồng được nên chỉ đẩy vào kênh,
//!   không chạm `cx`.
//! - **Luồng `taskbar`**: STA riêng, giữ `ITaskbarList3` và `HICON`. Có vòng `GetMessage` thật vì STA phải bơm thông điệp; lúc nghỉ
//!   không thức dậy.

use std::ffi::c_void;
use std::sync::{Arc, mpsc};
use std::thread;

use futures::StreamExt;
use futures::channel::mpsc as async_mpsc;
use groove_core::keymap::Command;
use groove_core::t;
use materiant::gpui_kit::{AnyWindowHandle, App};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass, THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_DISABLED,
    THBF_ENABLED, THBN_CLICKED, THUMBBUTTON, TaskbarList,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateIconFromResourceEx, DestroyIcon, DispatchMessageW, GetMessageW, HICON,
    LR_DEFAULTCOLOR, MSG, MSGFLT_ALLOW, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, RegisterWindowMessageW,
    TranslateMessage, WM_APP, WM_COMMAND, WM_DPICHANGED, WM_NCDESTROY, WM_QUIT,
};
use windows_sys::core::{GUID, HRESULT};

use crate::player::player;

const ID_PREV: u32 = 1;
const ID_TOGGLE: u32 = 2;
const ID_NEXT: u32 = 3;
const SUBCLASS_ID: usize = 0x6772_6f6f;
/// Đánh thức luồng `taskbar` khi kênh có việc.
const WM_WAKE: u32 = WM_APP + 1;
/// Cỡ icon nút ở 96 DPI.
const ICON_SIZE: u32 = 16;
/// Phiên bản định dạng `CreateIconFromResourceEx` đòi.
const ICON_VERSION: u32 = 0x0003_0000;

static PREV_ICO: &[u8] = include_bytes!("../../assets/taskbar/prev.ico");
static PLAY_ICO: &[u8] = include_bytes!("../../assets/taskbar/play.ico");
static PAUSE_ICO: &[u8] = include_bytes!("../../assets/taskbar/pause.ico");
static NEXT_ICO: &[u8] = include_bytes!("../../assets/taskbar/next.ico");

/// Thứ tự đúng theo `shobjidl_core.h`; `windows-sys` không có giao diện COM.
#[repr(C)]
struct TaskbarListVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut TaskbarListObject) -> u32,
    hr_init: unsafe extern "system" fn(*mut TaskbarListObject) -> HRESULT,
    add_tab: usize,
    delete_tab: usize,
    activate_tab: usize,
    set_active_alt: usize,
    mark_fullscreen_window: usize,
    set_progress_value: usize,
    set_progress_state: usize,
    register_tab: usize,
    unregister_tab: usize,
    set_tab_order: usize,
    set_tab_active: usize,
    thumb_bar_add_buttons: unsafe extern "system" fn(*mut TaskbarListObject, HWND, u32, *const THUMBBUTTON) -> HRESULT,
    thumb_bar_update_buttons:
        unsafe extern "system" fn(*mut TaskbarListObject, HWND, u32, *const THUMBBUTTON) -> HRESULT,
}

#[repr(C)]
struct TaskbarListObject {
    vtbl: *const TaskbarListVtbl,
}

const IID_ITASKBAR_LIST3: GUID = GUID::from_u128(0xea1afb91_9e28_4b86_90e9_9e9f8a5eefaf);

/// Điều quyết định ba nút. Bản cũ: Prev, Next chỉ bật khi có bài, nút giữa luôn bật và đổi theo trạng thái
/// (`src/shared/stores/audio-player.ts:430`); Play khi chưa có bài do `Player::play` tự lấy bài kế.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub has_track: bool,
    pub playing: bool,
}

/// `id` của nút ra lệnh, qua dispatcher chung với phím tắt (bảng 2.4).
fn command_of(id: u32) -> Option<Command> {
    match id {
        ID_PREV => Some(Command::PrevTrack),
        ID_TOGGLE => Some(Command::TogglePlay),
        ID_NEXT => Some(Command::NextTrack),
        _ => None,
    }
}

fn wide_tip(text: &str) -> [u16; 260] {
    let mut tip = [0u16; 260];
    // Chừa ô cuối cho số 0 kết thúc.
    for (slot, unit) in tip.iter_mut().take(259).zip(text.encode_utf16()) {
        *slot = unit;
    }
    tip
}

/// Lấy ảnh hợp nhất trong tệp `.ico`: nhỏ nhất không dưới `want`, không có thì lớn nhất. `CreateIconFromResourceEx` tự co.
fn icon_image(ico: &[u8], want: u32) -> Option<&[u8]> {
    const HEADER: usize = 6;
    const ENTRY: usize = 16;
    let count = usize::from(u16::from_le_bytes([*ico.get(4)?, *ico.get(5)?]));
    let mut best: Option<(u32, usize, usize)> = None;
    for index in 0..count {
        let entry = ico.get(HEADER + index * ENTRY..HEADER + (index + 1) * ENTRY)?;
        let width = if entry[0] == 0 { 256 } else { u32::from(entry[0]) };
        let size = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
        let better = match best {
            None => true,
            Some((current, ..)) if current >= want => width >= want && width < current,
            Some((current, ..)) => width > current,
        };
        if better {
            best = Some((width, offset, size));
        }
    }
    let (_, offset, size) = best?;
    ico.get(offset..offset.checked_add(size)?)
}

fn load_icon(ico: &[u8], size: u32) -> HICON {
    let Some(image) = icon_image(ico, size) else {
        return std::ptr::null_mut();
    };
    let size = size as i32;
    // SAFETY: `image` là khối ảnh hợp lệ theo độ dài đã kiểm, hàm chỉ đọc; `fIcon` = 1.
    unsafe {
        CreateIconFromResourceEx(image.as_ptr(), image.len() as u32, 1, ICON_VERSION, size, size, LR_DEFAULTCOLOR)
    }
}

struct Icons {
    prev: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

impl Icons {
    fn load(dpi: u32) -> Icons {
        let size = (ICON_SIZE * dpi.max(96)).div_ceil(96);
        Icons {
            prev: load_icon(PREV_ICO, size),
            play: load_icon(PLAY_ICO, size),
            pause: load_icon(PAUSE_ICO, size),
            next: load_icon(NEXT_ICO, size),
        }
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        for icon in [self.prev, self.play, self.pause, self.next] {
            if !icon.is_null() {
                // SAFETY: icon do `Icons::load` tạo, chỉ huỷ một lần ở đây.
                unsafe { DestroyIcon(icon) };
            }
        }
    }
}

/// Chữ tách khỏi `buttons`: ngôn ngữ là global của tiến trình, test không nên phụ thuộc.
struct Tips {
    previous: &'static str,
    play: &'static str,
    pause: &'static str,
    next: &'static str,
}

impl Tips {
    fn current() -> Tips {
        Tips {
            previous: t!("platform.previous"),
            play: t!("platform.play"),
            pause: t!("platform.pause"),
            next: t!("platform.next"),
        }
    }
}

fn buttons(state: ButtonState, icons: &Icons, tips: &Tips) -> [THUMBBUTTON; 3] {
    let navigation = if state.has_track { THBF_ENABLED } else { THBF_DISABLED };
    let button = |id, icon, tip: &str, flags| THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        iBitmap: 0,
        hIcon: icon,
        szTip: wide_tip(tip),
        dwFlags: flags,
    };
    let (toggle_icon, toggle_tip) = if state.playing { (icons.pause, tips.pause) } else { (icons.play, tips.play) };
    [
        button(ID_PREV, icons.prev, tips.previous, navigation),
        button(ID_TOGGLE, toggle_icon, toggle_tip, THBF_ENABLED),
        button(ID_NEXT, icons.next, tips.next, navigation),
    ]
}

// ---- Luồng `taskbar` ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Message {
    State(ButtonState),
    /// Taskbar vừa dựng nút của cửa sổ, kể cả sau khi Explorer khởi động lại: nút cũ đã mất.
    ButtonCreated,
    Dpi(u32),
}

/// Đầu gửi cho luồng `taskbar`. Gửi được từ subclass: `PostThreadMessageW` không chạy lồng. Bỏ đầu gửi cuối là luồng thoát.
struct WorkerHandle {
    tx: crossbeam_channel::Sender<Message>,
    thread_id: u32,
}

impl WorkerHandle {
    fn send(&self, message: Message) {
        if self.tx.send(message).is_ok() {
            // SAFETY: chỉ đẩy thông điệp vào hàng đợi; luồng đã chết thì trả lỗi, bỏ qua được.
            unsafe { PostThreadMessageW(self.thread_id, WM_WAKE, 0, 0) };
        }
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        // SAFETY: như `send`.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
    }
}

struct Worker {
    hwnd: HWND,
    list: *mut TaskbarListObject,
    icons: Icons,
    state: ButtonState,
    added: bool,
}

impl Worker {
    fn handle(&mut self, message: Message) {
        match message {
            Message::State(state) => self.state = state,
            Message::ButtonCreated => self.added = false,
            Message::Dpi(dpi) => self.icons = Icons::load(dpi),
        }
        self.publish();
    }

    /// `ThumbBarAddButtons` chỉ nhận một lần cho mỗi nút taskbar và hỏng khi nút chưa có. Thứ tự của `TaskbarButtonCreated` so với lần
    /// thêm đầu không chắc, nên thêm hỏng thì thử cập nhật trước khi kết luận.
    fn publish(&mut self) {
        if self.list.is_null() {
            return;
        }
        let buttons = buttons(self.state, &self.icons, &Tips::current());
        // SAFETY: `list` là `ITaskbarList3` đã `HrInit` của chính luồng này; `buttons` sống tới hết lời gọi.
        unsafe {
            let vtbl = &*(*self.list).vtbl;
            if !self.added {
                let hr = (vtbl.thumb_bar_add_buttons)(self.list, self.hwnd, 3, buttons.as_ptr());
                if hr >= 0 {
                    self.added = true;
                    return;
                }
                log::debug!("taskbar add buttons: {hr:#x}");
            }
            let hr = (vtbl.thumb_bar_update_buttons)(self.list, self.hwnd, 3, buttons.as_ptr());
            if hr >= 0 {
                self.added = true;
            } else {
                // Cửa sổ chưa có nút taskbar: `TaskbarButtonCreated` sẽ gọi lại.
                log::debug!("taskbar update buttons: {hr:#x}");
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if !self.list.is_null() {
            // SAFETY: nhả tham chiếu `CoCreateInstance` đã cho, một lần.
            unsafe { ((*(*self.list).vtbl).release)(self.list) };
        }
    }
}

fn create_list() -> *mut TaskbarListObject {
    let mut list: *mut c_void = std::ptr::null_mut();
    // SAFETY: GUID đúng của `TaskbarList` / `ITaskbarList3`; `list` chỉ dùng khi thành công.
    unsafe {
        let hr =
            CoCreateInstance(&TaskbarList, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, &IID_ITASKBAR_LIST3, &mut list);
        if hr < 0 || list.is_null() {
            log::warn!("cannot create the taskbar list: {hr:#x}");
            return std::ptr::null_mut();
        }
        let list = list.cast::<TaskbarListObject>();
        let hr = ((*(*list).vtbl).hr_init)(list);
        if hr < 0 {
            log::warn!("taskbar list init: {hr:#x}");
            ((*(*list).vtbl).release)(list);
            return std::ptr::null_mut();
        }
        list
    }
}

/// Trả đầu gửi khi luồng đã có hàng đợi thông điệp, và cờ `ITaskbarList3` có dùng được không.
fn spawn_worker(hwnd: isize, dpi: u32) -> Option<(WorkerHandle, bool, thread::JoinHandle<()>)> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let thread = thread::Builder::new()
        .name("taskbar".into())
        .spawn(move || {
            // SAFETY: khởi tạo COM của luồng mới; `msg` là bộ đệm của `PeekMessageW`, lời gọi này tạo hàng đợi thông điệp.
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            let thread_id = unsafe {
                CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
                PeekMessageW(&mut msg, std::ptr::null_mut(), WM_APP, WM_APP, PM_NOREMOVE);
                GetCurrentThreadId()
            };
            let mut worker = Worker {
                hwnd: hwnd as HWND,
                list: create_list(),
                icons: Icons::load(dpi),
                state: ButtonState::default(),
                added: false,
            };
            let _ = ready_tx.send((thread_id, !worker.list.is_null()));
            // SAFETY: vòng bơm chuẩn của STA, `msg` do `GetMessageW` điền.
            while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                if msg.message == WM_WAKE {
                    for message in rx.try_iter() {
                        worker.handle(message);
                    }
                } else {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        })
        .map_err(|err| log::warn!("cannot start the taskbar thread: {err}"))
        .ok()?;
    let (thread_id, usable) = ready_rx.recv().ok()?;
    Some((WorkerHandle { tx, thread_id }, usable, thread))
}

// ---- Subclass ----

struct Shared {
    clicks: async_mpsc::UnboundedSender<Command>,
    worker: Arc<WorkerHandle>,
    button_created: u32,
}

fn low_word(value: usize) -> u32 {
    (value & 0xffff) as u32
}

fn high_word(value: usize) -> u32 {
    ((value >> 16) & 0xffff) as u32
}

/// # Safety
/// Chỉ Windows gọi, với `data` là `Box<Shared>` mà [`subclass`] đã giao.
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: `data` còn sống tới `WM_NCDESTROY` bên dưới.
    let shared = unsafe { &*(data as *const Shared) };
    if message == shared.button_created {
        shared.worker.send(Message::ButtonCreated);
        return 0;
    }
    match message {
        WM_COMMAND if high_word(wparam) == THBN_CLICKED => {
            if let Some(command) = command_of(low_word(wparam)) {
                let _ = shared.clicks.unbounded_send(command);
                return 0;
            }
        }
        WM_DPICHANGED => shared.worker.send(Message::Dpi(low_word(wparam))),
        WM_NCDESTROY => {
            // SAFETY: gỡ đúng subclass đã cài, sau đó không còn ai gọi với `data` này.
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                drop(Box::from_raw(data as *mut Shared));
                return DefSubclassProc(hwnd, message, wparam, lparam);
            }
        }
        _ => {}
    }
    // SAFETY: chuyển tiếp nguyên tham số cho wndproc của GPUI.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Phải gọi trên luồng sở hữu cửa sổ.
fn subclass(hwnd: HWND, clicks: async_mpsc::UnboundedSender<Command>, worker: Arc<WorkerHandle>) -> bool {
    let name: Vec<u16> = "TaskbarButtonCreated".encode_utf16().chain([0]).collect();
    // SAFETY: `name` kết thúc bằng 0; `shared` thuộc về subclass, nhả ở `WM_NCDESTROY` hoặc ngay dưới đây nếu cài hỏng. Ba lời gọi này
    // không gửi thông điệp cho cửa sổ nên không chạy lồng wndproc của GPUI.
    unsafe {
        let button_created = RegisterWindowMessageW(name.as_ptr());
        // Chạy bằng quyền admin thì Explorer (quyền thấp hơn) bị UIPI chặn nếu không mở lọc.
        ChangeWindowMessageFilterEx(hwnd, button_created, MSGFLT_ALLOW, std::ptr::null_mut());
        ChangeWindowMessageFilterEx(hwnd, WM_COMMAND, MSGFLT_ALLOW, std::ptr::null_mut());
        let shared = Box::into_raw(Box::new(Shared { clicks, worker, button_created }));
        let installed = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, shared as usize) != 0;
        if !installed {
            drop(Box::from_raw(shared));
        }
        installed
    }
}

// ---- Nối vào app ----

/// HWND của cửa sổ chính. Media session cũng cần (đặc tả 5.4).
pub fn hwnd_of(window: AnyWindowHandle, cx: &mut App) -> Option<isize> {
    let raw = window.update(cx, |_, window, _| {
        let handle = HasWindowHandle::window_handle(&*window).ok()?;
        match handle.as_raw() {
            RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
            _ => None,
        }
    });
    raw.ok().flatten()
}

/// Cài nút taskbar lên cửa sổ chính. Gọi một lần, sau `commands::install`.
pub fn install(hwnd: isize, window: AnyWindowHandle, cx: &mut App) {
    let scale = window.update(cx, |_, window, _| window.scale_factor()).unwrap_or(1.0);
    let Some((worker, usable, _thread)) = spawn_worker(hwnd, (scale * 96.0).round() as u32) else { return };
    if !usable {
        return;
    }
    let worker = Arc::new(worker);
    let (clicks, mut click_rx) = async_mpsc::unbounded();
    if !subclass(hwnd as HWND, clicks, worker.clone()) {
        log::warn!("cannot subclass the main window: taskbar buttons stay off");
        return;
    }
    // Sự kiện về UI thread rồi mới đụng `cx`.
    cx.spawn(async move |cx| {
        while let Some(command) = click_rx.next().await {
            cx.update(|cx| crate::commands::run(command, cx));
        }
    })
    .detach();

    let player = player(cx);
    let state_of = |player: &crate::player::Player| ButtonState {
        has_track: player.current_song().is_some(),
        playing: player.is_playing(),
    };
    let mut last = state_of(player.read(cx));
    // Thử thêm ngay: `TaskbarButtonCreated` có thể đã chạy trước khi subclass kịp cài.
    worker.send(Message::State(last));
    cx.observe(&player, move |player, cx| {
        // Observer chạy 4 lần mỗi giây khi phát; chỉ báo khi nút thật sự đổi.
        let now = state_of(player.read(cx));
        if now != last {
            last = now;
            worker.send(Message::State(now));
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, SendMessageW};

    use super::*;

    /// Cửa sổ ẩn, không có nút taskbar, không ai thấy.
    fn hidden_window() -> HWND {
        let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
        // SAFETY: lớp hệ thống có sẵn, không có `WS_VISIBLE`.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        hwnd
    }

    fn fake_worker() -> (Arc<WorkerHandle>, crossbeam_channel::Receiver<Message>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        (Arc::new(WorkerHandle { tx, thread_id: 0 }), rx)
    }

    #[test]
    fn every_button_reaches_its_command() {
        assert_eq!(command_of(ID_PREV), Some(Command::PrevTrack));
        assert_eq!(command_of(ID_TOGGLE), Some(Command::TogglePlay));
        assert_eq!(command_of(ID_NEXT), Some(Command::NextTrack));
        assert_eq!(command_of(0), None);
        assert_eq!(command_of(4), None);
    }

    #[test]
    fn icon_image_picks_the_closest_and_rejects_garbage() {
        for ico in [PREV_ICO, PLAY_ICO, PAUSE_ICO, NEXT_ICO] {
            assert!(icon_image(ico, 16).is_some_and(|image| !image.is_empty()));
        }
        assert_eq!(icon_image(&[], 16), None);
        assert_eq!(icon_image(&[0, 0, 1, 0, 5, 0, 1], 16), None, "khai 5 ảnh mà tệp cụt");
        // Hai ảnh giả 16 và 48 px, khai offset, độ dài đúng.
        let mut ico = vec![0, 0, 1, 0, 2, 0];
        for (width, offset, size) in [(16u8, 38u32, 1u32), (48, 39, 2)] {
            ico.extend([width, width, 0, 0, 1, 0, 32, 0]);
            ico.extend(size.to_le_bytes());
            ico.extend(offset.to_le_bytes());
        }
        ico.extend([0xaa, 0xbb, 0xbb]);
        assert_eq!(icon_image(&ico, 16), Some(&[0xaa][..]));
        assert_eq!(icon_image(&ico, 20), Some(&[0xbb, 0xbb][..]));
        assert_eq!(icon_image(&ico, 96), Some(&[0xbb, 0xbb][..]), "không có ảnh đủ lớn thì lấy lớn nhất");
        // Độ dài vượt tệp.
        ico[14] = 0xff;
        ico[15] = 0xff;
        ico[16] = 0xff;
        ico[17] = 0xff;
        assert_eq!(icon_image(&ico, 16), None);
    }

    #[test]
    fn icons_load_at_every_dpi() {
        for dpi in [96, 120, 144, 192, 288] {
            let icons = Icons::load(dpi);
            for icon in [icons.prev, icons.play, icons.pause, icons.next] {
                assert!(!icon.is_null(), "dpi {dpi}");
            }
        }
    }

    #[test]
    fn buttons_follow_track_and_playback() {
        let icons = Icons::load(96);
        let tips = Tips { previous: "P", play: "Pl", pause: "Pa", next: "N" };
        let tip = |button: &THUMBBUTTON| {
            let end = button.szTip.iter().position(|&unit| unit == 0).unwrap();
            String::from_utf16(&button.szTip[..end]).unwrap()
        };

        let empty = buttons(ButtonState::default(), &icons, &tips);
        assert_eq!(empty.map(|b| b.iId), [ID_PREV, ID_TOGGLE, ID_NEXT]);
        assert_eq!(empty.map(|b| b.dwFlags), [THBF_DISABLED, THBF_ENABLED, THBF_DISABLED]);

        let paused = buttons(ButtonState { has_track: true, playing: false }, &icons, &tips);
        assert_eq!(paused.map(|b| b.dwFlags), [THBF_ENABLED; 3]);
        assert_eq!(paused[1].hIcon, icons.play);
        assert_eq!(tip(&paused[0]), "P");
        assert_eq!(tip(&paused[1]), "Pl");
        assert_eq!(tip(&paused[2]), "N");

        let playing = buttons(ButtonState { has_track: true, playing: true }, &icons, &tips);
        assert_eq!(playing[1].hIcon, icons.pause);
        assert_eq!(tip(&playing[1]), "Pa");
    }

    #[test]
    fn long_tip_keeps_the_terminator() {
        let tip = wide_tip(&"x".repeat(500));
        assert_eq!(tip[258], u16::from(b'x'));
        assert_eq!(tip[259], 0);
    }

    #[test]
    fn subclass_turns_messages_into_events_and_cleans_up() {
        let hwnd = hidden_window();
        let (clicks, mut click_rx) = async_mpsc::unbounded();
        let (worker, rx) = fake_worker();
        assert!(subclass(hwnd, clicks, worker.clone()));
        let name: Vec<u16> = "TaskbarButtonCreated".encode_utf16().chain([0]).collect();
        // SAFETY: gửi cho cửa sổ ẩn của chính luồng test.
        unsafe {
            let click = |id: usize| SendMessageW(hwnd, WM_COMMAND, (THBN_CLICKED as usize) << 16 | id, 0);
            assert_eq!(click(ID_TOGGLE as usize), 0);
            click(ID_PREV as usize);
            click(ID_NEXT as usize);
            // Mã nút lạ, và `WM_COMMAND` thường (menu) không phải nút taskbar.
            click(9);
            SendMessageW(hwnd, WM_COMMAND, ID_NEXT as usize, 0);
            SendMessageW(hwnd, RegisterWindowMessageW(name.as_ptr()), 0, 0);
            SendMessageW(hwnd, WM_DPICHANGED, (144 << 16) | 144, 0);
        }
        let received: Vec<_> = std::iter::from_fn(|| click_rx.try_recv().ok()).collect();
        assert_eq!(received, [Command::TogglePlay, Command::PrevTrack, Command::NextTrack]);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [Message::ButtonCreated, Message::Dpi(144)]);

        assert_eq!(Arc::strong_count(&worker), 2, "subclass đang giữ đầu gửi");
        // SAFETY: cửa sổ của test; huỷ thì `WM_NCDESTROY` nhả `Shared`.
        assert_ne!(unsafe { DestroyWindow(hwnd) }, 0);
        assert_eq!(Arc::strong_count(&worker), 1, "subclass nhả khi cửa sổ huỷ");
    }

    /// COM, vtable, vòng bơm thật. Cửa sổ ẩn không có nút taskbar nên Shell trả lỗi, không được sập; thả đầu gửi thì luồng thoát.
    #[test]
    fn worker_survives_a_window_without_a_taskbar_button() {
        let hwnd = hidden_window();
        let (worker, usable, thread) = spawn_worker(hwnd as isize, 144).expect("worker thread");
        assert!(usable, "ITaskbarList3 phải tạo được và HrInit xanh");
        worker.send(Message::State(ButtonState { has_track: true, playing: true }));
        worker.send(Message::ButtonCreated);
        worker.send(Message::Dpi(192));
        drop(worker);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !thread.is_finished() {
            assert!(std::time::Instant::now() < deadline, "luồng taskbar không thoát");
            thread::sleep(Duration::from_millis(5));
        }
        thread.join().unwrap();
        // SAFETY: cửa sổ của test.
        unsafe { DestroyWindow(hwnd) };
    }

    /// Chủ dự án chạy tay: `cargo test -p groove-app taskbar_windows -- --ignored --nocapture`. Mở một cửa sổ thật có nút taskbar,
    /// không giành focus, 30 giây: rê lên nút taskbar phải thấy ba nút.
    #[test]
    #[ignore = "mở cửa sổ thật, chỉ kiểm tay"]
    fn thumb_buttons_on_a_real_window() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow, WS_OVERLAPPEDWINDOW};
        let class: Vec<u16> = "STATIC".encode_utf16().chain([0]).collect();
        // SAFETY: lớp hệ thống, cửa sổ của test.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                300,
                200,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        // SAFETY: như trên.
        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
        let (worker, usable, _thread) = spawn_worker(hwnd as isize, 96).expect("worker thread");
        assert!(usable);
        worker.send(Message::State(ButtonState { has_track: true, playing: false }));
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        let end = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < end {
            // SAFETY: bơm thông điệp của cửa sổ thử.
            unsafe {
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 1) != 0 {
                    DispatchMessageW(&msg);
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}
