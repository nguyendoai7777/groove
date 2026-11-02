//! groove-gd1_24: bảng EQ của tab Cài đặt chung (đặc tả 1.10); dáng theo bản web cũ (`setting-general.vue`).
//!
//! Mẫu cài sẵn, thẻ bass boost, mười cột dọc có đường cong nối các núm. **Mọi thay đổi áp ngay** qua `Player::update_eq` (nó
//! lưu cài đặt rồi gửi engine; DSP tự trượt 10 ms nên kéo liên tục được). Bảng giữ thêm một bản `EqSettings` để vẽ: cùng một
//! phép đổi chạy trên bản đó và trên player, nên hai bên khớp mà view không phải đọc ngược cài đặt trong lúc kéo.
//!
//! Thanh dải và thanh bass tự vẽ bằng `canvas` (xem `eq_bands`): `materiant::slider` cố định rail 4 px, núm 16 px, vầng 34 px
//! nên không ra được cột dày, núm 14 px của bản web. Bàn phím: bảng có một điểm dừng Tab, ← → chọn dải, ↑ ↓ đổi 0,5 dB,
//! PageUp / PageDown 5 dB, Home / End về hai đầu.
//!
//! Luật EQ (kẹp, thành `Custom`, bass boost) nằm ở `groove_core::settings::EqSettings`; ở đây không viết lại.

use groove_core::settings::{EQ_BAND_COUNT, EQ_BANDS_HZ, EQ_GAIN_LIMIT_DB, EqPreset, EqSettings};
use groove_core::t;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Window, div, px,
};
use materiant::select::Select;
use materiant::theme::ActiveTheme;

use super::eq_bands::{BASS_HIT_HEIGHT, Drag, bands_canvas, bass_canvas};
use crate::player::player;

/// Cao vùng thanh dọc (đủ 20 dB chia nấc 0,5 mà vẫn dễ kéo; bản web 300 px gồm cả nhãn).
pub const BAND_PLOT_HEIGHT: f32 = 240.;
/// Cao một hàng nhãn (mức trên, tần số dưới).
const LABEL_HEIGHT: f32 = 22.;
/// Lề trong của khung mười cột.
const PLOT_PAD: f32 = 10.;
/// Nấc của thanh dải (đặc tả 1.10) và của thanh bass.
pub const BAND_STEP: f32 = 0.5;
pub const BASS_STEP: f32 = 1.;
/// PageUp / PageDown.
const PAGE_DB: f32 = 5.;
/// Rộng thanh bass (web: `w-44` trừ ô giá trị).
const BASS_TRACK_WIDTH: f32 = 140.;

/// Nhãn tần số dưới thanh: `31`, `500`, `1k`, `16k`.
pub fn band_label(hz: u32) -> String {
    if hz >= 1_000 { format!("{}k", hz / 1_000) } else { hz.to_string() }
}

/// Nhãn mức trên thanh: `+3.5`, `-1`, `0`.
pub fn gain_text(gain_db: f32) -> String {
    if gain_db == 0. {
        "0".to_owned()
    } else if gain_db.fract() == 0. {
        format!("{gain_db:+.0}")
    } else {
        format!("{gain_db:+.1}")
    }
}

pub fn preset_label(preset: EqPreset) -> &'static str {
    match preset {
        EqPreset::Custom => t!("settings.preset.custom"),
        EqPreset::Flat => t!("settings.preset.flat"),
        EqPreset::BassStrong => t!("settings.preset.bass_strong"),
        EqPreset::BassModerate => t!("settings.preset.bass_moderate"),
        EqPreset::Pop => t!("settings.preset.pop"),
        EqPreset::Rock => t!("settings.preset.rock"),
        EqPreset::Electronic => t!("settings.preset.electronic"),
        EqPreset::Vocal => t!("settings.preset.vocal"),
        EqPreset::Treble => t!("settings.preset.treble"),
    }
}

pub struct EqPanel {
    eq: EqSettings,
    /// Cột / thanh bass đang kéo.
    drag: Option<Drag>,
    /// Cột chuột đang rê qua, và thanh bass có đang được rê không.
    hover: Option<usize>,
    bass_hover: bool,
    /// Điểm dừng Tab của mười cột và dải đang được phím tác động.
    focus: FocusHandle,
    key_band: usize,
}

impl EqPanel {
    pub fn new(cx: &mut Context<Self>) -> EqPanel {
        EqPanel {
            eq: EqSettings::default(),
            drag: None,
            hover: None,
            bass_hover: false,
            focus: cx.focus_handle().tab_stop(true),
            key_band: 0,
        }
    }

    #[cfg(test)]
    pub fn eq(&self) -> &EqSettings {
        &self.eq
    }

    pub fn gains(&self) -> &[f32; EQ_BAND_COUNT] {
        &self.eq.gains
    }

    pub fn bass_boost(&self) -> f32 {
        self.eq.bass_boost
    }

    pub fn drag(&self) -> Option<Drag> {
        self.drag
    }

    pub fn hover(&self) -> Option<usize> {
        self.hover
    }

    pub fn bass_hover(&self) -> bool {
        self.bass_hover
    }

    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    pub fn key_band(&self) -> usize {
        self.key_band
    }

    pub fn set_drag(&mut self, drag: Option<Drag>, cx: &mut Context<Self>) {
        if self.drag != drag {
            self.drag = drag;
            cx.notify();
        }
    }

    pub fn set_hover(&mut self, band: Option<usize>, bass: bool, cx: &mut Context<Self>) {
        if self.hover != band || self.bass_hover != bass {
            self.hover = band;
            self.bass_hover = bass;
            cx.notify();
        }
    }

    pub fn set_key_band(&mut self, band: usize, cx: &mut Context<Self>) {
        self.key_band = band.min(EQ_BANDS_HZ.len() - 1);
        cx.notify();
    }

    /// Đặt một dải về `value` dB nếu khác giá trị đang có (kéo chuột bắn nhiều sự kiện cùng nấc).
    pub fn set_band(&mut self, band: usize, value: f32, cx: &mut Context<Self>) {
        if self.eq.gains.get(band).is_some_and(|gain| *gain != value) {
            self.apply(move |eq| eq.set_band(band, value), cx);
        }
    }

    pub fn set_bass(&mut self, value: f32, cx: &mut Context<Self>) {
        if self.eq.bass_boost != value {
            self.apply(move |eq| eq.set_bass_boost(value), cx);
        }
    }

    /// Nạp giá trị để hiển thị (lúc mở hộp). Không đụng player: giá trị đã là cái player đang giữ.
    pub fn show(&mut self, eq: EqSettings, cx: &mut Context<Self>) {
        self.eq = eq;
        cx.notify();
    }

    /// Một phép đổi EQ: chạy trên bản hiển thị và gửi cho player (lưu + tới engine).
    pub fn apply(&mut self, change: impl Fn(&mut EqSettings), cx: &mut Context<Self>) {
        change(&mut self.eq);
        player(cx).update(cx, |player, cx| player.update_eq(cx, &change));
        cx.notify();
    }

    /// Trả EQ về `eq` (Huỷ): hiển thị và player cùng về giá trị đó.
    pub fn restore(&mut self, eq: EqSettings, cx: &mut Context<Self>) {
        self.apply(|current| *current = eq.clone(), cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let band = self.key_band;
        let gain = self.eq.gains[band];
        let last = EQ_BANDS_HZ.len() - 1;
        let target = match event.keystroke.key.as_str() {
            "left" => return self.step_focus(band.saturating_sub(1), cx),
            "right" => return self.step_focus((band + 1).min(last), cx),
            "up" => gain + BAND_STEP,
            "down" => gain - BAND_STEP,
            "pageup" => gain + PAGE_DB,
            "pagedown" => gain - PAGE_DB,
            "home" => -EQ_GAIN_LIMIT_DB,
            "end" => EQ_GAIN_LIMIT_DB,
            _ => return,
        };
        cx.stop_propagation();
        self.set_band(band, target.clamp(-EQ_GAIN_LIMIT_DB, EQ_GAIN_LIMIT_DB), cx);
    }

    fn step_focus(&mut self, band: usize, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.set_key_band(band, cx);
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let selected = EqPreset::ALL.iter().position(|preset| *preset == self.eq.preset);
        let select = Select::new("eq-preset")
            .label_weight(materiant::gpui_kit::FontWeight::NORMAL)
            .selected_weight(materiant::gpui_kit::FontWeight::NORMAL)
            .items(EqPreset::ALL.map(preset_label))
            .selected(selected)
            .on_change(cx.listener(|this, ix: &usize, _, cx| {
                if let Some(preset) = EqPreset::ALL.get(*ix).copied() {
                    this.apply(move |eq| eq.select_preset(preset), cx);
                }
            }));
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_sm().text_color(c.muted).child(t!("settings.eq.title")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_size(px(12.)).text_color(c.muted).child(t!("settings.eq.preset")))
                    .child(div().w(px(150.)).child(select)),
            )
    }

    fn bass_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let view = cx.entity();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .px_3()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(c.border.opacity(0.5))
            .bg(c.text.opacity(0.04))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_sm()
                            .text_color(c.text)
                            .child(div().size(px(6.)).rounded_full().bg(c.primary))
                            .child(t!("settings.eq.bass_boost")),
                    )
                    .child(div().text_size(px(11.)).text_color(c.muted).child(t!("settings.eq.bass_boost_hint"))),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_3()
                    .child(div().w(px(BASS_TRACK_WIDTH)).h(px(BASS_HIT_HEIGHT)).child(bass_canvas(view)))
                    .child(div().w(px(48.)).flex().justify_end().text_sm().text_color(c.primary).child(format!(
                        "+{}{}",
                        self.eq.bass_boost,
                        t!("settings.eq.gain_unit")
                    ))),
            )
    }

    fn bands(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let view = cx.entity();
        let columns = EQ_BANDS_HZ.iter().enumerate().map(|(band, hz)| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .h(px(LABEL_HEIGHT))
                        .flex()
                        .items_center()
                        .text_size(px(11.))
                        .text_color(c.text.opacity(0.8))
                        .child(gain_text(self.eq.gains[band])),
                )
                .child(div().h(px(BAND_PLOT_HEIGHT)))
                .child(
                    div()
                        .h(px(LABEL_HEIGHT))
                        .flex()
                        .items_center()
                        .text_size(px(11.))
                        .text_color(c.muted)
                        .child(band_label(*hz)),
                )
        });
        div()
            .id("eq-bands")
            .relative()
            .flex()
            .w_full()
            .px(px(PLOT_PAD))
            .py(px(PLOT_PAD))
            .rounded_xl()
            .border_1()
            .border_color(c.border.opacity(0.4))
            .bg(c.text.opacity(0.03))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.on_key(event, cx)))
            .children(columns)
            .child(
                div()
                    .absolute()
                    .top(px(PLOT_PAD + LABEL_HEIGHT))
                    .left(px(PLOT_PAD))
                    .right(px(PLOT_PAD))
                    .h(px(BAND_PLOT_HEIGHT))
                    .child(bands_canvas(view)),
            )
    }
}

impl Render for EqPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap_3().child(self.header(cx)).child(self.bass_card(cx)).child(self.bands(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_labels_use_k_from_one_thousand() {
        let labels: Vec<String> = EQ_BANDS_HZ.iter().map(|hz| band_label(*hz)).collect();
        assert_eq!(labels, ["31", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"]);
    }

    #[test]
    fn gain_text_shows_sign_and_trims_zero() {
        assert_eq!(gain_text(0.), "0");
        assert_eq!(gain_text(3.), "+3");
        assert_eq!(gain_text(-1.5), "-1.5");
        assert_eq!(gain_text(10.), "+10");
        assert_eq!(gain_text(0.5), "+0.5");
    }

    #[test]
    fn every_preset_has_a_label_in_both_languages() {
        // Không đổi ngôn ngữ toàn tiến trình trong test (từng gây test chập chờn): chỉ soát khoá có chữ.
        for preset in EqPreset::ALL {
            let key = match preset {
                EqPreset::Custom => "settings.preset.custom",
                EqPreset::Flat => "settings.preset.flat",
                EqPreset::BassStrong => "settings.preset.bass_strong",
                EqPreset::BassModerate => "settings.preset.bass_moderate",
                EqPreset::Pop => "settings.preset.pop",
                EqPreset::Rock => "settings.preset.rock",
                EqPreset::Electronic => "settings.preset.electronic",
                EqPreset::Vocal => "settings.preset.vocal",
                EqPreset::Treble => "settings.preset.treble",
            };
            assert_ne!(preset_label(preset), key, "{preset:?} chưa có chữ");
        }
    }
}
