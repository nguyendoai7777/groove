//! groove-gd1_24: tab Cài đặt chung (đặc tả 1.10): giao diện — nền sáng / tối / hệ thống (nút segmented), bảng màu (preset của materiant, dạng thẻ như
//! trang theme của gallery), font chữ (D-20), cả ba áp ngay, Huỷ trả về —, bước tua, bước âm lượng, trình phát mặc định
//! (gd2_01). Bảng EQ ở tab riêng (`EqPanel`).
//!
//! Hai bước áp ngay như EQ (người dùng thấy tác dụng ở phím tua, bánh xe ngay khi gõ); Huỷ do hộp trả về giá trị lúc mở.
//! Ô trống hay ngoài khoảng chưa phải giá trị: bỏ qua, ô tự kẹp lại khi mất focus.

use groove_core::settings::{Settings, ThemeChoice, UiFont};
use groove_core::t;
use materiant::button::segmented::SegmentedButtons;
use materiant::field::FieldLabel;
use materiant::field::number::NumberField;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{Context, Entity, FontWeight, IntoElement, ParentElement, Render, Styled, Window, div, px};
use materiant::select::Select;
use materiant::theme::picker::ThemePicker;

use super::default_player::{Backend, DefaultPlayerPanel};
use crate::app;

/// Khoảng hợp lệ của bước tua, giây (`Settings::sanitize` chỉ chặn dưới; trần để ô không nhận số vô lý).
pub const SEEK_STEP_RANGE: std::ops::RangeInclusive<u32> = 1..=3600;
/// Khoảng hợp lệ của bước âm lượng (đặc tả 1.10).
pub const VOLUME_STEP_RANGE: std::ops::RangeInclusive<u8> = 1..=100;

pub struct GeneralPanel {
    seek_step: Entity<NumberField>,
    volume_step: Entity<NumberField>,
    default_player: Entity<DefaultPlayerPanel>,
}

impl GeneralPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> GeneralPanel {
        let seek_step = cx.new(|cx| {
            NumberField::new("settings-seek-step", window, cx)
                .label(t!("settings.general.seek_step"))
                .label_weight(materiant::gpui_kit::FontWeight::NORMAL)
                .hint(t!("settings.general.seek_step_hint"))
                .range(*SEEK_STEP_RANGE.start()..=*SEEK_STEP_RANGE.end())
                .step(1)
                .on_change(|value, _, cx| {
                    let Some(value) = value.map(|v| v as u32).filter(|v| SEEK_STEP_RANGE.contains(v)) else { return };
                    if app::settings(cx).seek_step != value {
                        app::update_settings(cx, |s| s.seek_step = value);
                    }
                })
        });
        let volume_step = cx.new(|cx| {
            NumberField::new("settings-volume-step", window, cx)
                .label(t!("settings.general.volume_step"))
                .label_weight(materiant::gpui_kit::FontWeight::NORMAL)
                .hint(t!("settings.general.volume_step_hint"))
                .range(*VOLUME_STEP_RANGE.start()..=*VOLUME_STEP_RANGE.end())
                .step(1)
                .on_change(|value, _, cx| {
                    let Some(value) = value.map(|v| v as u8).filter(|v| VOLUME_STEP_RANGE.contains(v)) else { return };
                    if app::settings(cx).volume_step != value {
                        app::update_settings(cx, |s| s.volume_step = value);
                    }
                })
        });
        let default_player = cx.new(|cx| DefaultPlayerPanel::new(Backend::SYSTEM, window, cx));
        GeneralPanel { seek_step, volume_step, default_player }
    }

    /// Nạp giá trị hiện có vào các ô (lúc mở hộp, và sau khi Huỷ trả về).
    pub fn show(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        let (seek, volume) = (f64::from(settings.seek_step), f64::from(settings.volume_step));
        self.seek_step.update(cx, |field, cx| field.set_value(seek, window, cx));
        self.volume_step.update(cx, |field, cx| field.set_value(volume, window, cx));
        self.default_player.update(cx, |panel, cx| panel.refresh(cx));
    }
}

#[cfg(test)]
use materiant::gpui_kit::App;

#[cfg(test)]
impl GeneralPanel {
    /// Số đang hiện trong ô bước tua.
    pub fn seek_step_value(&self, cx: &App) -> Option<f64> {
        self.seek_step.read(cx).value(cx)
    }

    /// Gõ `text` vào ô bước tua như người dùng (thay cả ô), đi qua đúng `on_change` thật.
    pub fn type_seek_step(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.to_owned();
        let state = self.seek_step.read(cx).input_state().clone();
        state.update(cx, |state, cx| state.replace_all(text, window, cx));
    }
}

/// Thứ tự mục của ô Nền.
const THEME_CHOICES: [ThemeChoice; 3] = [ThemeChoice::Light, ThemeChoice::Dark, ThemeChoice::System];

fn theme_choice_label(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::Light => t!("settings.general.mode_light"),
        ThemeChoice::Dark => t!("settings.general.mode_dark"),
        ThemeChoice::System => t!("settings.general.mode_system"),
    }
}

/// Nhãn ô (cùng kiểu nhãn của `Select`) trên một nhóm chọn không có nhãn riêng.
fn labeled(label: &'static str, body: impl IntoElement) -> impl IntoElement {
    div().flex().flex_col().child(FieldLabel::new(label).weight(FontWeight::NORMAL)).child(body)
}

impl Render for GeneralPanel {
    /// Giao diện (nền, font, bảng màu), bước tua, bước âm lượng, trình phát mặc định xếp dọc.
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = app::settings(cx);
        let presets = app::theme_presets();
        let preset = presets.iter().position(|p| p.key == settings.theme_preset.as_str()).unwrap_or(0);
        // Nền và bảng màu là hai mục độc lập, lưu riêng (`theme`, `themePreset`): đổi cái này không đụng cái kia.
        let mode = SegmentedButtons::new("settings-theme-mode")
            .items(THEME_CHOICES.map(theme_choice_label))
            .selected(THEME_CHOICES.iter().position(|c| *c == settings.theme))
            .on_change(|ix, _, cx| {
                let choice = THEME_CHOICES[*ix];
                if app::settings(cx).theme != choice {
                    app::update_settings(cx, |s| s.theme = choice);
                }
            });
        // Tên font là tên riêng, hiện nguyên văn ở cả hai ngôn ngữ; mỗi mục vẽ bằng chính font của nó thì đẹp hơn nhưng
        // `Select` chưa nhận mục tự vẽ.
        let font = Select::new("settings-font")
            .label(t!("settings.general.font"))
            .label_weight(FontWeight::NORMAL)
            .items(UiFont::ALL.map(app::font_family))
            .selected(UiFont::ALL.iter().position(|f| *f == settings.font))
            .on_change(|ix, _, cx| {
                let font = UiFont::ALL[*ix];
                if app::settings(cx).font != font {
                    app::update_settings(cx, |s| s.font = font);
                }
            })
            .w(px(240.));
        let picker = ThemePicker::new("settings-theme-preset")
            .items(presets.iter().cloned())
            .selected(preset)
            .on_change(move |ix, _, cx| {
                let key = presets[*ix].key.to_string();
                if app::settings(cx).theme_preset != key {
                    app::update_settings(cx, |s| s.theme_preset = key);
                }
            });
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().child(t!("settings.general.appearance")))
            .child(labeled(t!("settings.general.mode"), mode))
            .child(labeled(t!("settings.general.theme_preset"), picker))
            .child(font)
            .child(div().child(t!("settings.general.steps")))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(div().w(px(240.)).child(self.seek_step.clone()))
                    .child(div().w(px(240.)).child(self.volume_step.clone())),
            )
            .child(self.default_player.clone())
    }
}
