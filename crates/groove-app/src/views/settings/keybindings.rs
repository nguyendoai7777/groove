//! groove-gd1_24: tab Phím tắt (đặc tả 1.10): ô JSON `[{key, command}]`, Đặt lại mặc định, danh sách lệnh dùng được.
//!
//! Chữ trong ô là **bản nháp**: chỉ nút OK của hộp mới lưu (`check` rồi `app::update_settings`); đóng kiểu khác bỏ nháp (M3).
//! Soát hai tầng của `groove_core::keymap`: `parse_json` (hình dạng), `validate` (nội dung, M5). Lỗi đầu tiên được báo bằng
//! chữ `settings.keys.error.*`, kèm số phần tử (đếm từ 1, như người đọc).
//!
//! Không làm: tự đóng ngoặc, gợi ý tên lệnh khi gõ `"command":` (ô nhiều dòng của materiant chưa có chỗ chèn gợi ý); thay
//! vào đó liệt kê mọi lệnh dưới ô (`plan/confirm.md`).

use groove_core::keymap::{self, Command, KeyIssue, KeyJsonError};
use groove_core::settings::{KeyBinding, Settings, default_keybindings};
use groove_core::t;
use materiant::badge::Badge;
use materiant::button::Button;
use materiant::field::textarea::TextArea;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    App, ClickEvent, Context, Entity, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};
use materiant::theme::ActiveTheme;

/// Số dòng của ô JSON.
const ROWS: usize = 12;

/// Kết quả soát bản nháp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
    /// Giống danh sách đang có: không ghi gì.
    Unchanged,
    /// Đúng bằng mặc định: ghi `None` để phím mới của bản sau tới được người dùng.
    Defaults,
    Custom(Vec<KeyBinding>),
}

/// Chữ báo cho lỗi hình dạng.
pub fn json_error_text(error: &KeyJsonError) -> String {
    match error {
        KeyJsonError::Empty => t!("settings.keys.error.empty").to_owned(),
        KeyJsonError::Syntax { line, column } => t!("settings.keys.error.syntax", line = line, column = column),
        KeyJsonError::NotArray => t!("settings.keys.error.not_array").to_owned(),
        KeyJsonError::MissingField { index } => t!("settings.keys.error.missing_field", number = index + 1),
    }
}

/// Chữ báo cho lỗi nội dung; phần tử đếm từ 1.
pub fn issue_text(issue: &KeyIssue) -> String {
    let number = issue.index() + 1;
    match issue {
        KeyIssue::UnknownCommand { command, .. } => {
            t!("settings.keys.error.unknown_command", number = number, command = command)
        }
        KeyIssue::InvalidKey { key, .. } => t!("settings.keys.error.invalid_key", number = number, key = key),
        KeyIssue::DuplicateKey { first, key, .. } => {
            t!("settings.keys.error.duplicate", number = number, key = key, first = first + 1)
        }
        KeyIssue::UnreachableSequence { prefix, key, .. } => {
            t!("settings.keys.error.unreachable", number = number, key = key, prefix = prefix + 1)
        }
    }
}

/// Soát chữ trong ô. `current` là danh sách đang có hiệu lực (để biết có đổi không).
pub fn check_text(text: &str, current: &[KeyBinding]) -> Result<Draft, String> {
    let list = keymap::parse_json(text).map_err(|error| json_error_text(&error))?;
    if let Some(issue) = keymap::validate(&list).first() {
        return Err(issue_text(issue));
    }
    Ok(if list == current {
        Draft::Unchanged
    } else if list == default_keybindings() {
        Draft::Defaults
    } else {
        Draft::Custom(list)
    })
}

pub struct KeybindingsPanel {
    area: Entity<TextArea>,
    /// Danh sách có hiệu lực lúc mở hộp.
    current: Vec<KeyBinding>,
    /// Đang hiện lời báo lỗi dưới ô.
    error: bool,
}

impl KeybindingsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> KeybindingsPanel {
        let this = cx.entity().downgrade();
        let area = cx.new(|cx| {
            let mut area = TextArea::new("settings-keybindings", window, cx)
                .rows(ROWS)
                .hint(t!("settings.keys.hint"))
                .on_change(move |_, _, cx| {
                    // Sửa tiếp là lời báo lỗi cũ hết đúng.
                    this.update(cx, |this, cx| this.clear_error(cx)).ok();
                });
            area.set_accessibility_label(t!("settings.keys.area_label"), cx);
            area
        });
        KeybindingsPanel { area, current: default_keybindings(), error: false }
    }

    /// Nạp danh sách đang có hiệu lực vào ô (lúc mở hộp).
    pub fn show(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        self.current = settings.keybindings();
        self.error = false;
        let text = keymap::to_json(&self.current);
        self.area.update(cx, |area, cx| {
            area.set_value(text, window, cx);
            area.set_error(None, cx);
        });
    }

    /// Chữ trong ô.
    pub fn text(&self, cx: &App) -> SharedString {
        self.area.read(cx).value(cx)
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.into();
        self.area.update(cx, |area, cx| area.set_value(text, window, cx));
        self.clear_error(cx);
    }

    /// Gõ `text` vào ô như người dùng (thay cả ô): đi qua `on_change` thật.
    #[cfg(test)]
    pub fn type_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.to_owned();
        let state = self.area.read(cx).input_state().clone();
        state.update(cx, |state, cx| state.replace_all(text, window, cx));
    }

    #[cfg(test)]
    /// Lỗi đang hiện dưới ô (cho test).
    pub fn has_error(&self) -> bool {
        self.error
    }

    /// Soát bản nháp; lỗi thì hiện dưới ô và trả `Err`.
    pub fn check(&mut self, cx: &mut Context<Self>) -> Result<Draft, ()> {
        let text = self.text(cx);
        match check_text(&text, &self.current) {
            Ok(draft) => Ok(draft),
            Err(message) => {
                self.error = true;
                self.area.update(cx, |area, cx| area.set_error(Some(message.into()), cx));
                Err(())
            }
        }
    }

    fn clear_error(&mut self, cx: &mut Context<Self>) {
        if self.error {
            self.error = false;
            // Có thể đang chạy ngay trong `on_change` của chính ô (đang bị mượn): hoãn.
            let area = self.area.clone();
            cx.defer(move |cx| area.update(cx, |area, cx| area.set_error(None, cx)));
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_text(keymap::to_json(&default_keybindings()), window, cx);
    }
}

impl Render for KeybindingsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().colors().muted;
        let commands = Command::ALL.iter().map(|command| Badge::new(command.name()).outline());
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.area.clone())
            .child(
                div().flex().justify_end().child(
                    Button::new("settings-keys-reset", t!("settings.keys.reset"))
                        .outlined()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.reset(window, cx))),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(t!("settings.keys.commands")))
                    .child(div().flex().flex_wrap().gap_2().text_size(px(12.)).text_color(muted).children(commands)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(key: &str, command: &str) -> KeyBinding {
        KeyBinding { key: key.to_owned(), command: command.to_owned() }
    }

    fn json(list: &[KeyBinding]) -> String {
        keymap::to_json(list)
    }

    /// Mặc định tới đúng nó: không đổi. Bằng mặc định mà đang là bản tuỳ biến: về `None`.
    #[test]
    fn unchanged_defaults_and_custom_are_told_apart() {
        let defaults = default_keybindings();
        assert_eq!(check_text(&json(&defaults), &defaults), Ok(Draft::Unchanged));
        let custom = vec![binding("ctrl+k", "open_search")];
        assert_eq!(check_text(&json(&defaults), &custom), Ok(Draft::Defaults));
        assert_eq!(check_text(&json(&custom), &defaults), Ok(Draft::Custom(custom.clone())));
        assert_eq!(check_text("[]", &defaults), Ok(Draft::Custom(vec![])), "bỏ hết phím là việc được phép");
    }

    /// M5 — mỗi dạng lỗi ra một lời báo riêng, nhắc đúng thứ gây lỗi; không dạng nào ra chuỗi rỗng hay khoá trần.
    /// Không đặt ngôn ngữ toàn tiến trình: chỉ so phần không dịch (tên lệnh, phím, số).
    #[test]
    fn every_failure_has_its_own_message() {
        let defaults = default_keybindings();
        let cases = [
            ("   ", ""),
            ("[{\"key\": ", "1"),
            ("{\"a\": 1}", "["),
            ("[{\"key\": \"a\"}]", "1"),
            ("[{\"key\": \"a\", \"command\": \"nope\"}]", "nope"),
            ("[{\"key\": \"ctrl+\", \"command\": \"toggle_play\"}]", "ctrl+"),
            ("[{\"key\": \"a\", \"command\": \"toggle_play\"}, {\"key\": \"A\", \"command\": \"next_track\"}]", "a"),
            (
                "[{\"key\": \"g\", \"command\": \"toggle_play\"}, {\"key\": \"g m\", \"command\": \"next_track\"}]",
                "g m",
            ),
        ];
        let mut seen = Vec::new();
        for (text, expect) in cases {
            let message = check_text(text, &defaults).expect_err(text);
            assert!(message.contains(expect), "{text:?} -> {message}");
            assert!(!message.starts_with("settings."), "khoá trần lọt ra: {message}");
            assert!(!seen.contains(&message), "hai lỗi cùng một lời báo: {message}");
            seen.push(message);
        }
    }

    /// Phần tử sai nằm sau phần tử đúng: số báo là vị trí thật, đếm từ 1.
    #[test]
    fn element_number_counts_from_one() {
        let text = "[{\"key\": \"a\", \"command\": \"toggle_play\"}, {\"key\": \"b\", \"command\": \"zzz\"}]";
        let message = check_text(text, &[]).unwrap_err();
        assert!(message.contains('2') && message.contains("zzz"), "{message}");
    }
}
