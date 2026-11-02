//! groove-gd1_17: hộp tạo playlist: tên + màu, mặc định màu nhấn của theme (đặc tả 1.2; bản cũ `#06b6d4`, G-4).
//!
//! Tên rỗng thì toast; trùng tên báo ngay dưới ô bằng chữ i18n, không in lỗi SQL thô như bản cũ.

use groove_core::t;
use groove_library::actor::LibraryError;
use groove_library::db::queries::QueryError;
use groove_library::store::Library;
use materiant::button::Button;
use materiant::color_picker::{ColorPicker, ColorPickerLabels, Rgb};
use materiant::field::input::TextField;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{App, ClickEvent, Context, Entity, IntoElement, Render, WeakEntity, Window, div};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::theme::ActiveTheme;
use materiant::toast::{Toast, ToastKind, ToastSpec};

use crate::library_ctl::LibraryCtl;

/// Chữ cho lỗi tạo playlist; còn lại là lỗi người dùng không sửa được.
pub fn create_error_text(err: &LibraryError) -> &'static str {
    match err {
        LibraryError::Query(QueryError::EmptyName) => t!("actions.playlist.empty_name"),
        LibraryError::Query(QueryError::DuplicatePlaylist(_)) => t!("actions.playlist.duplicate"),
        _ => t!("actions.playlist.create_failed"),
    }
}

/// Màu mặc định của playlist mới: màu nhấn của theme lúc mở hộp (G-4).
fn default_color(cx: &App) -> Rgb {
    let rgba = cx.theme().colors().primary.to_rgb();
    let channel = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Rgb::new(channel(rgba.r), channel(rgba.g), channel(rgba.b))
}

pub struct PlaylistDialog {
    handle: DialogHandle,
    library: Option<Library>,
    ctl: WeakEntity<LibraryCtl>,
    name: Entity<TextField>,
    color: Entity<ColorPicker>,
    add_song: Option<i64>,
    busy: bool,
}

impl PlaylistDialog {
    pub fn new(
        library: Option<Library>,
        ctl: WeakEntity<LibraryCtl>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PlaylistDialog {
        let this = cx.entity().downgrade();
        let name = cx.new(|cx| {
            TextField::new("playlist-name", window, cx)
                .label(t!("actions.playlist.name_label"))
                .label_weight(materiant::gpui_kit::FontWeight::NORMAL)
                .placeholder(t!("actions.playlist.name_placeholder"))
                .on_submit(move |_, window, cx| {
                    this.update(cx, |this, cx| this.submit(window, cx)).ok();
                })
        });
        let color = cx.new(|cx| {
            let labels = ColorPickerLabels {
                area: t!("actions.playlist.color_area").into(),
                hue: t!("actions.playlist.color_hue").into(),
                hex: t!("actions.playlist.color_hex").into(),
            };
            ColorPicker::new("playlist-color", default_color(cx), window, cx).labels(labels, cx)
        });
        PlaylistDialog { handle: DialogHandle::new(cx), library, ctl, name, color, add_song: None, busy: false }
    }

    /// `add_song`: bài thêm luôn sau khi tạo.
    pub fn open(&mut self, add_song: Option<i64>, window: &mut Window, cx: &mut Context<Self>) {
        self.add_song = add_song;
        self.busy = false;
        self.name.update(cx, |name, cx| {
            name.clear(window, cx);
            name.set_error(None, cx);
        });
        self.color.update(cx, |color, cx| color.set_value(default_color(cx), window, cx));
        self.handle.open(window, cx);
        cx.notify();
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    #[cfg(test)]
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.name.read(cx).value(cx).trim().to_owned();
        if name.is_empty() {
            Toast::push(window, cx, ToastSpec::new(ToastKind::Warning, t!("actions.playlist.empty_name")));
            return;
        }
        let Some(library) = self.library.clone() else { return };
        let color = self.color.read(cx).value().to_hex();
        let add_song = self.add_song;
        self.busy = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let created = library.create_playlist(name, color).await;
            this.update_in(cx, |this, window, cx| this.finished(created, add_song, window, cx)).ok();
        })
        .detach();
    }

    fn finished(
        &mut self,
        created: Result<i64, LibraryError>,
        add_song: Option<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match created {
            Ok(id) => {
                self.handle.close(window, cx);
                if let (Some(song), Some(ctl)) = (add_song, self.ctl.upgrade()) {
                    ctl.update(cx, |ctl, cx| ctl.add_to_playlist(id, song, cx));
                }
            }
            Err(err) => {
                log::warn!("playlist dialog: cannot create: {err}");
                let text = create_error_text(&err);
                self.name.update(cx, |name, cx| name.set_error(Some(text.into()), cx));
            }
        }
        cx.notify();
    }

    #[cfg(test)]
    pub fn submit_for_test(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let name = name.to_owned();
        self.name.update(cx, |field, cx| field.set_value(name, window, cx));
        self.submit(window, cx);
    }
}

impl Render for PlaylistDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = self.handle.clone();
        div().child(
            Dialog::new("playlist-dialog", &self.handle)
                .title(t!("actions.playlist.title"))
                .title_weight(materiant::gpui_kit::FontWeight::NORMAL)
                .divided(true)
                .actions([
                    Button::new("playlist-cancel", t!("actions.playlist.cancel"))
                        .text()
                        .on_click(move |_: &ClickEvent, window, cx| handle.close(window, cx))
                        .into_any_element(),
                    Button::new("playlist-create", t!("actions.playlist.create"))
                        .filled()
                        .loading(self.busy)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.submit(window, cx)))
                        .into_any_element(),
                ])
                .child(div().flex().flex_col().gap_3().child(self.name.clone()).child(self.color.clone())),
        )
    }
}
