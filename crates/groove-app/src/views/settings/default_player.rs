//! groove-gd2_01: mục "Trình phát mặc định" trong tab Cài đặt chung.
//!
//! Cho biết app có đang mở tệp MP3 không, và một nút: Windows mở trang Default apps (người dùng tự chọn), Linux, macOS đặt cho
//! tài khoản này. **Chỉ đổi khi người dùng bấm**: mở hộp, quay lại cửa sổ chỉ hỏi lại trạng thái.
//!
//! Hỏi và đặt đều chặn (registry, tiến trình con) nên chạy ở luồng nền (N7). Hỏi lại khi mở hộp và mỗi lần cửa sổ được kích
//! hoạt lại, vì người dùng đặt xong ở trang hệ thống rồi mới quay về.

use groove_core::t;
use materiant::button::Button;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    ClickEvent, Context, FontWeight, IntoElement, ParentElement, Render, Styled, Subscription, Window, div, px,
};
use materiant::theme::ActiveTheme;

use crate::platform::default_player::{self, OPENS_SYSTEM_SETTINGS, Outcome, Status};

/// Chỗ hỏi, đặt thật; test thay bằng hàm giả để không chạm OS.
#[derive(Clone, Copy)]
pub struct Backend {
    pub status: fn() -> Status,
    pub make_default: fn() -> Result<Outcome, String>,
}

impl Backend {
    pub const SYSTEM: Backend = Backend { status: default_player::status, make_default: default_player::make_default };
}

/// Kết quả lần bấm gần nhất.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    OpenedSettings,
    Applied,
    Failed,
}

pub struct DefaultPlayerPanel {
    backend: Backend,
    /// `None`: đang hỏi lần đầu.
    status: Option<Status>,
    note: Option<Note>,
    busy: bool,
    generation: u64,
    _activation: Subscription,
}

impl DefaultPlayerPanel {
    pub fn new(backend: Backend, window: &mut Window, cx: &mut Context<Self>) -> DefaultPlayerPanel {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.refresh(cx);
            }
        });
        DefaultPlayerPanel { backend, status: None, note: None, busy: false, generation: 0, _activation: activation }
    }

    #[cfg(test)]
    pub fn status(&self) -> Option<Status> {
        self.status
    }

    #[cfg(test)]
    pub fn note(&self) -> Option<Note> {
        self.note
    }

    /// Hỏi lại OS ở nền. Lượt cũ về sau lượt mới thì bỏ.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let status = self.backend.status;
        let task = cx.background_spawn(async move { status() });
        cx.spawn(async move |this, cx| {
            let status = task.await;
            this.update(cx, |this, cx| {
                if this.generation == generation {
                    this.status = Some(status);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Người dùng bấm nút.
    pub fn make_default(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.note = None;
        let make_default = self.backend.make_default;
        let task = cx.background_spawn(async move { make_default() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                this.note = Some(match result {
                    Ok(Outcome::OpenedSystemSettings) => Note::OpenedSettings,
                    Ok(Outcome::Applied) => Note::Applied,
                    Err(err) => {
                        log::warn!("default player: {err}");
                        Note::Failed
                    }
                });
                this.refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Render for DefaultPlayerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().colors().muted;
        let status = match self.status {
            None => t!("settings.default_player.checking"),
            Some(Status::Default) => t!("settings.default_player.is_default"),
            Some(Status::NotDefault) => t!("settings.default_player.not_default"),
            Some(Status::NotInstalled) => t!("settings.default_player.not_installed"),
            Some(Status::Unknown) => t!("settings.default_player.unknown"),
        };
        let (label, hint) = if OPENS_SYSTEM_SETTINGS {
            (t!("settings.default_player.open_settings"), t!("settings.default_player.hint_settings"))
        } else {
            (t!("settings.default_player.set"), t!("settings.default_player.hint_set"))
        };
        let note = self.note.map(|note| match note {
            Note::OpenedSettings => t!("settings.default_player.opened"),
            Note::Applied => t!("settings.default_player.applied"),
            Note::Failed => t!("settings.default_player.failed"),
        });
        let usable = self.status != Some(Status::NotInstalled);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t!("settings.default_player.title")))
            .child(div().child(status))
            .child(
                div().flex().items_center().gap_3().child(
                    Button::new("settings-default-player", label)
                        .outlined()
                        .loading(self.busy)
                        .disabled(!usable || self.busy)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.make_default(cx))),
                ),
            )
            .children(note.map(|note| div().text_size(px(12.)).child(note)))
            .child(div().text_size(px(12.)).text_color(muted).child(hint))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use gpui::TestAppContext;

    use super::*;

    static CALLS: AtomicUsize = AtomicUsize::new(0);

    fn not_default() -> Status {
        Status::NotDefault
    }

    fn counting_make_default() -> Result<Outcome, String> {
        CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(Outcome::Applied)
    }

    fn opening_settings() -> Result<Outcome, String> {
        Ok(Outcome::OpenedSystemSettings)
    }

    fn failing_make_default() -> Result<Outcome, String> {
        Err("boom".to_owned())
    }

    fn panel(
        backend: Backend,
        cx: &mut TestAppContext,
    ) -> (gpui::Entity<DefaultPlayerPanel>, &mut gpui::VisualTestContext) {
        cx.update(|cx| {
            materiant::init(cx);
            groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        });
        let (view, cx) = cx.add_window_view(|window, cx| DefaultPlayerPanel::new(backend, window, cx));
        (view, cx)
    }

    /// C5: mở, hỏi lại bao nhiêu lần cũng không đặt gì; chỉ bấm mới gọi đặt, đúng một lần.
    #[gpui::test]
    fn never_changes_until_the_user_clicks(cx: &mut TestAppContext) {
        CALLS.store(0, Ordering::SeqCst);
        let (view, cx) = panel(Backend { status: not_default, make_default: counting_make_default }, cx);
        view.update(cx, |panel, cx| {
            panel.refresh(cx);
            panel.refresh(cx);
        });
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |panel, _| panel.status()), Some(Status::NotDefault));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);

        view.update(cx, |panel, cx| panel.make_default(cx));
        cx.run_until_parked();
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(view.read_with(cx, |panel, _| panel.note()), Some(Note::Applied));
    }

    /// Windows: nút chỉ mở trang hệ thống, chữ báo người dùng tự chọn ở đó.
    #[gpui::test]
    fn opening_settings_is_reported(cx: &mut TestAppContext) {
        let (view, cx) = panel(Backend { status: not_default, make_default: opening_settings }, cx);
        view.update(cx, |panel, cx| panel.make_default(cx));
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |panel, _| panel.note()), Some(Note::OpenedSettings));
        assert_eq!(view.read_with(cx, |panel, _| panel.status()), Some(Status::NotDefault));
    }

    #[gpui::test]
    fn failure_is_reported_not_swallowed(cx: &mut TestAppContext) {
        let (view, cx) = panel(Backend { status: not_default, make_default: failing_make_default }, cx);
        view.update(cx, |panel, cx| panel.make_default(cx));
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |panel, _| panel.note()), Some(Note::Failed));
    }
}
