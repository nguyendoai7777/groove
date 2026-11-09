//! groove-gd2_04: phím tắt toàn cục — nghe ở mức hệ thống, cả khi cửa sổ không focus (`KeyBinding::global`).
//!
//! Windows `RegisterHotKey`, macOS Carbon `RegisterEventHotKey`, cả hai qua crate `global-hotkey` (MIT / Apache-2.0, như
//! daemon của fastcast). Đăng ký hỏng nghĩa là app khác / OS đang giữ tổ hợp: nhớ lại để trang Phím tắt báo, không dừng app.
//! Linux chưa có: fastcast đi portal `GlobalShortcuts` (crate `global-hotkey` chỉ có X11), việc đó nằm ở backlog;
//! [`supported`] trả `false`, công tắc "Toàn cục" tắt.
//!
//! Đang ghi phím ở hộp sửa thì [`suspend`] gỡ hết khỏi OS (không thì bấm lại đúng tổ hợp đang gán sẽ chạy lệnh thay vì vào
//! bộ ghi). Bộ phím (`Registry`) tách khỏi OS qua [`Backend`] để test không đụng phím của máy.
//!
//! Windows: `GlobalHotKeyManager` tạo cửa sổ ẩn trên luồng gọi, `WM_HOTKEY` tới vòng thông điệp của luồng đó, nên
//! [`install`] phải chạy trên luồng UI của GPUI (vòng `PeekMessage` của GPUI phát cho mọi cửa sổ của luồng). macOS cũng
//! cần luồng chính.

use groove_core::keymap::{Command, KeySequence};
use groove_core::settings::KeyBinding;
use materiant::gpui_kit::{App, Global};

/// Nơi đăng ký phím thật, hoặc bản giả trong test.
pub trait Backend {
    /// `Ok(id)` là id sự kiện OS sẽ báo khi phím được bấm.
    fn register(&mut self, key: &KeySequence) -> Result<u32, String>;
    fn unregister(&mut self, key: &KeySequence);
}

/// Bộ phím toàn cục muốn có, phím đã đăng ký và lý do từng phím hỏng.
pub struct Registry<B: Backend> {
    backend: B,
    wanted: Vec<(KeySequence, Command)>,
    registered: Vec<(u32, KeySequence, Command)>,
    failed: Vec<(KeySequence, String)>,
    suspended: bool,
}

impl<B: Backend> Registry<B> {
    pub fn new(backend: B) -> Registry<B> {
        Registry { backend, wanted: Vec::new(), registered: Vec::new(), failed: Vec::new(), suspended: false }
    }

    /// Lấy các dòng `global` đúng dạng (một tổ hợp có phím bổ trợ, lệnh có thật), trùng phím thì dòng đầu thắng như `Keymap`;
    /// gỡ bộ cũ, đăng ký bộ mới (trừ lúc tạm gỡ).
    pub fn apply(&mut self, bindings: &[KeyBinding]) {
        let mut wanted: Vec<(KeySequence, Command)> = Vec::new();
        for binding in bindings.iter().filter(|binding| binding.global) {
            let (Ok(key), Some(command)) = (KeySequence::parse(&binding.key), Command::from_name(&binding.command))
            else {
                continue;
            };
            if key.global_issue().is_none() && !wanted.iter().any(|(other, _)| *other == key) {
                wanted.push((key, command));
            }
        }
        if wanted == self.wanted && self.failed.is_empty() {
            return;
        }
        self.wanted = wanted;
        if !self.suspended {
            self.register_all();
        }
    }

    pub fn suspend(&mut self, on: bool) {
        if on == self.suspended {
            return;
        }
        self.suspended = on;
        if on {
            self.unregister_all();
        } else {
            self.register_all();
        }
    }

    fn unregister_all(&mut self) {
        for (_, key, _) in self.registered.drain(..) {
            self.backend.unregister(&key);
        }
    }

    fn register_all(&mut self) {
        self.unregister_all();
        self.failed.clear();
        for (key, command) in self.wanted.clone() {
            match self.backend.register(&key) {
                Ok(id) => {
                    log::info!("global key: {key} -> {}", command.name());
                    self.registered.push((id, key, command));
                }
                Err(reason) => {
                    log::warn!("global key: cannot register {key} for {}: {reason}", command.name());
                    self.failed.push((key, reason));
                }
            }
        }
    }

    /// Lệnh của sự kiện OS `id`.
    #[cfg(any(windows, target_os = "macos", test))]
    pub fn command_for(&self, id: u32) -> Option<Command> {
        self.registered.iter().find(|(own, _, _)| *own == id).map(|&(_, _, command)| command)
    }

    /// Lý do OS từ chối `key` ở lần đăng ký gần nhất.
    pub fn failure(&self, key: &KeySequence) -> Option<&str> {
        self.failed.iter().find(|(other, _)| other == key).map(|(_, reason)| reason.as_str())
    }

    /// OS có nhận `key` không: đăng ký thử rồi gỡ ngay. Phím đang là của mình (kể cả lúc tạm gỡ) thì đã biết kết quả.
    pub fn probe(&mut self, key: &KeySequence) -> Result<(), String> {
        if let Some(reason) = self.failure(key) {
            return Err(reason.to_owned());
        }
        if self.wanted.iter().any(|(own, _)| own == key) {
            return Ok(());
        }
        self.backend.register(key).map(|_| self.backend.unregister(key))
    }
}

/// Có phím toàn cục trên OS này không.
pub const fn supported() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

struct GlobalKeys(Option<Registry<native::Native>>);

impl Global for GlobalKeys {}

/// Tạo bộ đăng ký trên luồng UI, nối sự kiện OS vào `commands::run`, đăng ký các dòng `global` của cài đặt. Gọi một lần
/// trong `app::run`, sau `commands::install`.
pub fn install(cx: &mut App) {
    let registry = native::Native::new(cx).map(Registry::new);
    cx.set_global(GlobalKeys(registry));
    let bindings = crate::app::settings(cx).keybindings();
    apply(&bindings, cx);
}

fn with<R>(cx: &mut App, f: impl FnOnce(&mut Registry<native::Native>) -> R) -> Option<R> {
    if !cx.has_global::<GlobalKeys>() {
        return None;
    }
    cx.global_mut::<GlobalKeys>().0.as_mut().map(f)
}

/// Đăng ký lại theo danh sách phím vừa lưu (OK của hộp Settings).
pub fn apply(bindings: &[KeyBinding], cx: &mut App) {
    with(cx, |registry| registry.apply(bindings));
}

/// Tạm gỡ (`true`) / đăng ký lại (`false`), quanh lúc ghi phím.
pub fn suspend(on: bool, cx: &mut App) {
    with(cx, |registry| registry.suspend(on));
}

/// Lý do OS từ chối `key` lần đăng ký gần nhất; `None` khi đã nhận hoặc không phải phím toàn cục.
pub fn failure(key: &KeySequence, cx: &App) -> Option<String> {
    let keys = cx.try_global::<GlobalKeys>()?;
    keys.0.as_ref()?.failure(key).map(str::to_owned)
}

/// Đăng ký thử `key` (hộp sửa, lúc bật "Toàn cục"). Không có bộ đăng ký (Linux, test) thì coi như được.
pub fn probe(key: &KeySequence, cx: &mut App) -> Result<(), String> {
    with(cx, |registry| registry.probe(key)).unwrap_or(Ok(()))
}

#[cfg(any(windows, target_os = "macos"))]
fn run_event(id: u32, cx: &mut App) {
    let command = cx.try_global::<GlobalKeys>().and_then(|keys| keys.0.as_ref()?.command_for(id));
    if let Some(command) = command {
        crate::commands::run(command, cx);
    }
}

#[cfg(any(windows, target_os = "macos"))]
mod native {
    use futures::StreamExt;
    use futures::channel::mpsc;
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    use groove_core::keymap::KeySequence;
    use materiant::gpui_kit::App;

    pub struct Native(GlobalHotKeyManager);

    impl Native {
        /// `None` khi OS không cho tạo bộ đăng ký (đã ghi log): phím toàn cục im lặng không chạy, app vẫn chạy.
        pub fn new(cx: &mut App) -> Option<Native> {
            let manager = match GlobalHotKeyManager::new() {
                Ok(manager) => manager,
                Err(err) => {
                    log::error!("global key: cannot create the manager: {err}");
                    return None;
                }
            };
            let (tx, mut rx) = mpsc::unbounded::<u32>();
            // Handler gọi trên luồng OS báo phím (Windows: luồng UI; macOS: luồng chính), chỉ đẩy id sang UI.
            GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
                if event.state == HotKeyState::Pressed {
                    let _ = tx.unbounded_send(event.id);
                }
            }));
            cx.spawn(async move |cx| {
                while let Some(id) = rx.next().await {
                    cx.update(|cx| super::run_event(id, cx));
                }
            })
            .detach();
            Some(Native(manager))
        }

        #[cfg(test)]
        pub fn bare() -> Native {
            Native(GlobalHotKeyManager::new().expect("hotkey manager"))
        }
    }

    impl super::Backend for Native {
        fn register(&mut self, key: &KeySequence) -> Result<u32, String> {
            let hotkey = to_native(key).ok_or_else(|| format!("{key} has no system key code"))?;
            self.0.register(hotkey).map(|()| hotkey.id()).map_err(|err| err.to_string())
        }

        fn unregister(&mut self, key: &KeySequence) {
            if let Some(hotkey) = to_native(key)
                && let Err(err) = self.0.unregister(hotkey)
            {
                log::warn!("global key: unregister {key}: {err}");
            }
        }
    }

    /// Tổ hợp groove → `HotKey`. groove gộp Ctrl với Cmd (`Chord::from_event`), nên trên macOS `ctrl` là Cmd như phím trong
    /// app.
    pub fn to_native(key: &KeySequence) -> Option<HotKey> {
        let [chord] = key.chords() else { return None };
        let primary = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };
        let mut mods = Modifiers::empty();
        for (on, modifier) in
            [(chord.ctrl(), primary), (chord.alt(), Modifiers::ALT), (chord.shift(), Modifiers::SHIFT)]
        {
            if on {
                mods |= modifier;
            }
        }
        Some(HotKey::new(Some(mods), code(chord.key())?))
    }

    fn code(key: &str) -> Option<Code> {
        let mut chars = key.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            let name = match c {
                'a'..='z' => format!("Key{}", c.to_ascii_uppercase()),
                '0'..='9' => format!("Digit{c}"),
                _ => return punctuation(c),
            };
            return name.parse().ok();
        }
        if let Some(n) = key.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
            return (1..=24).contains(&n).then(|| format!("F{n}").parse().ok()).flatten();
        }
        Some(match key {
            "space" => Code::Space,
            "enter" => Code::Enter,
            "escape" => Code::Escape,
            "tab" => Code::Tab,
            "backspace" => Code::Backspace,
            "delete" => Code::Delete,
            "insert" => Code::Insert,
            "home" => Code::Home,
            "end" => Code::End,
            "pageup" => Code::PageUp,
            "pagedown" => Code::PageDown,
            "arrowup" => Code::ArrowUp,
            "arrowdown" => Code::ArrowDown,
            "arrowleft" => Code::ArrowLeft,
            "arrowright" => Code::ArrowRight,
            _ => return None,
        })
    }

    fn punctuation(c: char) -> Option<Code> {
        Some(match c {
            '-' => Code::Minus,
            '=' => Code::Equal,
            ',' => Code::Comma,
            '.' => Code::Period,
            '/' => Code::Slash,
            '\\' => Code::Backslash,
            ';' => Code::Semicolon,
            '\'' => Code::Quote,
            '`' => Code::Backquote,
            '[' => Code::BracketLeft,
            ']' => Code::BracketRight,
            _ => return None,
        })
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod native {
    use groove_core::keymap::KeySequence;
    use materiant::gpui_kit::App;

    /// Linux: chưa có phím toàn cục (đầu tệp).
    pub enum Native {}

    impl Native {
        pub fn new(_: &mut App) -> Option<Native> {
            None
        }
    }

    impl super::Backend for Native {
        fn register(&mut self, _: &KeySequence) -> Result<u32, String> {
            match *self {}
        }

        fn unregister(&mut self, _: &KeySequence) {
            match *self {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    /// OS giả: giữ sẵn `taken`, ghi lại mọi lần đăng ký / gỡ.
    #[derive(Clone, Default)]
    struct Fake {
        taken: Vec<String>,
        live: Rc<RefCell<Vec<String>>>,
    }

    impl Backend for Fake {
        fn register(&mut self, key: &KeySequence) -> Result<u32, String> {
            let text = key.to_string();
            if self.taken.contains(&text) || self.live.borrow().contains(&text) {
                return Err("taken".into());
            }
            self.live.borrow_mut().push(text);
            Ok(self.live.borrow().len() as u32)
        }

        fn unregister(&mut self, key: &KeySequence) {
            self.live.borrow_mut().retain(|other| *other != key.to_string());
        }
    }

    fn global(key: &str, command: &str) -> KeyBinding {
        KeyBinding { global: true, ..KeyBinding::new(key, command) }
    }

    fn seq(text: &str) -> KeySequence {
        KeySequence::parse(text).unwrap()
    }

    #[test]
    fn only_valid_global_rows_are_registered_and_failures_are_remembered() {
        let fake = Fake { taken: vec!["ctrl+f8".into()], ..Fake::default() };
        let live = fake.live.clone();
        let mut registry = Registry::new(fake);
        registry.apply(&[
            global("ctrl+f7", "next_track"),
            KeyBinding::new("ctrl+f9", "prev_track"),
            global("g m", "go_to_library"),
            global("space", "toggle_play"),
            global("ctrl+f8", "prev_track"),
            global("ctrl+f7", "toggle_play"),
            global("ctrl+f10", "launch_rocket"),
        ]);
        assert_eq!(*live.borrow(), ["ctrl+f7"]);
        assert_eq!(registry.failure(&seq("ctrl+f8")), Some("taken"));
        assert_eq!(registry.failure(&seq("ctrl+f7")), None);
        let id = registry.registered[0].0;
        assert_eq!(registry.command_for(id), Some(Command::NextTrack), "trùng phím thì dòng đầu thắng");

        // Lưu bộ khác: gỡ phím cũ, quên lỗi cũ.
        registry.apply(&[global("alt+f1", "toggle_play")]);
        assert_eq!(*live.borrow(), ["alt+f1"]);
        assert_eq!(registry.failure(&seq("ctrl+f8")), None);
    }

    #[test]
    fn suspend_unregisters_and_resume_registers_again() {
        let fake = Fake::default();
        let live = fake.live.clone();
        let mut registry = Registry::new(fake);
        registry.apply(&[global("ctrl+f7", "next_track")]);
        registry.suspend(true);
        assert!(live.borrow().is_empty());
        // Lưu trong lúc tạm gỡ: chỉ nhớ, chưa đăng ký.
        registry.apply(&[global("ctrl+f6", "prev_track")]);
        assert!(live.borrow().is_empty());
        registry.suspend(false);
        assert_eq!(*live.borrow(), ["ctrl+f6"]);
    }

    #[test]
    fn probe_tries_the_os_and_leaves_nothing_registered() {
        let fake = Fake { taken: vec!["ctrl+f8".into()], ..Fake::default() };
        let live = fake.live.clone();
        let mut registry = Registry::new(fake);
        registry.apply(&[global("ctrl+f7", "next_track")]);
        assert_eq!(registry.probe(&seq("ctrl+f8")), Err("taken".into()));
        assert_eq!(registry.probe(&seq("ctrl+f5")), Ok(()));
        assert_eq!(registry.probe(&seq("ctrl+f7")), Ok(()), "phím của mình");
        registry.suspend(true);
        assert_eq!(registry.probe(&seq("ctrl+f7")), Ok(()), "phím của mình, kể cả lúc tạm gỡ");
        registry.suspend(false);
        assert_eq!(*live.borrow(), ["ctrl+f7"]);
    }

    /// C7 — OS thật: một bộ đăng ký khác (như app khác) giữ tổ hợp thì đăng ký / thử của mình bị từ chối. Tổ hợp hiếm
    /// (`Ctrl+Alt+Shift+F23`) để không đụng phím của máy; gỡ ngay khi xong.
    #[cfg(windows)]
    #[test]
    fn os_refuses_a_combination_held_by_someone_else() {
        let key = seq("ctrl+alt+shift+f23");
        let mut other = native::Native::bare();
        other.register(&key).expect("tổ hợp rảnh");
        let mut registry = Registry::new(native::Native::bare());
        assert!(registry.probe(&key).is_err(), "thử phải thấy bị giữ");
        registry.apply(&[global("ctrl+alt+shift+f23", "next_track")]);
        assert!(registry.failure(&key).is_some(), "đăng ký phải báo hỏng");
        other.unregister(&key);
        // Bên kia nhả: thử lại được, nhưng không để lại đăng ký.
        assert_eq!(
            registry.probe(&key),
            Err(registry.failure(&key).unwrap().to_owned()),
            "lỗi cũ còn tới lần apply sau"
        );
        registry.apply(&[global("ctrl+alt+shift+f23", "next_track")]);
        assert_eq!(registry.failure(&key), None);
        registry.suspend(true);
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn every_global_capable_key_maps_to_a_system_code() {
        for key in ["ctrl+f7", "alt+p", "ctrl+shift+arrowright", "f24", "ctrl+0", "alt+space", "ctrl+/", "ctrl+delete"]
        {
            assert!(native::to_native(&seq(key)).is_some(), "{key}");
        }
        assert!(native::to_native(&seq("g m")).is_none());
    }
}
