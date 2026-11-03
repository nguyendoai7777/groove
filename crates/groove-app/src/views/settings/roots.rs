//! groove-gd1_24: tab Thư viện: danh sách thư mục nhạc đang theo dõi (S6, N9), thêm, bỏ, quét lại.
//!
//! Đọc bằng `Library::watched_roots` (luồng nền, UI không chạm đĩa). Mọi thao tác đi qua `LibraryCtl`: thêm là
//! `pick_and_import` (hộp chọn thư mục của `Picker`), quét là `rescan`, bỏ là `forget_root` (xoá bài dưới gốc, không đụng tệp).
//! Bỏ gốc hỏi lại bằng nút đổi chữ (bấm lần hai mới bỏ), không mở thêm hộp trong hộp.
//!
//! Danh sách đọc lại khi mở tab và mỗi khi `LibraryCtl` báo một `Notice` (nhập xong, lỗi); bỏ gốc thì gỡ khỏi danh sách ngay.

use std::path::{Path, PathBuf};

use groove_core::t;
use groove_library::store::Library;
use materiant::button::{Button, Size};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, px,
};
use materiant::theme::ActiveTheme;

use crate::library_ctl::{LibraryCtl, LibraryCtlEvent};

/// Danh sách đang ở trạng thái nào.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootsState {
    /// Chưa có thư viện: không liệt kê được.
    Unavailable,
    Loading,
    Ready(Vec<PathBuf>),
}

pub struct RootsPanel {
    ctl: Option<Entity<LibraryCtl>>,
    state: RootsState,
    /// Gốc đang chờ bấm lần hai.
    confirming: Option<PathBuf>,
    generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl RootsPanel {
    pub fn new() -> RootsPanel {
        RootsPanel {
            ctl: None,
            state: RootsState::Unavailable,
            confirming: None,
            generation: 0,
            _subscriptions: vec![],
        }
    }

    #[cfg(test)]
    pub fn state(&self) -> &RootsState {
        &self.state
    }

    #[cfg(test)]
    pub fn roots(&self) -> &[PathBuf] {
        match &self.state {
            RootsState::Ready(roots) => roots,
            _ => &[],
        }
    }

    /// Gắn với `LibraryCtl` rồi đọc danh sách. Gọi mỗi lần mở hộp (ctl có thể chưa có lúc dựng hộp).
    pub fn bind(&mut self, ctl: Option<Entity<LibraryCtl>>, cx: &mut Context<Self>) {
        self._subscriptions.clear();
        if let Some(ctl) = &ctl {
            self._subscriptions.push(cx.subscribe(ctl, |this, _, _: &LibraryCtlEvent, cx| this.reload(cx)));
            // Cờ đang nhập, quét của ctl đổi theo `notify` của nó.
            self._subscriptions.push(cx.observe(ctl, |_, _, cx| cx.notify()));
        }
        self.ctl = ctl;
        self.confirming = None;
        self.reload(cx);
    }

    fn library(&self, cx: &App) -> Option<Library> {
        self.ctl.as_ref()?.read(cx).library().cloned()
    }

    /// Đọc lại danh sách ở nền.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.library(cx) else {
            self.state = RootsState::Unavailable;
            cx.notify();
            return;
        };
        self.generation += 1;
        let generation = self.generation;
        if !matches!(self.state, RootsState::Ready(_)) {
            self.state = RootsState::Loading;
        }
        cx.spawn(async move |this, cx| {
            let roots = library.watched_roots().await;
            this.update(cx, |this, cx| {
                // Lượt đọc cũ về sau lượt mới (hay sau khi vừa bỏ một gốc): bỏ.
                if this.generation != generation {
                    return;
                }
                match roots {
                    Ok(roots) => this.state = RootsState::Ready(roots),
                    Err(err) => {
                        log::warn!("settings: cannot read the library roots: {err}");
                        if !matches!(this.state, RootsState::Ready(_)) {
                            this.state = RootsState::Unavailable;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn busy(&self, cx: &App) -> bool {
        self.ctl.as_ref().is_some_and(|ctl| {
            let ctl = ctl.read(cx);
            ctl.is_importing() || ctl.is_rescanning()
        })
    }

    pub fn add(&mut self, cx: &mut Context<Self>) {
        if let Some(ctl) = self.ctl.clone() {
            ctl.update(cx, |ctl, cx| ctl.pick_and_import(cx));
        }
    }

    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        if let Some(ctl) = self.ctl.clone() {
            ctl.update(cx, |ctl, cx| ctl.rescan(cx));
            cx.notify();
        }
    }

    /// Bấm "Bỏ": lần đầu hỏi lại, lần hai bỏ thật.
    pub fn remove(&mut self, root: &Path, cx: &mut Context<Self>) {
        if self.confirming.as_deref() != Some(root) {
            self.confirming = Some(root.to_path_buf());
            cx.notify();
            return;
        }
        self.confirming = None;
        if let Some(ctl) = self.ctl.clone() {
            ctl.update(cx, |ctl, cx| ctl.forget_root(root.to_path_buf(), cx));
        }
        // Gỡ khỏi danh sách ngay, và huỷ lượt đọc đang bay (nó còn thấy gốc này).
        self.generation += 1;
        if let RootsState::Ready(roots) = &mut self.state {
            roots.retain(|existing| existing != root);
        }
        cx.notify();
    }

    fn row(&self, ix: usize, root: &Path, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors().clone();
        let confirming = self.confirming.as_deref() == Some(root);
        let target = root.to_path_buf();
        let label = if confirming { t!("settings.roots.confirm_remove") } else { t!("settings.roots.remove") };
        div()
            .flex()
            .items_center()
            .gap_3()
            .py_1()
            .border_b_1()
            .border_color(c.border)
            // Đường dẫn dài xuống dòng (ngắt giữa chữ khi không có chỗ ngắt), không cắt "…".
            .child(div().flex_1().min_w_0().child(root.display().to_string()))
            .child(
                Button::new(("settings-root-remove", ix), label)
                    .size(Size::Sm)
                    .danger()
                    .when(confirming, |button| button.filled())
                    .when(!confirming, |button| button.outlined())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove(&target, cx))),
            )
            .into_any_element()
    }
}

impl Default for RootsPanel {
    fn default() -> Self {
        RootsPanel::new()
    }
}

impl Render for RootsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().colors().muted;
        let busy = self.busy(cx);
        let usable = self.ctl.is_some() && self.state != RootsState::Unavailable;
        let list = match &self.state {
            RootsState::Unavailable => div().text_color(muted).child(t!("settings.roots.unavailable")),
            RootsState::Loading => div(),
            RootsState::Ready(roots) if roots.is_empty() => div().text_color(muted).child(t!("settings.roots.empty")),
            RootsState::Ready(roots) => {
                let roots = roots.clone();
                let rows: Vec<_> = roots.iter().enumerate().map(|(ix, root)| self.row(ix, root, cx)).collect();
                div().flex().flex_col().children(rows)
            }
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().child(t!("settings.roots.title")))
            .child(div().text_size(px(12.)).text_color(muted).child(t!("settings.roots.hint")))
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("settings-root-add", t!("settings.roots.add"))
                            .filled()
                            .disabled(!usable || busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add(cx))),
                    )
                    .child(
                        Button::new("settings-root-rescan", t!("settings.roots.rescan"))
                            .outlined()
                            .loading(busy)
                            .disabled(!usable || busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.rescan(cx))),
                    )
                    .children(
                        busy.then(|| div().text_size(px(12.)).text_color(muted).child(t!("settings.roots.busy"))),
                    ),
            )
            .child(div().text_size(px(12.)).text_color(muted).child(t!("settings.roots.remove_note")))
    }
}
