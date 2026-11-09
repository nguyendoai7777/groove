//! groove-gd1_22: dispatcher Command chung.
//!
//! Mọi nguồn lệnh đi một đường: phím tắt, bảng lệnh, media session, taskbar đều gọi [`run`]. `/play` và các lệnh chữ đi qua
//! [`run_cli`].
//!
//! # Phím tắt theo ngữ cảnh (M4)
//!
//! Bộ chặn phím của GPUI ([`on_keystroke`]) chạy trước mọi action. Nó đọc ngữ cảnh đang focus rồi đưa cho
//! `keymap::KeyMatcher`, không dùng `KeyBinding` của GPUI vì: chuỗi của GPUI quên sau 1 giây chứ không phải 800 ms; `meta` tính
//! là `ctrl`; đổi phím lúc chạy phải xoá cả phím của materiant. Quyết định phạm vi ở [`scope_of`]:
//!
//! - Tầng phủ (dialog, popover, menu) đang mở: không phím nào chạy, kể cả `Ctrl+K`.
//! - Đang gõ trong ô nhập: chỉ `Ctrl+K`.
//! - Thành phần đang focus có phím riêng (`VirtualList` giữ ↑ ↓, `Slider` giữ ← →): nó giữ phím đó.
//! - Space, Enter trên phần tử đến bằng Tab là bấm phần tử đó; focus do bấm chuột thì không, kẻo bấm Next xong Space lại Next.

use std::rc::Rc;

use groove_core::cli::CliCommand;
use groove_core::keymap::{Chord, Command, KeyMatcher, KeyScope, Keymap};
use groove_core::model::TrackRef;
use groove_core::settings::{KeyBinding, Settings, ThemeChoice};
use groove_library::db::queries;
use materiant::gpui_kit::{
    App, FocusHandle, Global, InputPreference, KeyContext, Keystroke, KeystrokeEvent, Task, Window,
};
use materiant::overlay::{dialog, stack};

use crate::app;
use crate::player::player;
use crate::route::Route;

/// Ba thao tác nhanh khi bảng lệnh trống (đặc tả 1.9). S21: cả ba đều chạy.
pub const QUICK_ACTIONS: [Command; 3] = [Command::PlayRandom, Command::GoToLibrary, Command::GoToNowPlaying];

/// Số bài `/play <tên>` lấy về, cùng trần với bảng lệnh (đặc tả 1.9).
const CLI_SEARCH_LIMIT: usize = 50;
/// Ngữ cảnh của ô nhập trong gpui-base (`input/base/state.rs`, không `pub`).
const TEXT_INPUT_CONTEXT: &str = "Input";

/// Phần của khung cửa sổ mà lệnh cần. `app::Shell` và cài đặt là `Global` riêng của `app.rs`, test dùng bản giả.
trait Host {
    fn open_search(&self, cx: &mut App);
    fn navigate(&self, route: Route, cx: &mut App);
    fn seek_step(&self, cx: &App) -> u32;
    fn volume_step(&self, cx: &App) -> u8;
    fn custom_keybindings(&self, cx: &App) -> Option<Vec<KeyBinding>>;
    fn theme(&self, cx: &App) -> ThemeChoice;
    fn set_theme(&self, theme: ThemeChoice, cx: &mut App);
}

struct AppHost;

impl Host for AppHost {
    fn open_search(&self, cx: &mut App) {
        app::open_search(cx);
    }

    fn navigate(&self, route: Route, cx: &mut App) {
        app::navigate(route, cx);
    }

    fn seek_step(&self, cx: &App) -> u32 {
        app::settings(cx).seek_step
    }

    fn volume_step(&self, cx: &App) -> u8 {
        app::settings(cx).volume_step
    }

    fn custom_keybindings(&self, cx: &App) -> Option<Vec<KeyBinding>> {
        app::settings(cx).custom_keybindings.clone()
    }

    fn theme(&self, cx: &App) -> ThemeChoice {
        app::settings(cx).theme
    }

    fn set_theme(&self, theme: ThemeChoice, cx: &mut App) {
        app::update_settings(cx, |settings| settings.theme = theme);
    }
}

struct CommandCenter {
    host: Rc<dyn Host>,
    keymap: Keymap,
    /// Nguồn của `keymap`. So ở mỗi phím thay vì nghe `Settings`: `app.rs` không cho quan sát từ ngoài, và phím thưa.
    source: Option<Vec<KeyBinding>>,
    matcher: KeyMatcher,
    /// Phần tử vừa tới bằng Tab, xem đầu tệp.
    tabbed_focus: Option<FocusHandle>,
    #[cfg(test)]
    ran: Vec<Command>,
}

impl Global for CommandCenter {}

/// Bảng phím từ cài đặt. `None` là mặc định, để phím mới của bản sau tới được người chưa từng sửa.
fn build_keymap(source: &Option<Vec<KeyBinding>>) -> Keymap {
    let bindings = Settings { custom_keybindings: source.clone(), ..Settings::default() }.keybindings();
    let issues = groove_core::keymap::validate(&bindings);
    if !issues.is_empty() {
        log::warn!("custom keybindings: {} bad lines ignored", issues.len());
    }
    Keymap::from_bindings(&bindings)
}

/// Bật phím tắt. Gọi một lần trong `app::run`, sau `player::install`.
pub fn install(cx: &mut App) {
    install_with(Rc::new(AppHost), cx);
}

fn install_with(host: Rc<dyn Host>, cx: &mut App) {
    let source = host.custom_keybindings(cx);
    let keymap = build_keymap(&source);
    cx.set_global(CommandCenter {
        host,
        keymap,
        source,
        matcher: KeyMatcher::default(),
        tabbed_focus: None,
        #[cfg(test)]
        ran: Vec::new(),
    });
    cx.intercept_keystrokes(on_keystroke).detach();
    cx.observe_keystrokes(remember_tabbed_focus).detach();
}

fn host(cx: &App) -> Rc<dyn Host> {
    cx.global::<CommandCenter>().host.clone()
}

pub(crate) fn chord_of(keystroke: &Keystroke) -> Option<Chord> {
    let modifiers = &keystroke.modifiers;
    Chord::from_event(&keystroke.key, modifiers.control || modifiers.platform, modifiers.alt, modifiers.shift)
}

fn on_keystroke(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    let Some(chord) = chord_of(&event.keystroke) else {
        return;
    };
    let scope = scope_of(event, window, cx);
    refresh_keymap(cx);
    let now = cx.background_executor().now();
    let CommandCenter { keymap, matcher, .. } = cx.global_mut::<CommandCenter>();
    let command = matcher.press(keymap, chord, scope, now);
    if let Some(command) = command {
        cx.stop_propagation();
        run(command, cx);
    }
}

/// Luật M4, xem đầu tệp.
fn scope_of(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) -> KeyScope {
    let contexts = window.context_stack();
    let in_context = |name: &str| contexts.iter().any(|context: &KeyContext| context.contains(name));
    if stack::depth(window, cx) > 0 || in_context(dialog::KEY_CONTEXT) {
        return KeyScope::Blocked;
    }
    // AltGr trên bàn phím quốc tế đang gõ chữ, không phải phím tắt.
    if event.input_preference == InputPreference::CharacterInput {
        return KeyScope::Blocked;
    }
    if in_context(TEXT_INPUT_CONTEXT) {
        return KeyScope::TextInput;
    }
    if widget_owns_key(&event.keystroke, &contexts, cx) || activates_tabbed_element(&event.keystroke, window, cx) {
        return KeyScope::Blocked;
    }
    KeyScope::Global
}

/// Có phím riêng trong đúng ngữ cảnh đang focus. Phím không gắn ngữ cảnh thì không tính, chúng không thuộc về thành phần nào.
fn widget_owns_key(keystroke: &Keystroke, contexts: &[KeyContext], cx: &App) -> bool {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let (bindings, _) = keymap.bindings_for_input(std::slice::from_ref(keystroke), contexts);
    bindings.iter().any(|binding| binding.predicate().is_some())
}

fn activates_tabbed_element(keystroke: &Keystroke, window: &Window, cx: &App) -> bool {
    if !matches!(keystroke.key.as_str(), "space" | "enter") {
        return false;
    }
    let tabbed = &cx.global::<CommandCenter>().tabbed_focus;
    tabbed.is_some() && *tabbed == window.focused(cx)
}

/// Chạy sau khi GPUI xử lý xong phím, nên đọc được focus mới của Tab.
fn remember_tabbed_focus(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    if event.keystroke.key == "tab" {
        let focused = window.focused(cx);
        cx.global_mut::<CommandCenter>().tabbed_focus = focused;
    }
}

fn refresh_keymap(cx: &mut App) {
    let center = cx.global::<CommandCenter>();
    let now = center.host.custom_keybindings(cx);
    if now != center.source {
        let keymap = build_keymap(&now);
        let center = cx.global_mut::<CommandCenter>();
        center.keymap = keymap;
        center.source = now;
        center.matcher.reset();
    }
}

/// Chạy một lệnh. Đường duy nhất cho phím tắt, thao tác nhanh của bảng lệnh, media session, taskbar.
pub fn run(command: Command, cx: &mut App) {
    #[cfg(test)]
    cx.global_mut::<CommandCenter>().ran.push(command);
    let host = host(cx);
    let player = player(cx);
    match command {
        Command::OpenSearch => host.open_search(cx),
        Command::GoToLibrary => host.navigate(Route::MyMusic, cx),
        Command::GoToNowPlaying => host.navigate(Route::NowPlaying, cx),
        Command::TogglePlay => player.update(cx, |p, cx| p.toggle_play(cx)),
        Command::PrevTrack => player.update(cx, |p, cx| p.prev(cx)),
        Command::NextTrack => player.update(cx, |p, cx| p.next(cx)),
        Command::PlayRandom => player.update(cx, |p, cx| p.play_random(cx)),
        Command::SeekBackward | Command::SeekForward => {
            let step = f64::from(host.seek_step(cx));
            let delta = if command == Command::SeekBackward { -step } else { step };
            player.update(cx, |p, cx| p.seek_by(delta, cx));
        }
        Command::VolumeUp | Command::VolumeDown => {
            let step = i16::from(host.volume_step(cx));
            let delta = if command == Command::VolumeDown { -step } else { step };
            player.update(cx, |p, cx| p.adjust_volume(delta, cx));
        }
    }
}

/// Nhãn phím đang gán cho lệnh, cho `Kbd` ở thao tác nhanh. `None` khi người dùng bỏ phím của lệnh này.
pub fn key_label(command: Command, cx: &App) -> Option<String> {
    cx.global::<CommandCenter>().keymap.sequence_for(command).map(|sequence| sequence.label())
}

/// Chọn một bài trong kết quả tìm của bảng lệnh (S21): chèn làm bài tiếp theo và phát, hàng đợi còn nguyên. Đang là bài đó thì
/// phát / dừng như bản cũ. Bảng lệnh không đóng (đặc tả 1.9).
pub fn play_search_result(track: TrackRef, cx: &mut App) {
    let player = player(cx);
    player.update(cx, |p, cx| {
        if p.current_track() == Some(&track) {
            p.toggle_play(cx);
        } else {
            p.play_track(track, cx);
        }
    });
}

/// Kết quả một lệnh chữ, để bảng lệnh quyết định đóng hay báo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliOutcome {
    /// Đã chạy, đóng bảng.
    Done,
    /// `/help`: giữ bảng mở, hiện `cli::COMMANDS` (điền `/` vào ô).
    ShowHelp,
    /// Không có bài nào khớp, hoặc `/playall` khi danh sách trống.
    NoMatch,
}

/// Chạy một lệnh chữ đã đọc bằng `cli::resolve`. Tìm bài thì đọc DB ở nền (N7), nên trả `Task`.
pub fn run_cli(command: CliCommand, cx: &mut App) -> Task<CliOutcome> {
    let player = player(cx);
    let outcome = match command {
        CliCommand::Play(Some(query)) => {
            return search_then(query, cx, |tracks, cx| {
                let player = crate::player::player(cx);
                let first = tracks[0].clone();
                player.update(cx, |p, cx| p.play_list(first, tracks, cx));
            });
        }
        CliCommand::Queue(query) => {
            return search_then(query, cx, |mut tracks, cx| {
                let player = crate::player::player(cx);
                tracks.truncate(1);
                player.update(cx, |p, cx| p.append(tracks, cx));
            });
        }
        CliCommand::Play(None) => {
            player.update(cx, |p, cx| p.play(cx));
            CliOutcome::Done
        }
        CliCommand::PlayAll => {
            let list = player.read(cx).queue().original().to_vec();
            match list.first().cloned() {
                Some(first) => {
                    player.update(cx, |p, cx| p.play_list(first, list, cx));
                    CliOutcome::Done
                }
                None => CliOutcome::NoMatch,
            }
        }
        CliCommand::Pause => {
            player.update(cx, |p, cx| p.pause(cx));
            CliOutcome::Done
        }
        CliCommand::Next => {
            player.update(cx, |p, cx| p.next(cx));
            CliOutcome::Done
        }
        CliCommand::Prev => {
            player.update(cx, |p, cx| p.prev(cx));
            CliOutcome::Done
        }
        CliCommand::Volume(volume) => {
            player.update(cx, |p, cx| p.set_volume(volume, cx));
            CliOutcome::Done
        }
        CliCommand::Theme(choice) => {
            let host = host(cx);
            let theme = choice.unwrap_or(match host.theme(cx) {
                ThemeChoice::Light => ThemeChoice::Dark,
                ThemeChoice::Dark | ThemeChoice::System => ThemeChoice::Light,
            });
            host.set_theme(theme, cx);
            CliOutcome::Done
        }
        CliCommand::Help => CliOutcome::ShowHelp,
    };
    Task::ready(outcome)
}

/// Tìm trong thư viện ở nền, có bài thì làm `then` trên UI thread. Dùng `read` thay vì `Library::search`: lệnh đang chạy không
/// được bị coi là lượt cũ khi ô tìm gõ tiếp (S18).
fn search_then(query: String, cx: &mut App, then: impl FnOnce(Vec<TrackRef>, &mut App) + 'static) -> Task<CliOutcome> {
    let Some(library) = app::try_library(cx).cloned() else {
        return Task::ready(CliOutcome::NoMatch);
    };
    cx.spawn(async move |cx| {
        let found = library.read(move |conn| Ok(queries::search(conn, &query, CLI_SEARCH_LIMIT)?)).await;
        let tracks: Vec<TrackRef> = match found {
            Ok(songs) => songs.into_iter().map(|song| song.track).collect(),
            Err(err) => {
                log::warn!("cli search: {err}");
                Vec::new()
            }
        };
        if tracks.is_empty() {
            return CliOutcome::NoMatch;
        }
        cx.update(|cx| then(tracks, cx));
        CliOutcome::Done
    })
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::path::Path;
    use std::time::{Duration, Instant};

    use gpui::{TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::cli;
    use groove_core::settings::DEFAULT_KEYBINDINGS;
    use groove_core::test_dir::TestDir;
    use groove_library::db::migrate::{CoverRecord, CoverSink};
    use groove_library::db::queries::SongUpsert;
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::store::{Library, LibraryConfig};
    use materiant::button::Button;
    use materiant::field::input::TextField;
    use materiant::gpui_kit::base::Root;
    use materiant::gpui_kit::prelude::*;
    use materiant::gpui_kit::{
        AppContext, Context, Entity, Focusable, IntoElement, KeyBinding as GpuiKeyBinding, KeyDownEvent, KeyUpEvent,
        Render, div, point, px, size,
    };
    use materiant::overlay::dialog::{Dialog, DialogHandle};

    use super::*;
    use crate::player::{self, PlayerDeps};

    materiant::gpui_kit::actions!(commands_test, [OwnerAction]);

    const OWNER_CONTEXT: &str = "OwnerWidget";
    /// Toạ độ góc trên trái của nút trong màn thử.
    const BUTTON_AT: f32 = 300.;

    #[derive(Default)]
    struct FakeHost {
        searches: Cell<usize>,
        routes: RefCell<Vec<Route>>,
        custom: RefCell<Option<Vec<KeyBinding>>>,
        theme: Cell<Option<ThemeChoice>>,
    }

    impl Host for FakeHost {
        fn open_search(&self, _: &mut App) {
            self.searches.set(self.searches.get() + 1);
        }

        fn navigate(&self, route: Route, _: &mut App) {
            self.routes.borrow_mut().push(route);
        }

        fn seek_step(&self, _: &App) -> u32 {
            5
        }

        fn volume_step(&self, _: &App) -> u8 {
            2
        }

        fn custom_keybindings(&self, _: &App) -> Option<Vec<KeyBinding>> {
            self.custom.borrow().clone()
        }

        fn theme(&self, _: &App) -> ThemeChoice {
            self.theme.get().unwrap_or(ThemeChoice::Dark)
        }

        fn set_theme(&self, theme: ThemeChoice, _: &mut App) {
            self.theme.set(Some(theme));
        }
    }

    /// Màn thử: ô nhập, nút, vùng có phím riêng, dialog.
    struct TestView {
        field: Entity<TextField>,
        owner: FocusHandle,
        dialog: DialogHandle,
        clicks: Rc<Cell<usize>>,
        owner_actions: Rc<Cell<usize>>,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            let owner_actions = self.owner_actions.clone();
            div()
                .size_full()
                .child(self.field.clone())
                .child(div().absolute().left(px(BUTTON_AT)).top(px(BUTTON_AT)).child(
                    Button::new("button", "Button").tab_index(1).on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                ))
                .child(
                    div()
                        .id("owner")
                        .key_context(OWNER_CONTEXT)
                        .track_focus(&self.owner)
                        .size(px(20.))
                        .on_action(move |_: &OwnerAction, _, _| owner_actions.set(owner_actions.get() + 1)),
                )
                .child(Dialog::new("dialog", &self.dialog).title("Dialog"))
        }
    }

    struct Rig<'a> {
        cx: &'a mut VisualTestContext,
        host: Rc<FakeHost>,
        view: Entity<TestView>,
    }

    impl Rig<'_> {
        fn press(&mut self, keys: &str) {
            self.cx.simulate_keystrokes(keys);
            self.draw();
        }

        /// `simulate_keystrokes` chỉ có nhấn; GPUI bấm nút bằng Space lúc nhả.
        fn press_with_release(&mut self, key: &'static str) {
            let keystroke = Keystroke::parse(key).unwrap();
            self.cx.simulate_event(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            self.cx.simulate_event(KeyUpEvent { keystroke });
            self.draw();
        }

        fn draw(&mut self) {
            self.cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            });
        }

        fn take_ran(&mut self) -> Vec<Command> {
            self.cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<CommandCenter>().ran))
        }

        fn focus(&mut self, get: impl FnOnce(&TestView, &App) -> FocusHandle) {
            let view = self.view.clone();
            self.cx.update(|window, cx| {
                let handle = get(view.read(cx), cx);
                window.focus(&handle, cx);
            });
            self.draw();
        }

        fn field_value(&mut self) -> String {
            let view = self.view.clone();
            self.cx.update(|_, cx| view.read(cx).field.read(cx).value(cx).to_string())
        }
    }

    fn player_deps(library: Option<Library>) -> PlayerDeps {
        PlayerDeps {
            library,
            output: Box::new(FakeOutput::new(48_000, 2)),
            options: EngineOptions {
                stall_timeout: Duration::from_secs(3600),
                reopen_interval: Duration::from_millis(1),
                reopen_attempts: 1,
                ..EngineOptions::default()
            },
            session_file: None,
        }
    }

    fn rig_with_library<'a>(cx: &'a mut TestAppContext, library: Option<Library>) -> Rig<'a> {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.executor().allow_parking();
        let host: Rc<FakeHost> = Rc::default();
        let installed = host.clone();
        cx.update(|cx| {
            materiant::init(cx);
            cx.bind_keys([GpuiKeyBinding::new("up", OwnerAction, Some(OWNER_CONTEXT))]);
            if let Some(library) = &library {
                app::set_library(library.clone(), cx);
            }
            player::install_for_test(player_deps(library), cx);
            install_with(installed, cx);
        });
        let window = cx.add_window(|window, cx| {
            let field = cx.new(|cx| TextField::new("field", window, cx));
            let view = cx.new(|cx| TestView {
                field,
                owner: cx.focus_handle(),
                dialog: DialogHandle::new(cx),
                clicks: Rc::default(),
                owner_actions: Rc::default(),
            });
            Root::new(view, window, cx)
        });
        let view = window.update(cx, |root, _, _| root.view().clone().downcast::<TestView>().unwrap()).unwrap();
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(800.), px(600.)));
        let mut rig = Rig { cx, host, view };
        rig.draw();
        rig
    }

    fn rig(cx: &mut TestAppContext) -> Rig<'_> {
        rig_with_library(cx, None)
    }

    /// C1 — mọi phím mặc định chạy đúng lệnh khi không có gì focus.
    #[gpui::test]
    fn default_keys_run_their_commands(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        let gpui_keys = [
            ("ctrl-k", "open_search"),
            ("space", "toggle_play"),
            ("left", "seek_backward"),
            ("right", "seek_forward"),
            ("alt-left", "prev_track"),
            ("alt-right", "next_track"),
            ("up", "volume_up"),
            ("down", "volume_down"),
            ("alt-p", "play_random"),
            ("g m", "go_to_library"),
            ("g p", "go_to_now_playing"),
        ];
        assert_eq!(gpui_keys.len(), DEFAULT_KEYBINDINGS.len());
        for (keys, name) in gpui_keys {
            rig.press(keys);
            let ran: Vec<&str> = rig.take_ran().into_iter().map(Command::name).collect();
            assert_eq!(ran, [name], "phím {keys}");
        }
        assert_eq!(rig.host.searches.get(), 1);
        assert_eq!(*rig.host.routes.borrow(), [Route::MyMusic, Route::NowPlaying]);
    }

    /// C2 — meta tính là ctrl.
    #[gpui::test]
    fn meta_counts_as_ctrl(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.press("cmd-k");
        assert_eq!(rig.take_ran(), [Command::OpenSearch]);
    }

    /// C3 — chuỗi quên sau 800 ms, theo đồng hồ của executor.
    #[gpui::test]
    fn sequence_times_out(cx: &mut TestAppContext) {
        let executor = cx.executor();
        let mut rig = rig(cx);
        rig.press("g");
        executor.advance_clock(Duration::from_millis(800));
        rig.press("m");
        assert!(rig.take_ran().is_empty());
        rig.press("g");
        executor.advance_clock(Duration::from_millis(799));
        rig.press("m");
        assert_eq!(rig.take_ran(), [Command::GoToLibrary]);
    }

    /// C4 (M4) — trong ô nhập chỉ Ctrl+K chạy (bản cũ ngược lại: Ctrl+K chết); chữ vào ô như thường.
    #[gpui::test]
    fn text_input_blocks_all_but_ctrl_k(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.focus(|view, cx| view.field.focus_handle(cx));
        rig.press("g m");
        rig.press("space");
        rig.press("left");
        assert!(rig.take_ran().is_empty());
        assert_eq!(rig.field_value(), "gm ");
        rig.press("ctrl-k");
        assert_eq!(rig.take_ran(), [Command::OpenSearch]);
    }

    /// C5 (M4) — dialog mở thì không phím toàn cục nào chạy, cả khi focus không ở ô nhập; đóng thì chạy lại.
    #[gpui::test]
    fn open_dialog_blocks_global_keys(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        let view = rig.view.clone();
        rig.cx.update(|window, cx| view.read(cx).dialog.clone().open(window, cx));
        rig.draw();
        // Mất focus giữa chừng (bấm vào chữ trong hộp) vẫn chặn: đây chính là lỗi của bản cũ.
        rig.cx.update(|window, cx| window.blur(cx));
        rig.draw();
        for keys in ["space", "ctrl-k", "up", "alt-p", "g m"] {
            rig.press(keys);
        }
        assert!(rig.take_ran().is_empty());
        rig.cx.update(|window, cx| view.read(cx).dialog.clone().close(window, cx));
        rig.draw();
        rig.press("space");
        assert_eq!(rig.take_ran(), [Command::TogglePlay]);
    }

    /// C6 — thành phần đang focus giữ phím của nó, phím khác vẫn là phím toàn cục.
    #[gpui::test]
    fn focused_widget_keeps_its_keys(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.focus(|view, _| view.owner.clone());
        rig.press("up");
        assert!(rig.take_ran().is_empty());
        assert_eq!(rig.view.read_with(rig.cx, |view, _| view.owner_actions.get()), 1);
        rig.press("left");
        assert_eq!(rig.take_ran(), [Command::SeekBackward]);
    }

    /// C7 — nút đến bằng Tab: Space, Enter là bấm nút.
    #[gpui::test]
    fn keys_click_a_button_reached_by_tab(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        let clicks = rig.view.read_with(rig.cx, |view, _| view.clicks.clone());
        rig.focus(|view, cx| view.field.focus_handle(cx));
        rig.press("tab");
        rig.press_with_release("space");
        assert_eq!(clicks.get(), 1);
        assert!(rig.take_ran().is_empty());
        rig.press_with_release("enter");
        assert_eq!(clicks.get(), 2);
    }

    /// C7 — nút đến bằng chuột: Space vẫn phát / dừng, không bấm lại nút (bấm Next xong Space không được Next tiếp).
    #[gpui::test]
    fn space_toggles_play_after_mouse_click_on_a_button(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        let clicks = rig.view.read_with(rig.cx, |view, _| view.clicks.clone());
        rig.cx.simulate_click(point(px(BUTTON_AT + 10.), px(BUTTON_AT + 10.)), Default::default());
        rig.draw();
        assert_eq!(clicks.get(), 1, "cú bấm phải trúng nút");
        rig.press_with_release("space");
        assert_eq!(clicks.get(), 1);
        assert_eq!(rig.take_ran(), [Command::TogglePlay]);
    }

    /// C8 — đổi phím trong cài đặt áp ngay ở phím kế; lệnh lạ, phím trùng trong tệp không làm hỏng dòng đúng (M5).
    #[gpui::test]
    fn custom_bindings_apply_live_and_survive_bad_lines(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        assert_eq!(rig.cx.update(|_, cx| key_label(Command::OpenSearch, cx)), Some("Ctrl+K".to_owned()));
        *rig.host.custom.borrow_mut() = Some(vec![
            KeyBinding::new("x", "launch_rocket"),
            KeyBinding::new("y", "toggle_play"),
            KeyBinding::new("y", "next_track"),
        ]);
        rig.press("space");
        assert!(rig.take_ran().is_empty(), "phím mặc định hết tác dụng khi đã có bảng riêng");
        rig.press("x");
        assert!(rig.take_ran().is_empty());
        rig.press("y");
        assert_eq!(rig.take_ran(), [Command::TogglePlay]);
        assert_eq!(rig.cx.update(|_, cx| key_label(Command::OpenSearch, cx)), None);
    }

    /// C9 (S21) — ba thao tác nhanh đều chạy.
    #[gpui::test]
    fn quick_actions_all_run(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        let host = rig.host.clone();
        rig.cx.update(|_, cx| {
            for command in QUICK_ACTIONS {
                run(command, cx);
            }
        });
        assert_eq!(rig.take_ran(), QUICK_ACTIONS);
        assert_eq!(*host.routes.borrow(), [Route::MyMusic, Route::NowPlaying]);
    }

    fn no_covers() -> Box<dyn CoverSink + Send> {
        Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("no covers in this test") })
    }

    fn add_song(library: &Library, path: &Path, title: &str) -> i64 {
        let (path, title) = (path.to_owned(), title.to_owned());
        library
            .writer()
            .transaction(move |conn| {
                let upsert = SongUpsert {
                    path,
                    folder_id: 1,
                    album_id: None,
                    title: Some(title),
                    artist: None,
                    album_artist: None,
                    duration_sec: 1.0,
                    track_no: None,
                    disc_no: None,
                    lyrics: None,
                    timeline: None,
                    cover_hash: None,
                    file_mtime: None,
                    file_size: None,
                    tags_read_at: None,
                };
                conn.execute("INSERT OR IGNORE INTO folders (id, name, path) VALUES (1, 'f', 'f')", [])
                    .map_err(queries::QueryError::from)?;
                Ok(queries::upsert_song(conn, &upsert)?.id)
            })
            .wait()
            .unwrap()
    }

    fn until(cx: &mut VisualTestContext, what: &str, condition: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !cx.update(|_, cx| condition(cx)) {
            assert!(Instant::now() < deadline, "hết giờ chờ: {what}");
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// C10..C13 (S21) — lệnh chữ làm thật: `/queue` thêm cuối, `/play` tìm rồi phát, chọn bài từ kết quả tìm giữ hàng đợi.
    #[gpui::test]
    fn cli_commands_do_real_work(cx: &mut TestAppContext) {
        let dir = TestDir::new();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let library = Library::open(config, no_covers()).unwrap();
        let wav = |name: &str| fixtures::write_sample(dir.path(), name, AudioKind::Wav).unwrap();
        let a = add_song(&library, &wav("alpha"), "Alpha");
        let b = add_song(&library, &wav("beta"), "Beta");
        let c = add_song(&library, &wav("gamma"), "Gamma");
        let (a, b, c) = (TrackRef::Library(a), TrackRef::Library(b), TrackRef::Library(c));

        let mut rig = rig_with_library(cx, Some(library));
        let run_text = |rig: &mut Rig<'_>, text: &str| {
            let command = cli::parse(text).unwrap();
            let task = rig.cx.update(|_, cx| run_cli(command, cx));
            let result = Rc::new(Cell::new(None));
            let out = result.clone();
            rig.cx.update(|_, cx| {
                cx.spawn(async move |_| out.set(Some(task.await))).detach();
            });
            let deadline = Instant::now() + Duration::from_secs(15);
            while result.get().is_none() {
                assert!(Instant::now() < deadline, "hết giờ chờ lệnh");
                rig.cx.run_until_parked();
                std::thread::sleep(Duration::from_millis(5));
            }
            result.get().unwrap()
        };
        let current = |rig: &mut Rig<'_>| rig.cx.update(|_, cx| player(cx).read(cx).current_track().cloned());
        let queued = |rig: &mut Rig<'_>| rig.cx.update(|_, cx| player(cx).read(cx).queue().queue().to_vec());

        // Danh sách trống: `/playall` báo, không im lặng.
        assert_eq!(run_text(&mut rig, "/playall"), CliOutcome::NoMatch);
        assert_eq!(run_text(&mut rig, "/play khong co bai nay"), CliOutcome::NoMatch);

        // `/queue` khi chưa có bài nào: phát bài đó.
        assert_eq!(run_text(&mut rig, "/queue beta"), CliOutcome::Done);
        assert_eq!(current(&mut rig), Some(b.clone()));
        // `/queue` lúc đang phát: nằm cuối hàng đợi, bài đang phát không đổi.
        assert_eq!(run_text(&mut rig, "/queue gamma"), CliOutcome::Done);
        assert_eq!(current(&mut rig), Some(b.clone()));
        assert_eq!(queued(&mut rig), std::slice::from_ref(&c));

        // Chọn bài trong kết quả tìm: phát ngay, hàng đợi còn nguyên (bản cũ thay hẳn hàng đợi bằng một bài).
        rig.cx.update(|_, cx| play_search_result(a.clone(), cx));
        until(rig.cx, "phát alpha", |cx| player(cx).read(cx).current_track() == Some(&a));
        assert_eq!(queued(&mut rig), std::slice::from_ref(&c));
        // Chọn lại đúng bài đang phát: phát / dừng.
        // `current_track` đổi trước khi engine nạp xong: chờ thật sự phát rồi mới bấm, kẻo bấm dừng thành không đổi gì.
        until(rig.cx, "alpha đang phát", |cx| player(cx).read(cx).is_playing());
        rig.cx.update(|_, cx| play_search_result(a.clone(), cx));
        until(rig.cx, "đã dừng", |cx| !player(cx).read(cx).is_playing());
        assert_eq!(current(&mut rig), Some(a.clone()));

        // `/play <tên>`: danh sách mới từ kết quả tìm.
        assert_eq!(run_text(&mut rig, "/play gamma"), CliOutcome::Done);
        assert_eq!(current(&mut rig), Some(c.clone()));

        // `/playall` về đầu danh sách hiện tại.
        assert_eq!(run_text(&mut rig, "/playall"), CliOutcome::Done);
        assert_eq!(current(&mut rig), Some(c));

        assert_eq!(run_text(&mut rig, "/pause"), CliOutcome::Done);
        assert_eq!(run_text(&mut rig, "/volume 40"), CliOutcome::Done);
        assert_eq!(run_text(&mut rig, "/help"), CliOutcome::ShowHelp);

        // `/theme` làm thật: đối số, và đổi qua lại khi không đối số.
        assert_eq!(run_text(&mut rig, "/theme light"), CliOutcome::Done);
        assert_eq!(rig.host.theme.get(), Some(ThemeChoice::Light));
        run_text(&mut rig, "/theme");
        assert_eq!(rig.host.theme.get(), Some(ThemeChoice::Dark));
        run_text(&mut rig, "/theme");
        assert_eq!(rig.host.theme.get(), Some(ThemeChoice::Light));
    }
}
