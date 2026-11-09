//! groove-gd2_04: hộp thêm / sửa một phím tắt (lồng trong hộp Settings): tên hiển thị, chức năng, ô ghi phím, công tắc toàn
//! cục, dòng cảnh báo trùng phím / OS từ chối.
//!
//! Ghi phím như hộp gán phím của fastcast (`fastcast/crates/ui/src/hotkey_dialog.rs`): phím đi vào bộ ghi qua
//! `intercept_keystrokes`, trước mọi action (Esc của hộp, Enter của nút); GPUI vẫn chạy key listener sau bộ chặn nên có cờ
//! `swallow`. Khác fastcast: nhận cả phím trơn và chuỗi hai phím (`g m`): phím sau tới trong `SEQUENCE_TIMEOUT` thì nối
//! thành chuỗi, như luật khớp của `keymap::KeyMatcher`. Đang ghi thì phím toàn cục tạm gỡ (`global_keys::suspend`), không
//! thì bấm lại đúng tổ hợp đang gán sẽ chạy lệnh thay vì vào bộ ghi.

use std::time::Instant;

use groove_core::keymap::{self, Chord, Command, Conflict, GlobalIssue, KeySequence, MAX_SEQUENCE, SEQUENCE_TIMEOUT};
use groove_core::settings::KeyBinding;
use groove_core::t;
use materiant::button::Button;
use materiant::field::TextField;
use materiant::gpui_kit::base::Disableable;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontWeight, Hsla, KeyDownEvent, Keystroke,
    Render, SharedString, Subscription, Window, div, px,
};
use materiant::icon::{Icon, IconName};
use materiant::kbd::Kbd;
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::select::Select;
use materiant::switch::Switch;
use materiant::theme::{ActiveTheme, Neutral};

use crate::platform::global_keys;

/// Bề ngang phần thân hộp.
const BODY_WIDTH: f32 = 420.;
/// Cao tối thiểu của ô ghi phím (đủ một hàng chip).
const RECORDER_HEIGHT: f32 = 48.;
/// Lề trái chữ gợi ý dưới ô, bằng gợi ý của `TextField`.
const HINT_INSET: f32 = 8.;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyEditorEvent {
    /// Lưu dòng `index` (`None` là dòng mới).
    Save { index: Option<usize>, binding: KeyBinding },
}

struct Recording {
    chords: Vec<Chord>,
    last: Option<Instant>,
    /// Bộ chặn phím của cửa sổ — rơi là thôi chặn.
    _intercept: Subscription,
}

pub struct KeyEditor {
    handle: DialogHandle,
    index: Option<usize>,
    /// Các dòng của danh sách (kể cả dòng đang sửa, nằm ở `index`), để soát trùng.
    list: Vec<KeyBinding>,
    label: Entity<TextField>,
    /// Chỉ số vào `Command::ALL`; `None` khi dòng cũ có lệnh lạ.
    command: Option<usize>,
    key: Option<KeySequence>,
    global: bool,
    recording: Option<Recording>,
    /// Bộ chặn vừa dùng phím này: key listener của ô ghi bỏ qua đúng một lần.
    swallow: bool,
    /// Bấm Lưu khi chưa có phím.
    need_key: bool,
    /// Kết quả đăng ký thử phím toàn cục với OS, cho `key` hiện tại.
    os_refused: bool,
    recorder_focus: FocusHandle,
}

impl EventEmitter<KeyEditorEvent> for KeyEditor {}

impl KeyEditor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> KeyEditor {
        let label = cx.new(|cx| {
            TextField::new("key-editor-label", window, cx)
                .label(t!("settings.keys.editor.label"))
                .label_weight(FontWeight::NORMAL)
                .hint(t!("settings.keys.editor.label_hint"))
        });
        KeyEditor {
            handle: DialogHandle::new(cx),
            index: None,
            list: Vec::new(),
            label,
            command: None,
            key: None,
            global: false,
            recording: None,
            swallow: false,
            need_key: false,
            os_refused: false,
            recorder_focus: cx.focus_handle().tab_stop(true),
        }
    }

    /// Mở hộp cho dòng `index` của `list` (`None`: thêm dòng mới).
    pub fn open(&mut self, list: Vec<KeyBinding>, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let binding = index.and_then(|ix| list.get(ix)).cloned();
        let binding = binding.unwrap_or_else(|| KeyBinding::new("", Command::ALL[0].name()));
        self.stop_recording(cx);
        self.list = list;
        self.index = index;
        self.command = Command::from_name(&binding.command).and_then(|c| Command::ALL.iter().position(|o| *o == c));
        self.key = KeySequence::parse(&binding.key).ok();
        self.global = binding.global && global_keys::supported();
        self.need_key = false;
        let text = binding.label;
        self.label.update(cx, |field, cx| field.set_value(text, window, cx));
        self.refresh_probe(cx);
        self.handle.open(window, cx);
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    #[cfg(test)]
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.handle.close(window, cx);
        cx.notify();
    }

    fn start_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            return;
        }
        let view = cx.entity().downgrade();
        let own = window.window_handle().window_id();
        let intercept = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() != own {
                return;
            }
            let keystroke = event.keystroke.clone();
            if view.update(cx, |editor, cx| editor.recorder_key(&keystroke, cx)).unwrap_or(false) {
                cx.stop_propagation();
            }
        });
        self.recording = Some(Recording { chords: Vec::new(), last: None, _intercept: intercept });
        self.need_key = false;
        global_keys::suspend(true, cx);
        window.focus(&self.recorder_focus, cx);
        cx.notify();
    }

    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if self.recording.take().is_some() {
            global_keys::suspend(false, cx);
            cx.notify();
        }
    }

    /// Một phím lúc ghi. `true`: đã dùng (chặn khỏi action). Enter / Esc trơn là xong / huỷ.
    fn recorder_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(recording) = &self.recording else { return false };
        let has_chords = !recording.chords.is_empty();
        self.swallow = true;
        let this = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = this.update(cx, |editor, _| editor.swallow = false);
        });
        let m = &keystroke.modifiers;
        let plain = !(m.control || m.alt || m.shift || m.platform || m.function);
        match keystroke.key.as_str() {
            "escape" if plain => self.stop_recording(cx),
            "enter" if plain && has_chords => self.finish_recording(cx),
            _ => {
                let Some(chord) = crate::commands::chord_of(keystroke) else { return true };
                if let Some(recording) = &mut self.recording {
                    append_chord(&mut recording.chords, &mut recording.last, chord, Instant::now());
                }
                cx.notify();
            }
        }
        true
    }

    fn finish_recording(&mut self, cx: &mut Context<Self>) {
        let chords = self.recording.as_ref().map(|r| r.chords.clone()).unwrap_or_default();
        self.stop_recording(cx);
        if !chords.is_empty() {
            let text = chords.iter().map(Chord::to_string).collect::<Vec<_>>().join(" ");
            self.key = KeySequence::parse(&text).ok();
            self.need_key = false;
            self.refresh_probe(cx);
        }
    }

    fn refresh_probe(&mut self, cx: &mut Context<Self>) {
        self.os_refused = match &self.key {
            Some(key) if self.global && key.global_issue().is_none() => global_keys::probe(key, cx).is_err(),
            _ => false,
        };
    }

    fn set_global(&mut self, on: bool, cx: &mut Context<Self>) {
        self.global = on;
        self.refresh_probe(cx);
        cx.notify();
    }

    /// Dòng đang soạn; `None` khi còn thiếu phím hoặc chức năng.
    fn draft(&self, cx: &Context<Self>) -> Option<KeyBinding> {
        let command = Command::ALL.get(self.command?)?;
        let key = self.key.as_ref()?;
        Some(KeyBinding {
            key: key.to_string(),
            command: command.name().to_owned(),
            label: self.label.read(cx).value(cx).trim().to_owned(),
            global: self.global,
        })
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.finish_recording(cx);
        }
        match self.draft(cx) {
            Some(binding) => {
                let index = self.index;
                self.close(window, cx);
                cx.emit(KeyEditorEvent::Save { index, binding });
            }
            None => {
                self.need_key = self.key.is_none();
                cx.notify();
            }
        }
    }

    /// Mọi cảnh báo của dòng đang soạn, theo thứ tự đọc.
    fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.need_key {
            out.push(t!("settings.keys.editor.need_key").to_owned());
        }
        let Some(key) = &self.key else { return out };
        if let Some(conflict) = keymap::conflict(&self.list, self.index, key) {
            out.push(conflict_text(conflict, &self.list));
        }
        if self.global {
            match key.global_issue() {
                Some(issue) => out.push(global_issue_text(issue).to_owned()),
                None if self.os_refused => out.push(t!("settings.keys.warn.os_taken", key = key.label())),
                None => {}
            }
        }
        out
    }

    fn recorder(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let ring = materiant::style::focus_visible(&self.recorder_focus, window, cx);
        let recording = self.recording.as_ref();
        let chords: Option<Vec<String>> = match recording {
            Some(r) if !r.chords.is_empty() => Some(r.chords.iter().map(Chord::label).collect()),
            Some(_) => None,
            None => self.key.as_ref().map(|key| key.chords().iter().map(Chord::label).collect()),
        };
        let note =
            if recording.is_some() { t!("settings.keys.editor.recording") } else { t!("settings.keys.editor.record") };
        let shown: AnyElement = match chords {
            Some(chords) => div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(chords.iter().map(|l| Kbd::new(l)))
                .into_any_element(),
            None if recording.is_some() => {
                div().text_color(c.muted).child(t!("settings.keys.editor.waiting")).into_any_element()
            }
            None => div().text_color(c.muted).child("—").into_any_element(),
        };
        let border = if recording.is_some() || ring { c.primary } else { c.border };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_sm().child(t!("settings.keys.editor.key")))
            .child(
                div()
                    .id("key-editor-recorder")
                    .track_focus(&self.recorder_focus)
                    .debug_selector(|| "key-editor-recorder".into())
                    .min_h(px(RECORDER_HEIGHT))
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .border_1()
                    .when(recording.is_some() || ring, |d| d.border_2())
                    .border_color(border)
                    .cursor_pointer()
                    .hover(|s| s.bg(c.layer(Neutral::N100)))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.start_recording(window, cx)))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if std::mem::take(&mut this.swallow) {
                            return;
                        }
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            this.start_recording(window, cx);
                        }
                    }))
                    .child(shown),
            )
            // Thụt như gợi ý dưới ô của `TextField`.
            .child(div().pl(px(HINT_INSET)).text_size(px(12.)).text_color(c.muted).child(note))
    }

    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let labels: Vec<SharedString> = Command::ALL.iter().map(|command| command.label().into()).collect();
        let warnings = self.warnings();
        let supported = global_keys::supported();
        div()
            .w(px(BODY_WIDTH))
            .flex()
            .flex_col()
            .gap_4()
            .child(self.label.clone())
            .child(
                Select::new("key-editor-command")
                    .label(t!("settings.keys.editor.command"))
                    .label_weight(FontWeight::NORMAL)
                    .selected_weight(FontWeight::NORMAL)
                    .items(labels)
                    .selected(self.command)
                    .w_full()
                    .on_change(cx.listener(|this, ix: &usize, _, cx| {
                        this.command = Some(*ix);
                        cx.notify();
                    })),
            )
            .child(self.recorder(window, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Switch::new("key-editor-global")
                            .on(self.global)
                            .label(t!("settings.keys.editor.global"))
                            .disabled(!supported)
                            .on_change(cx.listener(|this, on: &bool, _, cx| this.set_global(*on, cx))),
                    )
                    .child(div().text_size(px(12.)).text_color(c.muted).child(if supported {
                        t!("settings.keys.editor.global_hint")
                    } else {
                        t!("settings.keys.editor.global_unsupported")
                    })),
            )
            .children(warnings.into_iter().map(|text| warning_line(text, c.warning)))
    }

    #[cfg(test)]
    pub fn press_for_test(&mut self, keystroke: &'static str, cx: &mut Context<Self>) {
        let keystroke = Keystroke::parse(keystroke).expect("keystroke");
        self.recorder_key(&keystroke, cx);
    }

    #[cfg(test)]
    pub fn start_recording_for_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_recording(window, cx);
    }

    #[cfg(test)]
    pub fn set_for_test(&mut self, command: Command, key: &str, global: bool, cx: &mut Context<Self>) {
        self.command = Command::ALL.iter().position(|c| *c == command);
        self.key = KeySequence::parse(key).ok();
        self.global = global;
        self.refresh_probe(cx);
    }

    #[cfg(test)]
    pub fn set_label_for_test(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.to_owned();
        self.label.update(cx, |field, cx| field.set_value(text, window, cx));
    }

    #[cfg(test)]
    pub fn save_for_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }

    #[cfg(test)]
    pub fn key_text(&self) -> Option<String> {
        self.key.as_ref().map(ToString::to_string)
    }

    #[cfg(test)]
    pub fn warning_texts(&self) -> Vec<String> {
        self.warnings()
    }
}

/// Nối `chord` vào bộ ghi: phím tới trong `SEQUENCE_TIMEOUT` sau phím trước thì thành chuỗi, quá hạn hay đã đủ
/// `MAX_SEQUENCE` phím thì bắt đầu lại từ nó.
fn append_chord(chords: &mut Vec<Chord>, last: &mut Option<Instant>, chord: Chord, now: Instant) {
    let fresh = last.is_none_or(|last| now.duration_since(last) >= SEQUENCE_TIMEOUT);
    if fresh || chords.len() >= MAX_SEQUENCE {
        chords.clear();
    }
    chords.push(chord);
    *last = Some(now);
}

/// Tên hiển thị của một dòng: tên người dùng đặt, không thì tên chức năng, lệnh lạ thì tên thô.
pub fn binding_name(binding: &KeyBinding) -> String {
    if !binding.label.trim().is_empty() {
        return binding.label.trim().to_owned();
    }
    Command::from_name(&binding.command).map_or_else(|| binding.command.clone(), |c| c.label().to_owned())
}

pub fn conflict_text(conflict: Conflict, list: &[KeyBinding]) -> String {
    let other = &list[conflict.index()];
    let name = binding_name(other);
    match conflict {
        Conflict::Same { .. } => t!("settings.keys.warn.same", name = name),
        Conflict::ShadowedBy { .. } => {
            let key = KeySequence::parse(&other.key).map(|k| k.label()).unwrap_or_else(|_| other.key.clone());
            t!("settings.keys.warn.shadowed_by", key = key, name = name)
        }
        Conflict::Shadows { .. } => t!("settings.keys.warn.shadows", name = name),
    }
}

pub fn global_issue_text(issue: GlobalIssue) -> &'static str {
    match issue {
        GlobalIssue::Sequence => t!("settings.keys.warn.global_sequence"),
        GlobalIssue::NoModifier => t!("settings.keys.warn.global_modifier"),
    }
}

/// Dòng cảnh báo: icon tam giác + chữ màu cảnh báo, xuống dòng khi dài.
pub fn warning_line(text: String, color: Hsla) -> AnyElement {
    div()
        .flex()
        .items_start()
        .gap_2()
        .text_size(px(12.))
        .text_color(color)
        .child(div().pt(px(1.)).child(Icon::new(IconName::Warning).size(px(14.)).color(color)))
        .child(div().flex_1().min_w_0().child(text))
        .into_any_element()
}

impl Render for KeyEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Đóng bằng Esc / bấm nền lúc không ghi: không còn gì để huỷ ngoài bộ ghi (đã tắt).
        if !self.handle.is_open() {
            if self.recording.is_some() {
                self.stop_recording(cx);
            }
            return div().into_any_element();
        }
        let recording = self.recording.is_some();
        let title = if self.index.is_some() {
            t!("settings.keys.editor.title_edit")
        } else {
            t!("settings.keys.editor.title_add")
        };
        let actions: [AnyElement; 2] = [
            Button::new("key-editor-cancel", t!("common.cancel"))
                .text()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close(window, cx)))
                .into_any_element(),
            Button::new("key-editor-save", t!("common.save"))
                .primary()
                .disabled(self.command.is_none())
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx)))
                .into_any_element(),
        ];
        div()
            .child(
                Dialog::new("key-editor", &self.handle)
                    .title(title)
                    .title_weight(FontWeight::NORMAL)
                    .divided(true)
                    // Đang ghi: Esc là của bộ ghi, bấm nền không được đóng ngang (phím toàn cục kẹt ở trạng thái gỡ).
                    .close_on_escape(!recording)
                    .close_on_backdrop(!recording)
                    .actions(actions)
                    .child(self.body(window, cx)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse(text).unwrap()
    }

    #[test]
    fn chords_within_the_timeout_make_a_sequence() {
        let (mut chords, mut last, start) = (Vec::new(), None, Instant::now());
        append_chord(&mut chords, &mut last, chord("g"), start);
        append_chord(&mut chords, &mut last, chord("m"), start + Duration::from_millis(799));
        assert_eq!(chords, [chord("g"), chord("m")]);
        // Phím thứ ba bắt đầu lại.
        append_chord(&mut chords, &mut last, chord("ctrl+k"), start + Duration::from_millis(900));
        assert_eq!(chords, [chord("ctrl+k")]);
        // Quá hạn cũng bắt đầu lại.
        append_chord(&mut chords, &mut last, chord("x"), start + Duration::from_millis(1_700));
        assert_eq!(chords, [chord("x")]);
    }

    #[test]
    fn name_falls_back_to_the_command_then_the_raw_text() {
        let mut binding = KeyBinding::new("ctrl+k", "open_search");
        assert_eq!(binding_name(&binding), Command::OpenSearch.label());
        binding.label = "  Tìm  ".into();
        assert_eq!(binding_name(&binding), "Tìm");
        assert_eq!(binding_name(&KeyBinding::new("x", "launch_rocket")), "launch_rocket");
    }
}
