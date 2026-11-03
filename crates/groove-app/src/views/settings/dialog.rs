//! groove-gd1_24: hộp Settings (đặc tả 1.10), 800 px, cột trái bốn tab: Cài đặt chung, EQ, Phím tắt, Thư viện.
//!
//! Mở bằng `ShellEvent::OpenSettings` (nút ở đáy cột trái, hay lệnh). Luật của đặc tả:
//!
//! - **EQ và hai bước áp ngay** (nghe, thấy tác dụng liền). **Huỷ** khôi phục giá trị lúc mở; **OK** lưu phím tắt.
//! - **OK** soát JSON phím tắt (M5): sai thì nhảy về tab Phím tắt, hiện lỗi dưới ô, **không đóng**.
//! - **Bấm nền đóng, coi như Huỷ** (G-5; không còn nút ✕): Esc hay bấm nền đều trả bước tua, bước âm lượng, EQ đã áp tạm
//!   về lúc mở và bỏ nháp phím tắt; hộp phát hiện mình đã đóng mà chưa qua OK thì trả giá trị về. Không lặp M3 của bản
//!   cũ (bấm ra ngoài giữ EQ mà bỏ phím tắt): mọi thứ cùng về như lúc mở.
//! - Hộp chia header / body / footer có đường kẻ (`Dialog::divided`); body không đệm (`flush_body`) để khung cuộn chạm
//!   mép phải của hộp và hai đường kẻ, đệm ngang nằm trong nội dung cuộn (D-16).
//! - Phím toàn cục tự tắt khi hộp mở (`commands`), nên gõ vào ô JSON không kích hoạt lệnh.
//!
//! Hộp nằm trong cây của khung như bảng lệnh (`Slot::Settings`), dựng một lần ở `install`.

use groove_core::settings::{EqSettings, LayoutStyle, Settings, ThemeChoice, UiFont};
use groove_core::t;
use materiant::button::Button;
use materiant::effect::ripple::{RippleConfig, RippleExt};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, AnyWindowHandle, App, ClickEvent, Context, Entity, FocusHandle, IntoElement, KeyDownEvent,
    ParentElement, Render, Styled, Window, div, px,
};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::scroll::ScrollArea;
use materiant::style::ActiveTokens;
use materiant::tabs::InkBar;
use materiant::theme::ActiveTheme;

use super::eq_panel::EqPanel;
use super::general::GeneralPanel;
use super::keybindings::{Draft, KeybindingsPanel};
use super::roots::RootsPanel;
use crate::app::{self, ShellEvent, Slot};
use crate::library_ctl;
use crate::views::side_nav::{ACTIVE_BAR, NavInk};

/// Bề ngang hộp (đặc tả 1.10).
const DIALOG_WIDTH: f32 = 800.;
/// Cao vùng nội dung (đủ chỗ cho bảng EQ: thẻ bass, mười cột).
const BODY_HEIGHT: f32 = 470.;
/// Chừa cho tiêu đề, hàng nút, lề khi cửa sổ thấp hơn hộp.
const CHROME_HEIGHT: f32 = 170.;
/// Rộng cột tab (web: `w-40`) và cao một hàng (hàng cao đều để vạch trượt khỏi đo).
const TABS_WIDTH: f32 = 160.;
const TAB_HEIGHT: f32 = 40.;
/// Đệm của body: cột tab sát mép trái hộp (vạch màu nhấn chạm mép), đệm trên dưới `EDGE`; nội dung cuộn đệm `CONTENT_PADDING`
/// đều hai bên (≥ vùng bắt chuột 9 px của thanh cuộn, D-16), trên dưới `EDGE`.
const EDGE: f32 = 12.;
const CONTENT_PADDING: f32 = 16.;
/// Khe giữa cột tab và khung cuộn.
const TABS_GAP: f32 = 8.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Eq,
    Keybindings,
    Library,
}

impl SettingsTab {
    const ALL: [SettingsTab; 4] =
        [SettingsTab::General, SettingsTab::Eq, SettingsTab::Keybindings, SettingsTab::Library];

    fn label(self) -> &'static str {
        match self {
            SettingsTab::General => t!("settings.tab.general"),
            SettingsTab::Eq => t!("settings.tab.eq"),
            SettingsTab::Keybindings => t!("settings.tab.keybindings"),
            SettingsTab::Library => t!("settings.tab.library"),
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }
}

/// Giá trị lúc mở hộp, để Huỷ trả về.
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    theme: ThemeChoice,
    theme_preset: String,
    font: UiFont,
    layout_style: LayoutStyle,
    seek_step: u32,
    volume_step: u8,
    eq: EqSettings,
}

impl Snapshot {
    fn of(settings: &Settings) -> Snapshot {
        Snapshot {
            theme: settings.theme,
            theme_preset: settings.theme_preset.clone(),
            font: settings.font,
            layout_style: settings.layout_style,
            seek_step: settings.seek_step,
            volume_step: settings.volume_step,
            eq: settings.eq.clone(),
        }
    }
}

pub struct SettingsDialog {
    handle: DialogHandle,
    tab: SettingsTab,
    general: Entity<GeneralPanel>,
    eq: Entity<EqPanel>,
    keys: Entity<KeybindingsPanel>,
    roots: Entity<RootsPanel>,
    /// Điểm dừng Tab của từng mục ở cột tab (↑ ↓ dời và chọn).
    tab_focus: [FocusHandle; SettingsTab::ALL.len()],
    /// `Some` suốt lúc hộp mở và chưa qua OK / Huỷ.
    opened: Option<Snapshot>,
}

impl SettingsDialog {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> SettingsDialog {
        SettingsDialog {
            handle: DialogHandle::new(cx),
            tab: SettingsTab::General,
            general: cx.new(|cx| GeneralPanel::new(window, cx)),
            eq: cx.new(EqPanel::new),
            keys: cx.new(|cx| KeybindingsPanel::new(window, cx)),
            roots: cx.new(|_| RootsPanel::new()),
            tab_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            opened: None,
        }
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    #[cfg(test)]
    pub fn tab(&self) -> SettingsTab {
        self.tab
    }

    #[cfg(test)]
    pub fn keybindings(&self) -> &Entity<KeybindingsPanel> {
        &self.keys
    }

    #[cfg(test)]
    pub fn general(&self) -> &Entity<GeneralPanel> {
        &self.general
    }

    #[cfg(test)]
    pub fn eq(&self) -> &Entity<EqPanel> {
        &self.eq
    }

    #[cfg(test)]
    pub fn roots(&self) -> &Entity<RootsPanel> {
        &self.roots
    }

    /// Mở hộp: nhớ giá trị hiện có, nạp vào các tab, về tab đầu.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle.is_open() {
            return;
        }
        let settings = app::settings(cx).clone();
        self.opened = Some(Snapshot::of(&settings));
        self.tab = SettingsTab::General;
        self.general.update(cx, |general, cx| general.show(&settings, window, cx));
        let eq = settings.eq.clone();
        self.eq.update(cx, |panel, cx| panel.show(eq, cx));
        self.keys.update(cx, |keys, cx| keys.show(&settings, window, cx));
        let ctl = library_ctl::ctl(cx);
        self.roots.update(cx, |roots, cx| roots.bind(ctl, cx));
        self.handle.open(window, cx);
        cx.notify();
    }

    fn select_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            cx.notify();
        }
    }

    /// OK: lưu phím tắt rồi đóng. Phím tắt sai thì ở lại, nhảy về tab của nó.
    pub fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.keys.update(cx, |keys, cx| keys.check(cx));
        match draft {
            Err(()) => self.select_tab(SettingsTab::Keybindings, cx),
            Ok(draft) => {
                match draft {
                    Draft::Unchanged => {}
                    Draft::Defaults => app::update_settings(cx, |s| s.custom_keybindings = None),
                    Draft::Custom(list) => app::update_settings(cx, |s| s.custom_keybindings = Some(list)),
                }
                self.opened = None;
                self.handle.close(window, cx);
                cx.notify();
            }
        }
    }

    /// Huỷ: trả nền, bảng màu, font, kiểu chia vùng, bước tua, bước âm lượng, EQ về lúc mở rồi đóng. Phím tắt chưa lưu nên chỉ cần bỏ nháp.
    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(snapshot) = self.opened.take() {
            self.revert(snapshot, cx);
        }
        self.handle.close(window, cx);
        cx.notify();
    }

    fn revert(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        app::update_settings(cx, |s| {
            s.theme = snapshot.theme;
            s.theme_preset = snapshot.theme_preset;
            s.font = snapshot.font;
            s.layout_style = snapshot.layout_style;
            s.seek_step = snapshot.seek_step;
            s.volume_step = snapshot.volume_step;
        });
        // EQ đi qua player để engine nghe lại giá trị cũ; bảng EQ cũng về hiển thị giá trị đó.
        let eq = snapshot.eq;
        self.eq.update(cx, |panel, cx| panel.restore(eq, cx));
    }

    /// Cột tab dọc, cùng kiểu cột trái chính (`side_nav`): hàng phẳng có vạch trái 3 px; vạch màu nhấn của mục đang chọn là
    /// một `InkBar` trượt lên xuống khi đổi mục, nền `layer(N200)`, chữ thường; mục khác chữ `muted`, rê thì nền `layer(N100)`
    /// và vạch nhạt. Mỗi hàng một điểm dừng Tab, ↑ ↓ Home End dời và chọn, vòng focus khi đi bằng phím.
    fn tab_list(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let primary = cx.theme().colors().primary;
        let ring_color = cx.focus_ring_color();
        let items: Vec<AnyElement> = SettingsTab::ALL
            .into_iter()
            .enumerate()
            .map(|(ix, tab)| {
                let selected = tab == self.tab;
                let ink = NavInk::resolve(selected, cx);
                let ring = materiant::style::focus_visible(&self.tab_focus[ix], window, cx);
                let go = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>, to: SettingsTab| {
                    this.select_tab(to, cx);
                    window.focus(&this.tab_focus[to.index()], cx);
                };
                div()
                    .id(("settings-tab", ix))
                    .track_focus(&self.tab_focus[ix])
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(TAB_HEIGHT))
                    .pl(px(13.))
                    .pr_3()
                    .border_l(px(ACTIVE_BAR))
                    .border_color(ink.bar)
                    .cursor_pointer()
                    .text_sm()
                    .text_color(ink.label)
                    .when(selected, |d| d.bg(ink.active_bg))
                    .when(!selected, |d| {
                        d.hover(move |s| s.bg(ink.hover_bg).border_color(ink.hover_bar).text_color(ink.hover))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| go(this, window, cx, tab)))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        let last = SettingsTab::ALL.len() - 1;
                        let to = match event.keystroke.key.as_str() {
                            "up" => ix.saturating_sub(1),
                            "down" => (ix + 1).min(last),
                            "home" => 0,
                            "end" => last,
                            _ => return,
                        };
                        cx.stop_propagation();
                        go(this, window, cx, SettingsTab::ALL[to]);
                    }))
                    .child(div().flex_1().min_w_0().truncate().child(tab.label()))
                    .when(ring, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(2.))
                                .bottom(px(2.))
                                .left(px(2.))
                                .right(px(2.))
                                .border_2()
                                .rounded(px(4.))
                                .border_color(ring_color),
                        )
                    })
                    .ripple(RippleConfig::default())
                    .into_any_element()
            })
            .collect();
        let ink = InkBar::new("settings-tab-ink", self.tab.index() as f32 * TAB_HEIGHT, TAB_HEIGHT)
            .vertical()
            .thickness(ACTIVE_BAR)
            .color(primary)
            .build(window, cx)
            .debug_selector(|| "settings-tab-ink".into());
        div()
            .flex_none()
            .w(px(TABS_WIDTH))
            .py(px(EDGE))
            .debug_selector(|| "settings-tabs".into())
            .child(div().relative().flex().flex_col().children(items).child(ink))
    }

    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let height = BODY_HEIGHT.min((f32::from(window.viewport_size().height) - CHROME_HEIGHT).max(120.));
        let panel = match self.tab {
            SettingsTab::General => self.general.clone().into_any_element(),
            SettingsTab::Eq => self.eq.clone().into_any_element(),
            SettingsTab::Keybindings => self.keys.clone().into_any_element(),
            SettingsTab::Library => self.roots.clone().into_any_element(),
        };
        let tabs = self.tab_list(window, cx);
        div()
            .flex()
            .gap(px(TABS_GAP))
            .h(px(height))
            .child(tabs)
            // Kiểu lưới: đường kẻ giữa danh sách tab và nội dung (`views::layout`).
            .children(crate::views::layout::vline(cx))
            .child(ScrollArea::new(("settings-body", self.tab.index())).vertical().flex_1().min_w_0().h_full().child(
                div().px(px(CONTENT_PADDING)).py(px(EDGE)).debug_selector(|| "settings-content".into()).child(panel),
            ))
            .into_any_element()
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Đóng bằng Esc hay bấm nền không đi qua `cancel`: phát hiện ở đây, trả giá trị về sau khung này (G-5).
        if !self.handle.is_open()
            && let Some(snapshot) = self.opened.take()
        {
            cx.defer_in(window, move |this, _, cx| this.revert(snapshot, cx));
        }
        let actions: [AnyElement; 2] = [
            Button::new("settings-cancel", t!("common.cancel"))
                .text()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel(window, cx)))
                .into_any_element(),
            Button::new("settings-ok", t!("common.ok"))
                .primary()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.confirm(window, cx)))
                .into_any_element(),
        ];
        Dialog::new("settings-dialog", &self.handle)
            .max_width(px(DIALOG_WIDTH))
            .title(t!("settings.title"))
            .title_weight(materiant::gpui_kit::FontWeight::NORMAL)
            .divided(true)
            .flush_body(true)
            .actions(actions)
            .child(self.body(window, cx))
    }
}

/// Dựng hộp vào `Slot::Settings` và mở khi `ShellEvent::OpenSettings`. Gọi một lần trong `app::run`, sau `library_page::install`.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    let result = window.update(cx, |_, window, cx| {
        let dialog = cx.new(|cx| SettingsDialog::new(window, cx));
        app::install_slot(Slot::Settings, dialog.clone(), cx);
        let shell = app::shell(cx);
        window
            .subscribe(&shell, cx, move |_, event: &ShellEvent, window, cx| {
                if *event == ShellEvent::OpenSettings {
                    dialog.update(cx, |dialog, cx| dialog.open(window, cx));
                }
            })
            .detach();
    });
    if let Err(err) = result {
        log::error!("cannot install the settings dialog: {err}");
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::{TestAppContext, VisualTestContext, point, size};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::model::TrackRef;
    use groove_core::settings::{KeyBinding, default_keybindings};
    use groove_library::covers::CoverStore;
    use groove_library::db::queries;
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::indexer::Indexer;
    use groove_library::store::{Library, LibraryConfig};
    use materiant::gpui_kit::SharedString;
    use materiant::gpui_kit::base::Root;
    use tempfile::TempDir;

    use super::*;
    use crate::library_ctl::{LibraryCtl, PickKind, Picker};
    use crate::player::{self, PlayerDeps};
    use crate::views::settings::roots::RootsState;

    const TIMEOUT: Duration = Duration::from_secs(20);
    /// Dưới mức này là khung im lặng (engine chưa kịp nạp ring lúc máy bận), không phải kết quả của EQ: bỏ qua.
    const SOUND: f32 = 0.5;

    /// Hộp trong cửa sổ ảo, cài đặt mặc định không lưu đĩa, player với `FakeOutput`: không phát tiếng, không DB thật.
    struct Rig {
        dir: TempDir,
        fake: FakeOutput,
        dialog: Entity<SettingsDialog>,
    }

    fn rig(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
        // Engine chạy luồng thật, đánh thức foreground task từ luồng khác.
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let fake = FakeOutput::new(48_000, 2);
        cx.update(|cx| {
            materiant::init(cx);
            app::install_settings_for_test(Settings::default(), cx);
            install_player(&fake, cx);
        });
        let window = cx.add_window(|window, cx| {
            let dialog = cx.new(|cx| SettingsDialog::new(window, cx));
            Root::new(dialog, window, cx)
        });
        let dialog = window.update(cx, |root, _, _| root.view().clone().downcast::<SettingsDialog>().unwrap()).unwrap();
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(1000.), px(700.)));
        (Rig { dir, fake, dialog }, cx)
    }

    /// Player thật nhưng đầu ra là `FakeOutput`, không thư viện, không `session.json`.
    fn install_player(fake: &FakeOutput, cx: &mut App) {
        let deps = PlayerDeps {
            library: None,
            output: Box::new(fake.clone()),
            options: EngineOptions {
                stall_timeout: Duration::from_secs(3600),
                reopen_interval: Duration::from_millis(1),
                reopen_attempts: 1,
                ..EngineOptions::default()
            },
            session_file: None,
        };
        player::install_for_test(deps, cx);
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn open(rig: &Rig, cx: &mut VisualTestContext) {
        rig.dialog.update_in(cx, |dialog, window, cx| dialog.open(window, cx));
        draw(cx);
    }

    fn confirm(rig: &Rig, cx: &mut VisualTestContext) {
        rig.dialog.update_in(cx, |dialog, window, cx| dialog.confirm(window, cx));
        draw(cx);
    }

    fn cancel(rig: &Rig, cx: &mut VisualTestContext) {
        rig.dialog.update_in(cx, |dialog, window, cx| dialog.cancel(window, cx));
        draw(cx);
    }

    fn settings(cx: &mut VisualTestContext) -> Settings {
        cx.update(|_, cx| app::settings(cx).clone())
    }

    fn is_open(rig: &Rig, cx: &VisualTestContext) -> bool {
        rig.dialog.read_with(cx, |dialog, _| dialog.is_open())
    }

    fn type_seek(rig: &Rig, text: &'static str, cx: &mut VisualTestContext) {
        rig.dialog.update_in(cx, |d, window, cx| d.general().update(cx, |g, cx| g.type_seek_step(text, window, cx)));
        draw(cx);
    }

    fn set_keys_text(rig: &Rig, text: impl Into<SharedString>, cx: &mut VisualTestContext) {
        let text = text.into();
        rig.dialog.update_in(cx, |d, window, cx| d.keybindings().update(cx, |k, cx| k.set_text(text, window, cx)));
    }

    fn keys_text(rig: &Rig, cx: &VisualTestContext) -> String {
        rig.dialog.read_with(cx, |d, cx| d.keybindings().read(cx).text(cx).to_string())
    }

    fn keys_have_error(rig: &Rig, cx: &VisualTestContext) -> bool {
        rig.dialog.read_with(cx, |d, cx| d.keybindings().read(cx).has_error())
    }

    fn eq_panel(rig: &Rig, cx: &VisualTestContext) -> Entity<EqPanel> {
        rig.dialog.read_with(cx, |dialog, _| dialog.eq().clone())
    }

    /// Sóng sin 440 Hz dài 30 giây, đủ lâu để chạy cả test dù kéo mẫu nhanh hơn thời gian thật.
    fn write_tone(dir: &Path) -> PathBuf {
        let rate = 48_000u32;
        let samples = rate * 30;
        let mut data = Vec::with_capacity(samples as usize * 2);
        for i in 0..samples {
            let phase = i as f32 * 440. * std::f32::consts::TAU / rate as f32;
            data.extend_from_slice(&((phase.sin() * 0.5 * f32::from(i16::MAX)) as i16).to_le_bytes());
        }
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        let path = dir.join("tone.wav");
        fs::write(&path, wav).unwrap();
        path
    }

    /// Độ lớn một khung đầu ra; kéo mẫu mỗi lần gọi nên đồng thời là đồng hồ của engine.
    fn frame_loudness(fake: &FakeOutput) -> f32 {
        fake.pull(480).map_or(0.0, |samples| samples.iter().map(|s| s.abs()).sum())
    }

    /// Kéo mẫu (đúng nhịp thật) tới khi độ lớn thoả `ok`, trả giá trị đó. EQ áp ở decoder trước ring nên đổi EQ trễ cỡ 200 ms: chờ theo điều
    /// kiện, không ngủ cố định.
    fn loudness_until(rig: &Rig, cx: &mut VisualTestContext, what: &str, ok: impl Fn(f32) -> bool) -> f32 {
        let deadline = Instant::now() + TIMEOUT;
        let mut seen: Vec<i32> = Vec::new();
        loop {
            cx.run_until_parked();
            let loudness = frame_loudness(&rig.fake);
            if ok(loudness) {
                return loudness;
            }
            seen.push(loudness as i32);
            if seen.len() > 400 {
                seen.remove(0);
            }
            assert!(Instant::now() < deadline, "timed out: {what} (last loudness {:?})", seen);
            // Kéo đúng nhịp thời gian thật (10 ms âm thanh mỗi 10 ms): kéo nhanh hơn thì máy bận làm engine nạp ring không kịp,
            // còn máy rảnh thì ngốn hết đoạn mẫu trước khi EQ kịp tới.
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Phát sóng sin rồi trả độ lớn ổn định. Bỏ nửa giây đầu: âm lượng nội suy từ 0 nên các khung đầu nhỏ hơn mức thật (máy
    /// bận thì đoạn nội suy kéo dài hơn), lấy làm mốc sẽ lệch. Mốc là trung bình mười khung, vì một khung dao động vài phần
    /// trăm theo pha của sóng.
    fn play_tone(rig: &Rig, cx: &mut VisualTestContext) -> f32 {
        let track = TrackRef::from_path(write_tone(rig.dir.path()));
        cx.update(|_, cx| player::player(cx).update(cx, |p, cx| p.play_track(track, cx)));
        let frame = |cx: &mut VisualTestContext| {
            let loudness = loudness_until(rig, cx, "khung có tiếng", |l| l > SOUND);
            std::thread::sleep(Duration::from_millis(10));
            loudness
        };
        for _ in 0..50 {
            frame(cx);
        }
        let frames: Vec<f32> = (0..10).map(|_| frame(cx)).collect();
        frames.iter().sum::<f32>() / frames.len() as f32
    }

    /// C1 — EQ áp ngay xuống engine, không chỉ lưu: kéo mọi dải xuống -10 dB làm tiếng nhỏ đi rõ; Huỷ trả tiếng về như cũ.
    /// Đi đúng đường của thanh trượt (`EqPanel::apply`), tới `Player::update_eq`, tới DSP, tới `FakeOutput`.
    #[gpui::test]
    fn c1_eq_reaches_the_engine_and_cancel_restores_it(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        let before = play_tone(&rig, cx);

        open(&rig, cx);
        let panel = eq_panel(&rig, cx);
        panel.update(cx, |panel, cx| {
            for band in 0..10 {
                panel.apply(move |eq| eq.set_band(band, -10.0), cx);
            }
        });
        assert_eq!(panel.read_with(cx, |p, _| p.eq().gains), [-10.0; 10], "bảng hiển thị đúng giá trị vừa đặt");
        loudness_until(&rig, cx, "tiếng nhỏ đi", |l| l > SOUND && l < before * 0.6);

        cancel(&rig, cx);
        assert!(!is_open(&rig, cx));
        assert_eq!(panel.read_with(cx, |p, _| p.eq().gains), [0.0; 10], "Huỷ trả bảng về lúc mở");
        loudness_until(&rig, cx, "tiếng trở lại", |l| l > before * 0.85);
    }

    /// C1b — chọn mẫu cài sẵn đi cùng đường: `Vocal` đổi tiếng của engine, `Flat` trả về.
    #[gpui::test]
    fn c1b_preset_reaches_the_engine(cx: &mut TestAppContext) {
        use groove_core::settings::EqPreset;

        let (rig, cx) = rig(cx);
        let flat = play_tone(&rig, cx);
        open(&rig, cx);
        let panel = eq_panel(&rig, cx);
        // Không còn preamp (G-7): dải không chạm 440 Hz thì 440 Hz giữ nguyên, nên chọn mẫu có dải 250 / 500 Hz dương.
        panel.update(cx, |panel, cx| panel.apply(|eq| eq.select_preset(EqPreset::Vocal), cx));
        assert_eq!(panel.read_with(cx, |p, _| p.eq().preset), EqPreset::Vocal);
        loudness_until(&rig, cx, "tiếng đổi theo preset", |l| l > SOUND && (l - flat).abs() > flat * 0.1);
        panel.update(cx, |panel, cx| panel.apply(|eq| eq.select_preset(EqPreset::Flat), cx));
        loudness_until(&rig, cx, "về phẳng", |l| l > SOUND && (l - flat).abs() < flat * 0.15);
    }

    /// C2 — mở hộp nạp đúng giá trị đang có vào các ô, về tab đầu.
    #[gpui::test]
    fn c2_open_shows_the_current_values(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        cx.update(|_, cx| {
            app::update_settings(cx, |s| {
                s.seek_step = 12;
                s.custom_keybindings = Some(vec![KeyBinding { key: "x".into(), command: "toggle_play".into() }]);
                s.eq.set_band(2, 4.5);
            })
        });
        open(&rig, cx);
        assert!(is_open(&rig, cx));
        assert_eq!(rig.dialog.read_with(cx, |d, _| d.tab()), SettingsTab::General);
        let seek = rig.dialog.read_with(cx, |d, cx| d.general().read(cx).seek_step_value(cx));
        assert_eq!(seek, Some(12.0));
        let text = keys_text(&rig, cx);
        assert!(text.contains("\"key\": \"x\"") && !text.contains("ctrl+k"), "{text}");
        assert_eq!(eq_panel(&rig, cx).read_with(cx, |p, _| p.eq().gains[2]), 4.5);
    }

    /// C3 — gõ vào ô bước áp ngay; ô trống hay 0 chưa phải giá trị nên bỏ qua.
    #[gpui::test]
    fn c3_steps_apply_while_typing_and_ignore_invalid_input(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        type_seek(&rig, "30", cx);
        assert_eq!(settings(cx).seek_step, 30);
        type_seek(&rig, "", cx);
        assert_eq!(settings(cx).seek_step, 30, "ô trống không đổi giá trị");
        type_seek(&rig, "0", cx);
        assert_eq!(settings(cx).seek_step, 30, "0 ngoài khoảng, không áp");
        type_seek(&rig, "7", cx);
        assert_eq!(settings(cx).seek_step, 7);
    }

    /// C4 — Huỷ khôi phục nền, bảng màu, font, bước tua, EQ và bỏ nháp phím tắt (đặc tả 1.10).
    #[gpui::test]
    fn c4_cancel_restores_everything_and_drops_the_key_draft(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        type_seek(&rig, "45", cx);
        // Ô chọn font gọi đúng lời này (`general.rs`).
        cx.update(|_, cx| crate::app::update_settings(cx, |s| s.font = UiFont::BeVietnamPro));
        cx.update(|_, cx| crate::app::update_settings(cx, |s| s.layout_style = LayoutStyle::Grid));
        // Ô Nền và thẻ bảng màu cũng vậy.
        cx.update(|_, cx| {
            crate::app::update_settings(cx, |s| {
                s.theme = ThemeChoice::Light;
                s.theme_preset = "rose-red".into();
            })
        });
        set_keys_text(&rig, "[]", cx);
        eq_panel(&rig, cx).update(cx, |panel, cx| panel.apply(|eq| eq.set_band(0, 6.0), cx));
        draw(cx);
        assert_eq!(settings(cx).seek_step, 45);
        let panel = eq_panel(&rig, cx);
        assert_eq!(panel.read_with(cx, |p, _| p.eq().gains[0]), 6.0);

        cancel(&rig, cx);
        let after = settings(cx);
        assert_eq!(after.seek_step, 5);
        assert_eq!(after.font, UiFont::MonaSans, "font về như lúc mở");
        assert_eq!(after.layout_style, LayoutStyle::Cards, "kiểu chia vùng về như lúc mở");
        assert_eq!(
            (after.theme, after.theme_preset.as_str()),
            (ThemeChoice::Dark, "default"),
            "nền, bảng màu về như lúc mở"
        );
        assert_eq!(panel.read_with(cx, |p, _| p.eq().gains[0]), 0.0);
        assert_eq!(after.custom_keybindings, None, "nháp phím tắt không bao giờ được lưu");
        assert!(!is_open(&rig, cx));
    }

    /// Sửa bước tua, EQ, nháp phím tắt như người dùng đang dở tay.
    fn edit_everything(rig: &Rig, cx: &mut VisualTestContext) {
        type_seek(rig, "45", cx);
        eq_panel(rig, cx).update(cx, |panel, cx| {
            panel.apply(|eq| eq.set_band(3, 5.0), cx);
            panel.apply(|eq| eq.set_bass_boost(4.0), cx);
        });
        set_keys_text(rig, "[ nháp ]", cx);
        draw(cx);
        assert_eq!(settings(cx).seek_step, 45);
        assert_eq!(eq_panel(rig, cx).read_with(cx, |p, _| p.eq().gains[3]), 5.0);
    }

    /// Mọi thứ về như lúc mở: bước tua, EQ (bảng lẫn cài đặt), nháp phím tắt không được lưu và mở lại không còn.
    fn assert_everything_reverted(rig: &Rig, cx: &mut VisualTestContext, how: &str) {
        assert!(!is_open(rig, cx), "{how}: đóng");
        let after = settings(cx);
        assert_eq!(after.seek_step, 5, "{how}: trả bước tua");
        assert_eq!(after.eq, EqSettings::default(), "{how}: trả EQ đã lưu");
        let eq = eq_panel(rig, cx).read_with(cx, |p, _| p.eq().clone());
        assert_eq!(eq, EqSettings::default(), "{how}: bảng EQ về lúc mở");
        assert_eq!(after.custom_keybindings, None, "{how}: không lưu nháp phím tắt");
        open(rig, cx);
        assert!(keys_text(rig, cx).contains("ctrl+k"), "{how}: mở lại thì nháp cũ không còn");
    }

    /// C5 (G-5) — bấm lớp phủ đóng hộp, coi như Huỷ: trả mọi thay đổi chưa OK, kể cả EQ đã áp tạm.
    #[gpui::test]
    fn c5_backdrop_click_closes_as_cancel(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        edit_everything(&rig, cx);
        cx.simulate_click(point(px(4.), px(4.)), Default::default());
        draw(cx);
        draw(cx);
        assert_everything_reverted(&rig, cx, "bấm nền");
    }

    /// Bấm trong hộp (vùng trống của body) không đóng.
    #[gpui::test]
    fn click_inside_the_panel_keeps_it_open(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        let footer = cx.debug_bounds("dialog-footer").expect("có footer");
        cx.simulate_click(point(footer.origin.x + px(20.), footer.center().y), Default::default());
        draw(cx);
        assert!(is_open(&rig, cx));
    }

    /// C5b — Esc coi như Huỷ, như bấm nền.
    #[gpui::test]
    fn c5b_escape_cancels(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        edit_everything(&rig, cx);
        cx.simulate_keystrokes("escape");
        draw(cx);
        draw(cx);
        assert_everything_reverted(&rig, cx, "Esc");
    }

    /// Cột tab như cột trái chính: vạch 3 px của mục đang chọn trượt tới hàng mới khi đổi tab (không nhảy), ↓ dời và chọn.
    #[gpui::test]
    fn tab_ink_slides_to_the_new_row(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        let ink = |cx: &mut VisualTestContext| cx.debug_bounds("settings-tab-ink").expect("vạch đã vẽ");
        let first = ink(cx);
        assert_eq!((first.size.width, first.size.height), (px(ACTIVE_BAR), px(TAB_HEIGHT)));

        rig.dialog.update_in(cx, |d, window, cx| {
            window.focus(&d.tab_focus[0], cx);
        });
        cx.simulate_keystrokes("down");
        assert_eq!(rig.dialog.read_with(cx, |d, _| d.tab()), SettingsTab::Eq);
        draw(cx);
        cx.executor().advance_clock(Duration::from_millis(120));
        draw(cx);
        let mid = ink(cx).origin.y;
        assert!(mid > first.origin.y && mid < first.origin.y + px(TAB_HEIGHT), "đang trượt: {mid:?}");
        cx.executor().advance_clock(Duration::from_millis(400));
        draw(cx);
        draw(cx);
        assert_eq!(ink(cx).origin.y, first.origin.y + px(TAB_HEIGHT));

        // End tới mục cuối (Thư viện, hàng thứ tư): vạch dừng đúng hàng đó.
        cx.simulate_keystrokes("end");
        assert_eq!(rig.dialog.read_with(cx, |d, _| d.tab()), SettingsTab::Library);
        draw(cx);
        cx.executor().advance_clock(Duration::from_millis(400));
        draw(cx);
        draw(cx);
        assert_eq!(ink(cx).origin.y, first.origin.y + px(TAB_HEIGHT * 3.));
    }

    /// Bố cục: header / body / footer chồng sát nhau, cùng bề ngang; cột tab sát mép trái của hộp; khung cuộn chạm mép phải
    /// của hộp và hai đường kẻ, nội dung đệm đều 16 px hai bên trong khung cuộn (D-16).
    #[gpui::test]
    fn layout_has_divided_sections_and_flush_scroll(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        let header = cx.debug_bounds("dialog-header").expect("có header");
        let body = cx.debug_bounds("dialog-body").expect("có body");
        let footer = cx.debug_bounds("dialog-footer").expect("có footer");
        assert_eq!(header.bottom(), body.origin.y);
        assert_eq!(body.bottom(), footer.origin.y);
        assert_eq!(f32::from(header.size.width), DIALOG_WIDTH);
        let tabs = cx.debug_bounds("settings-tabs").expect("có cột tab");
        assert_eq!(tabs.origin.x, body.origin.x, "cột tab sát mép trái, không đệm");
        assert_eq!(tabs.origin.y, body.origin.y, "cột tab đệm trên `EDGE` bên trong");
        let content = cx.debug_bounds("settings-content").expect("có nội dung");
        assert_eq!(content.right(), body.right(), "khung cuộn chạm mép phải của hộp");
        assert_eq!(content.origin.y, body.origin.y, "nội dung cuộn bắt đầu ngay dưới đường kẻ");
        let _ = rig;
    }

    /// C6 (M5) — OK với JSON sai: nhảy về tab Phím tắt, báo lỗi, không đóng, không lưu gì.
    #[gpui::test]
    fn c6_ok_with_bad_json_jumps_to_the_keys_tab_and_stays(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        for text in ["[{\"key\": ", "{}", "[{\"key\": \"a\", \"command\": \"nope\"}]", "[{\"key\": \"a\"}]"] {
            rig.dialog.update_in(cx, |d, _, cx| d.select_tab(SettingsTab::General, cx));
            set_keys_text(&rig, text, cx);
            confirm(&rig, cx);
            assert!(is_open(&rig, cx), "{text}: không đóng");
            assert_eq!(rig.dialog.read_with(cx, |d, _| d.tab()), SettingsTab::Keybindings, "{text}: nhảy về tab phím");
            assert!(keys_have_error(&rig, cx), "{text}: có lỗi dưới ô");
            assert_eq!(settings(cx).custom_keybindings, None);
        }
        // Gõ tiếp thì lời báo cũ biến mất.
        rig.dialog.update_in(cx, |d, window, cx| d.keybindings().update(cx, |k, cx| k.type_text("[]", window, cx)));
        draw(cx);
        assert!(!keys_have_error(&rig, cx));
    }

    /// C7 — OK với phím hợp lệ: lưu, đóng, `Settings::keybindings` trả đúng danh sách; trả về mặc định thì ghi `None`;
    /// không đổi gì thì không đụng tới cài đặt.
    #[gpui::test]
    fn c7_ok_saves_valid_keybindings_and_closes(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        let custom = vec![
            KeyBinding { key: "ctrl+j".into(), command: "open_search".into() },
            KeyBinding { key: "g m".into(), command: "go_to_library".into() },
        ];
        open(&rig, cx);
        set_keys_text(&rig, groove_core::keymap::to_json(&custom), cx);
        confirm(&rig, cx);
        assert!(!is_open(&rig, cx));
        assert_eq!(settings(cx).custom_keybindings, Some(custom.clone()));
        assert_eq!(settings(cx).keybindings(), custom);

        // Mở lại, không sửa gì, OK: giữ nguyên.
        open(&rig, cx);
        confirm(&rig, cx);
        assert_eq!(settings(cx).custom_keybindings, Some(custom));

        // Đặt lại mặc định rồi OK: về `None` để phím mới của bản sau tới người dùng.
        open(&rig, cx);
        set_keys_text(&rig, groove_core::keymap::to_json(&default_keybindings()), cx);
        confirm(&rig, cx);
        assert_eq!(settings(cx).custom_keybindings, None);
        assert!(!is_open(&rig, cx));
    }

    /// C9 — nối vào khung: bấm nút Settings ở đáy cột trái phát `OpenSettings`, hộp trong `Slot::Settings` mở, Esc đóng.
    #[gpui::test]
    fn c9_the_settings_button_opens_the_dialog_in_its_slot(cx: &mut TestAppContext) {
        use crate::app::Shell;
        use crate::route::Route;

        cx.executor().allow_parking();
        cx.update(|cx| {
            materiant::init(cx);
            app::install_settings_for_test(Settings::default(), cx);
            install_player(&FakeOutput::new(48_000, 2), cx);
        });
        let window = cx.add_window(|window, cx| {
            let shell = cx.new(|cx| Shell::new(Route::MyMusic, cx));
            Root::new(shell, window, cx)
        });
        let shell = window.update(cx, |root, _, _| root.view().clone().downcast::<Shell>().unwrap()).unwrap();
        cx.update(|cx| app::set_shell_for_test(shell.clone(), cx));
        cx.update(|cx| install(window.into(), cx));
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(size(px(800.), px(600.)));
        draw(cx);

        let dialog = shell
            .read_with(cx, |shell, _| shell.slot(Slot::Settings).cloned())
            .expect("install đặt hộp vào slot")
            .downcast::<SettingsDialog>()
            .expect("slot giữ SettingsDialog");
        assert!(!dialog.read_with(cx, |d, _| d.is_open()));

        // Nút Settings ở đáy cột trái, phía trên thanh phát (cao 90 px).
        cx.simulate_click(point(px(100.), px(600. - 90. - 20.)), Default::default());
        draw(cx);
        assert!(dialog.read_with(cx, |d, _| d.is_open()), "bấm nút mở hộp");

        cx.simulate_keystrokes("escape");
        draw(cx);
        assert!(!dialog.read_with(cx, |d, _| d.is_open()));
    }

    /// OK không đụng EQ và bước đã áp: chúng đã lưu từ lúc kéo.
    #[gpui::test]
    fn ok_keeps_the_live_changes(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        type_seek(&rig, "20", cx);
        eq_panel(&rig, cx).update(cx, |panel, cx| panel.apply(|eq| eq.set_bass_boost(4.0), cx));
        confirm(&rig, cx);
        assert_eq!(settings(cx).seek_step, 20);
        assert_eq!(eq_panel(&rig, cx).read_with(cx, |p, _| p.eq().bass_boost), 4.0);
    }

    // ---- Tab Thư viện: qua `LibraryCtl` thật với thư viện tạm và hộp chọn giả ----

    struct Env {
        dir: TempDir,
        library: Library,
        covers: CoverStore,
    }

    fn env() -> Env {
        let dir = TempDir::new().unwrap();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let covers = CoverStore::new(dir.path().join("covers"));
        let library = Library::open(config, Box::new(CoverStore::new(dir.path().join("covers")))).unwrap();
        Env { dir, library, covers }
    }

    impl Env {
        /// Thư mục nhạc có một tệp, đã nhập.
        fn music(&self, name: &str) -> PathBuf {
            let root = self.dir.path().join(name);
            fs::create_dir_all(&root).unwrap();
            fixtures::write_sample(&root, "song", AudioKind::Wav).unwrap();
            Indexer::new(self.library.clone(), self.covers.clone()).import(root.clone()).wait().unwrap();
            root
        }

        fn roots(&self) -> Vec<PathBuf> {
            self.library.watched_roots().wait().unwrap()
        }

        fn songs(&self) -> usize {
            self.library.read(|conn| Ok(queries::song_file_states(conn)?.len())).wait().unwrap()
        }
    }

    fn picker(answer: Option<PathBuf>, asked: Rc<RefCell<Vec<PickKind>>>) -> Picker {
        Rc::new(move |kind, _| {
            asked.borrow_mut().push(kind);
            let answer = answer.clone();
            Box::pin(async move { answer })
        })
    }

    fn wait_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out: {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// C8 — tab Thư viện liệt kê gốc từ DB, Thêm đi qua `Picker` rồi nhập và danh sách đọc lại, Bỏ hỏi lại rồi chỉ xoá bài,
    /// không đụng tệp.
    #[gpui::test]
    fn c8_roots_tab_lists_adds_and_forgets_roots(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        let env = env();
        let first = env.music("first");
        let second = env.dir.path().join("second");
        fs::create_dir_all(&second).unwrap();
        fixtures::write_sample(&second, "other", AudioKind::Wav).unwrap();

        let asked: Rc<RefCell<Vec<PickKind>>> = Rc::default();
        let (library, covers) = (env.library.clone(), env.covers.clone());
        let pick = picker(Some(second.clone()), asked.clone());
        let ctl = cx.update(|_, cx| cx.new(|cx| LibraryCtl::new(Some(library), Some(covers), pick, None, cx)));
        let roots = rig.dialog.read_with(cx, |d, _| d.roots().clone());
        roots.update(cx, |panel, cx| panel.bind(Some(ctl.clone()), cx));
        wait_until(cx, "danh sách đầu tiên", |cx| {
            roots.read_with(cx, |p, _| matches!(p.state(), RootsState::Ready(_)))
        });
        assert_eq!(roots.read_with(cx, |p, _| p.roots().to_vec()), vec![first.clone()]);

        // Thêm: hộp chọn thư mục, nhập xong thì danh sách có hai gốc.
        roots.update(cx, |panel, cx| panel.add(cx));
        wait_until(cx, "gốc mới hiện", |cx| roots.read_with(cx, |p, _| p.roots().len() == 2));
        assert_eq!(*asked.borrow(), [PickKind::Folder]);
        assert_eq!(env.songs(), 2);

        // Bỏ: lần bấm đầu chỉ hỏi lại.
        roots.update(cx, |panel, cx| panel.remove(&first, cx));
        assert_eq!(env.songs(), 2, "bấm lần đầu chưa xoá gì");
        assert_eq!(roots.read_with(cx, |p, _| p.roots().len()), 2);
        roots.update(cx, |panel, cx| panel.remove(&first, cx));
        assert_eq!(roots.read_with(cx, |p, _| p.roots().to_vec()), vec![second.clone()], "gỡ khỏi danh sách ngay");
        wait_until(cx, "bài bị gỡ khỏi thư viện", |_| env.songs() == 1);
        assert_eq!(env.roots(), [second]);
        assert!(first.join("song.wav").exists(), "tệp trên đĩa vẫn còn");
    }

    /// Không có thư viện thì tab báo không liệt kê được, các nút không làm gì và không panic.
    #[gpui::test]
    fn roots_tab_without_a_library_says_so(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        open(&rig, cx);
        let roots = rig.dialog.read_with(cx, |d, _| d.roots().clone());
        assert_eq!(roots.read_with(cx, |p, _| p.state().clone()), RootsState::Unavailable);
        roots.update(cx, |panel, cx| {
            panel.add(cx);
            panel.rescan(cx);
        });
        rig.dialog.update_in(cx, |d, _, cx| d.select_tab(SettingsTab::Library, cx));
        draw(cx);
    }

    /// Vẽ được cả bốn tab ở cửa sổ nhỏ nhất, không panic.
    #[gpui::test]
    fn every_tab_draws_in_the_smallest_window(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx);
        cx.simulate_resize(size(px(560.), px(360.)));
        open(&rig, cx);
        for tab in SettingsTab::ALL {
            rig.dialog.update_in(cx, |d, _, cx| d.select_tab(tab, cx));
            draw(cx);
        }
    }
}
