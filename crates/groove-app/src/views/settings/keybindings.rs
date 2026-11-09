//! groove-gd1_24, groove-gd2_04: tab Phím tắt — nút Thêm và Đặt lại mặc định ở đầu, dưới là danh sách phím tắt (tên, chip
//! phím, nhãn "Toàn cục", sửa / xoá); thêm / sửa ở hộp lồng [`KeyEditor`]. Thay ô JSON của gd1_24 (chủ dự án 09/11/2026),
//! giữ dạng lưu `[{key, command}]` thêm `label`, `global`.
//!
//! Thứ tự là thứ tự sửa, mới nhất trên đầu (chủ dự án 09/11/2026): dòng vừa thêm / sửa dời lên đầu danh sách và lưu theo thứ
//! tự đó, nên không cần dấu thời gian. Hệ quả: dòng trùng phím thì dòng vừa sửa thắng (`Keymap` lấy dòng trước).
//!
//! Danh sách là **bản nháp**: chỉ nút OK của hộp Settings mới lưu (`draft` rồi `app::update_settings`); đóng kiểu khác bỏ
//! nháp (M3). Dòng có vấn đề hiện cảnh báo ngay dưới nó: trùng phím, chuỗi bị che, lệnh lạ / phím sai (tệp sửa tay), phím
//! toàn cục sai dạng hay bị OS từ chối ở lần đăng ký gần nhất. Cảnh báo không chặn OK: dòng trùng thì dòng trước thắng như
//! `Keymap`.

use groove_core::keymap::{self, Chord, Command, KeySequence};
use groove_core::settings::{KeyBinding, Settings, default_keybindings};
use groove_core::t;
use materiant::button::{Button, IconButton};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, px,
};
use materiant::kbd::Kbd;
use materiant::overlay::tooltip::TooltipExt;
use materiant::theme::{ActiveTheme, Neutral};

use super::key_editor::{self, KeyEditor, KeyEditorEvent};
use crate::icons::GrooveIcon;
use crate::platform::global_keys;

/// Cạnh nút sửa / xoá và cỡ icon của chúng.
const ACTION_EDGE: f32 = 28.;
const ACTION_ICON: f32 = 16.;
/// Bo góc của khung danh sách.
const LIST_RADIUS: f32 = 8.;

/// Kết quả so bản nháp với lúc mở.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
    /// Giống danh sách đang có: không ghi gì.
    Unchanged,
    /// Đúng bằng mặc định: ghi `None` để phím mới của bản sau tới được người dùng.
    Defaults,
    Custom(Vec<KeyBinding>),
}

pub fn draft_of(list: &[KeyBinding], current: &[KeyBinding]) -> Draft {
    if list == current {
        Draft::Unchanged
    } else if list == default_keybindings() {
        Draft::Defaults
    } else {
        Draft::Custom(list.to_vec())
    }
}

/// Cảnh báo của dòng `index` (không tính OS từ chối: cần `App`).
pub fn row_warnings(list: &[KeyBinding], index: usize) -> Vec<String> {
    let binding = &list[index];
    let mut out = Vec::new();
    if Command::from_name(&binding.command).is_none() {
        out.push(t!("settings.keys.warn.unknown_command", command = binding.command));
    }
    let Ok(key) = KeySequence::parse(&binding.key) else {
        out.push(t!("settings.keys.warn.invalid_key", key = binding.key));
        return out;
    };
    if let Some(conflict) = keymap::conflict(list, Some(index), &key) {
        out.push(key_editor::conflict_text(conflict, list));
    }
    if binding.global
        && let Some(issue) = key.global_issue()
    {
        out.push(key_editor::global_issue_text(issue).to_owned());
    }
    out
}

/// Lưu dòng thêm (`index` `None`) / sửa: dời lên đầu, mới nhất trên đầu.
pub fn save_row(list: &mut Vec<KeyBinding>, index: Option<usize>, binding: KeyBinding) {
    if let Some(ix) = index.filter(|ix| *ix < list.len()) {
        list.remove(ix);
    }
    list.insert(0, binding);
}

pub struct KeybindingsPanel {
    /// Bản nháp.
    list: Vec<KeyBinding>,
    /// Danh sách có hiệu lực lúc mở hộp.
    current: Vec<KeyBinding>,
    editor: Entity<KeyEditor>,
    _subscriptions: Vec<Subscription>,
}

impl KeybindingsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> KeybindingsPanel {
        let editor = cx.new(|cx| KeyEditor::new(window, cx));
        let saved = cx.subscribe(&editor, |this, _, event: &KeyEditorEvent, cx| {
            let KeyEditorEvent::Save { index, binding } = event;
            save_row(&mut this.list, *index, binding.clone());
            cx.notify();
        });
        KeybindingsPanel {
            list: default_keybindings(),
            current: default_keybindings(),
            editor,
            _subscriptions: vec![saved],
        }
    }

    /// Nạp danh sách đang có hiệu lực (lúc mở hộp Settings).
    pub fn show(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        self.current = settings.keybindings();
        self.list = self.current.clone();
        self.editor.update(cx, |editor, cx| {
            if editor.is_open() {
                editor.close(window, cx);
            }
        });
        cx.notify();
    }

    pub fn draft(&self) -> Draft {
        draft_of(&self.list, &self.current)
    }

    #[cfg(test)]
    pub fn list(&self) -> &[KeyBinding] {
        &self.list
    }

    #[cfg(test)]
    pub fn editor(&self) -> &Entity<KeyEditor> {
        &self.editor
    }

    pub fn open_editor(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let list = self.list.clone();
        self.editor.update(cx, |editor, cx| editor.open(list, index, window, cx));
    }

    pub fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.list.len() {
            self.list.remove(index);
            cx.notify();
        }
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.list = default_keybindings();
        cx.notify();
    }

    fn row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors().clone();
        let binding = &self.list[index];
        let name = key_editor::binding_name(binding);
        let mut warnings = row_warnings(&self.list, index);
        let key = KeySequence::parse(&binding.key).ok();
        if binding.global
            && let Some(key) = &key
            && key.global_issue().is_none()
            && global_keys::failure(key, cx).is_some()
        {
            warnings.push(t!("settings.keys.warn.os_taken", key = key.label()));
        }
        let keys: AnyElement = match &key {
            Some(key) => div()
                .flex()
                .flex_none()
                .items_center()
                .gap_2()
                .children(key.chords().iter().map(|chord: &Chord| Kbd::new(&chord.label())))
                .into_any_element(),
            None => div().flex_none().text_color(c.muted).child(binding.key.clone()).into_any_element(),
        };
        let action = |id: &'static str, icon: GrooveIcon, label: &'static str, danger: bool| {
            let ink = if danger { c.danger } else { c.text };
            IconButton::new((id, index), icon)
                .accessibility_label(label)
                .ink(ink)
                .w(px(ACTION_EDGE))
                .h(px(ACTION_EDGE))
                .icon_size(px(ACTION_ICON))
                .rounded_full()
        };
        let edit = action("settings-key-edit", GrooveIcon::Edit, t!("settings.keys.edit"), false)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_editor(Some(index), window, cx)));
        let delete = action("settings-key-delete", GrooveIcon::Delete, t!("settings.keys.delete"), true)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove(index, cx)));
        let badge = binding.global.then(|| {
            div()
                .flex_none()
                .px_2()
                .rounded(px(4.))
                .border_1()
                .border_color(c.primary)
                .text_size(px(11.))
                .text_color(c.primary_ink)
                .child(t!("settings.keys.global_badge"))
        });
        div()
            .id(("settings-key-row", index))
            .debug_selector(move || format!("settings-key-row-{index}"))
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .when(index > 0, |d| d.border_t_1().border_color(c.border))
            // Nền rê của dòng đầu / cuối tự bo theo khung (GPUI không cắt con theo bo góc), trừ bề dày viền.
            .when(index == 0, |d| d.rounded_t(px(LIST_RADIUS - 1.)))
            .when(index + 1 == self.list.len(), |d| d.rounded_b(px(LIST_RADIUS - 1.)))
            .hover(|s| s.bg(c.layer(Neutral::N100)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    // Tên người dùng đặt có thể dài: xuống dòng, không cắt "…".
                    .child(
                        div().flex_1().min_w_0().flex().flex_wrap().items_center().gap_2().child(name).children(badge),
                    )
                    .child(keys)
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .gap_1()
                            .child(
                                div()
                                    .child(edit)
                                    .with_tooltip(("settings-key-edit-tip", index), t!("settings.keys.edit")),
                            )
                            .child(
                                div()
                                    .child(delete)
                                    .with_tooltip(("settings-key-delete-tip", index), t!("settings.keys.delete")),
                            ),
                    ),
            )
            .children(warnings.into_iter().map(|text| key_editor::warning_line(text, c.warning)))
            .into_any_element()
    }
}

impl Render for KeybindingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let rows: Vec<AnyElement> = (0..self.list.len()).map(|index| self.row(index, cx)).collect();
        let list = if rows.is_empty() {
            div().px_3().py_3().text_color(c.muted).child(t!("settings.keys.empty")).into_any_element()
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let actions = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                Button::new("settings-keys-add", t!("settings.keys.add"))
                    .filled()
                    .primary()
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_editor(None, window, cx))),
            )
            .child(
                Button::new("settings-keys-reset", t!("settings.keys.reset"))
                    .outlined()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reset(cx))),
            );
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(actions)
            .child(
                div()
                    .debug_selector(|| "settings-keys-list".into())
                    .rounded(px(LIST_RADIUS))
                    .border_1()
                    .border_color(c.border)
                    .overflow_hidden()
                    .child(list),
            )
            .child(self.editor.clone())
    }
}

/// Phím toàn cục theo danh sách vừa lưu.
pub fn apply_global(cx: &mut App) {
    let bindings = crate::app::settings(cx).keybindings();
    global_keys::apply(&bindings, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(key: &str, command: &str) -> KeyBinding {
        KeyBinding::new(key, command)
    }

    /// Mặc định tới đúng nó: không đổi. Bằng mặc định mà đang là bản tuỳ biến: về `None`.
    #[test]
    fn unchanged_defaults_and_custom_are_told_apart() {
        let defaults = default_keybindings();
        assert_eq!(draft_of(&defaults, &defaults), Draft::Unchanged);
        let custom = vec![binding("ctrl+k", "open_search")];
        assert_eq!(draft_of(&defaults, &custom), Draft::Defaults);
        assert_eq!(draft_of(&custom, &defaults), Draft::Custom(custom.clone()));
        assert_eq!(draft_of(&[], &defaults), Draft::Custom(vec![]), "bỏ hết phím là việc được phép");
    }

    /// Thêm và sửa đều dời dòng lên đầu; sửa không nhân đôi dòng.
    #[test]
    fn saved_rows_move_to_the_top() {
        let keys = |list: &[KeyBinding]| list.iter().map(|b| b.key.clone()).collect::<Vec<_>>();
        let mut list = vec![binding("a", "toggle_play"), binding("b", "next_track"), binding("c", "open_search")];
        save_row(&mut list, Some(2), binding("j", "open_search"));
        assert_eq!(keys(&list), ["j", "a", "b"]);
        save_row(&mut list, None, binding("x", "previous_track"));
        assert_eq!(keys(&list), ["x", "j", "a", "b"]);
    }

    /// Mỗi vấn đề của dòng ra lời báo riêng, nhắc đúng thứ gây ra; không khoá trần lọt ra.
    #[test]
    fn row_warnings_name_the_problem() {
        let mut global_seq = binding("g x", "toggle_play");
        global_seq.global = true;
        let list = [
            binding("ctrl+k", "open_search"),
            binding("ctrl+k", "next_track"),
            binding("x", "launch_rocket"),
            binding("ctrl+", "toggle_play"),
            binding("g", "play_random"),
            binding("g m", "go_to_library"),
            global_seq,
        ];
        let all: Vec<Vec<String>> = (0..list.len()).map(|ix| row_warnings(&list, ix)).collect();
        assert_eq!(all[0].len(), 1, "trùng hiện ở cả hai dòng");
        assert!(all[1][0].contains(Command::OpenSearch.label()), "{:?}", all[1]);
        assert!(all[2][0].contains("launch_rocket"));
        assert!(all[3][0].contains("ctrl+"));
        assert!(all[5][0].contains(Command::PlayRandom.label()), "chuỗi bị phím đơn che: {:?}", all[5]);
        assert_eq!(all[6].len(), 2, "che + phím toàn cục là chuỗi: {:?}", all[6]);
        for text in all.iter().flatten() {
            assert!(!text.starts_with("settings."), "khoá trần lọt ra: {text}");
        }
    }
}
