//! groove-gd1_24: bảng EQ của tab Cài đặt chung (đặc tả 1.10).
//!
//! Mẫu cài sẵn, mười thanh dọc có đường cong đè lên, bass boost. **Mọi thay đổi áp ngay** qua `Player::update_eq` (nó lưu
//! cài đặt rồi gửi engine; DSP tự trượt 10 ms nên kéo liên tục được). Bảng giữ thêm một bản `EqSettings` để vẽ: cùng một
//! phép đổi chạy trên bản đó và trên player, nên hai bên khớp mà view không phải đọc ngược cài đặt trong lúc kéo.
//!
//! Luật EQ (kẹp, thành `Custom`, bass boost) nằm ở `groove_core::settings::EqSettings`; ở đây không viết lại.

use groove_core::settings::{BASS_BOOST_MAX_DB, EQ_BANDS_HZ, EQ_GAIN_LIMIT_DB, EqPreset, EqSettings};
use groove_core::t;
use materiant::gpui_kit::{Context, FontWeight, IntoElement, ParentElement, Render, Styled, Window, div, px};
use materiant::select::Select;
use materiant::slider::Slider;
use materiant::theme::ActiveTheme;

use super::eq_curve::eq_curve;
use crate::player::player;

/// Cao thanh dọc (đủ 20 dB chia nấc 0,5 mà vẫn dễ kéo).
pub const SLIDER_HEIGHT: f32 = 140.;
/// Cao một hàng nhãn (mức trên, tần số dưới).
const LABEL_HEIGHT: f32 = 18.;
/// Nấc của thanh dải (đặc tả 1.10).
const BAND_STEP: f32 = 0.5;

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
}

impl EqPanel {
    pub fn new() -> EqPanel {
        EqPanel { eq: EqSettings::default() }
    }

    #[cfg(test)]
    pub fn eq(&self) -> &EqSettings {
        &self.eq
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

    fn preset_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = EqPreset::ALL.iter().position(|preset| *preset == self.eq.preset);
        let select = Select::new("eq-preset")
            .label(t!("settings.eq.preset"))
            .items(EqPreset::ALL.map(preset_label))
            .selected(selected)
            .on_change(cx.listener(|this, ix: &usize, _, cx| {
                if let Some(preset) = EqPreset::ALL.get(*ix).copied() {
                    this.apply(move |eq| eq.select_preset(preset), cx);
                }
            }));
        div().w(px(220.)).child(select)
    }

    fn bands(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        let columns = EQ_BANDS_HZ.iter().enumerate().map(|(band, hz)| {
            let gain = self.eq.gains[band];
            let label = band_label(*hz);
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .child(div().h(px(LABEL_HEIGHT)).text_size(px(11.)).text_color(c.text).child(gain_text(gain)))
                .child(
                    Slider::new(("eq-band", band))
                        .vertical()
                        .min(-EQ_GAIN_LIMIT_DB)
                        .max(EQ_GAIN_LIMIT_DB)
                        .step(BAND_STEP)
                        .value(gain)
                        .h(px(SLIDER_HEIGHT))
                        .accessibility_label(t!("settings.eq.band_label", hz = label.clone()))
                        .on_change(cx.listener(move |this, value: &f32, _, cx| {
                            let value = *value;
                            this.apply(move |eq| eq.set_band(band, value), cx);
                        })),
                )
                .child(div().h(px(LABEL_HEIGHT)).text_size(px(11.)).text_color(c.muted).child(label))
        });
        div()
            .relative()
            .flex()
            .w_full()
            .children(columns)
            // Đường cong vẽ sau thanh trượt (đặc tả 1.10), chỉ phủ phần thanh, không phủ hàng nhãn.
            .child(
                div()
                    .absolute()
                    .top(px(LABEL_HEIGHT))
                    .left_0()
                    .w_full()
                    .h(px(SLIDER_HEIGHT))
                    .child(eq_curve(self.eq.gains, c.primary)),
            )
    }

    fn bass_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors().clone();
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(div().w(px(110.)).child(t!("settings.eq.bass_boost")))
            .child(
                div().flex_1().child(
                    Slider::new("eq-bass-boost")
                        .min(0.)
                        .max(BASS_BOOST_MAX_DB)
                        .step(1.)
                        .value(self.eq.bass_boost)
                        .accessibility_label(t!("settings.eq.bass_boost"))
                        .on_change(cx.listener(|this, value: &f32, _, cx| {
                            let value = *value;
                            this.apply(move |eq| eq.set_bass_boost(value), cx);
                        })),
                ),
            )
            .child(div().w(px(56.)).text_color(c.muted).child(format!(
                "{} {}",
                self.eq.bass_boost,
                t!("settings.eq.gain_unit")
            )))
    }
}

impl Default for EqPanel {
    fn default() -> Self {
        EqPanel::new()
    }
}

impl Render for EqPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().colors().muted;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(t!("settings.eq.title")))
            .child(self.preset_row(cx))
            .child(self.bands(cx))
            .child(self.bass_row(cx))
            .child(div().text_size(px(12.)).text_color(muted).child(t!("settings.eq.note")))
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
