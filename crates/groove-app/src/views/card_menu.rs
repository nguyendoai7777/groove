//! groove-gd1_17: menu chuột phải của thẻ (Play all, đổi bìa S13, xoá) và hộp xác nhận xoá (N9).
//!
//! Hộp xác nhận nói rõ chỉ bỏ khỏi thư viện, tệp trên đĩa còn nguyên; album và thư mục lần quét sau không quay lại.

use groove_core::model::CategoryRef;
use groove_core::t;
use materiant::button::Button;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{App, ClickEvent, Context, IntoElement, Render, SharedString, WeakEntity, Window, div};
use materiant::menu::{Menu, MenuItem};
use materiant::overlay::dialog::{Dialog, DialogHandle};

use crate::library_ctl::{self, LibraryCtl};

/// Mục của menu thẻ. Playlist không có ảnh nên không có mục đổi bìa (đặc tả 1.2).
pub fn build(menu: Menu, reference: CategoryRef, cx: &App) -> Menu {
    let Some(ctl) = library_ctl::ctl(cx) else { return menu };
    let play = ctl.clone();
    let menu = menu.item(MenuItem::new("card-menu-play", t!("actions.card.play_all")).on_select(move |_, cx| {
        play.update(cx, |ctl, cx| ctl.play_category(reference, cx));
    }));
    let menu = if matches!(reference, CategoryRef::Playlist(_)) {
        menu
    } else {
        let (cover, metadata) = (ctl.clone(), ctl.clone());
        menu.item(MenuItem::new("card-menu-cover", t!("actions.card.set_cover")).on_select(move |_, cx| {
            cover.update(cx, |ctl, cx| ctl.pick_category_cover(reference, cx));
        }))
        .item(MenuItem::new("card-menu-metadata", t!("actions.card.edit_metadata")).on_select(
            move |_, cx| {
                metadata.update(cx, |ctl, cx| ctl.open_category_metadata(reference, cx));
            },
        ))
    };
    menu.separator().item(MenuItem::new("card-menu-delete", delete_label(reference)).danger(true).on_select(
        move |_, cx| {
            ctl.update(cx, |ctl, cx| ctl.request_delete(reference, cx));
        },
    ))
}

/// Nhãn đúng với việc làm: album, thư mục chỉ bỏ khỏi thư viện (N9).
pub fn delete_label(reference: CategoryRef) -> &'static str {
    match reference {
        CategoryRef::Playlist(_) => t!("actions.card.delete_playlist"),
        CategoryRef::Album(_) | CategoryRef::Folder(_) => t!("actions.card.remove_from_library"),
    }
}

fn confirm_body(reference: CategoryRef, name: &str) -> String {
    match reference {
        CategoryRef::Album(_) => t!("actions.confirm.album", name = name),
        CategoryRef::Folder(_) => t!("actions.confirm.folder", name = name),
        CategoryRef::Playlist(_) => t!("actions.confirm.playlist", name = name),
    }
}

pub struct ConfirmDelete {
    handle: DialogHandle,
    ctl: WeakEntity<LibraryCtl>,
    target: Option<(CategoryRef, SharedString)>,
}

impl ConfirmDelete {
    pub fn new(ctl: WeakEntity<LibraryCtl>, cx: &mut Context<Self>) -> ConfirmDelete {
        ConfirmDelete { handle: DialogHandle::new(cx), ctl, target: None }
    }

    pub fn open(&mut self, reference: CategoryRef, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.target = Some((reference, name.into()));
        self.handle.open(window, cx);
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.close(window, cx);
        if let Some((reference, _)) = self.target.take()
            && let Some(ctl) = self.ctl.upgrade()
        {
            ctl.update(cx, |ctl, cx| ctl.remove_category(reference, cx));
        }
    }

    #[cfg(test)]
    pub fn confirm_for_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm(window, cx);
    }
}

impl Render for ConfirmDelete {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (label, body) = match &self.target {
            Some((reference, name)) => (delete_label(*reference), confirm_body(*reference, name)),
            None => ("", String::new()),
        };
        let handle = self.handle.clone();
        div().child(
            Dialog::new("confirm-delete", &self.handle)
                .title(t!("actions.confirm.title"))
                .actions([
                    Button::new("confirm-delete-cancel", t!("actions.confirm.cancel"))
                        .text()
                        .on_click(move |_: &ClickEvent, window, cx| handle.close(window, cx))
                        .into_any_element(),
                    Button::new("confirm-delete-ok", label)
                        .filled()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.confirm(window, cx)))
                        .into_any_element(),
                ])
                .child(body),
        )
    }
}
