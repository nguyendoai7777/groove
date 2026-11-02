//! groove-gd1_11: biquad EQ, preamp (S23). Bass boost cộng ở `groove_core::settings::EqSettings::effective_gains`.
//!
//! Luồng decoder gọi [`Equalizer::process`] trước khi đẩy vào ring (đặc tả 5.1). `process` và `set_params` không cấp phát,
//! không khoá; chỉ `new` và `set_format` cấp phát.
//!
//! Công thức khớp `BiquadFilterNode` của Web Audio: lowshelf, highshelf bỏ qua Q (S = 1), peaking dùng Q; tần số từ Nyquist
//! trở lên thì bộ lọc thành đơn vị. Test đối chiếu số đo lấy từ `getFrequencyResponse` của Chromium, cùng lõi với WebView2
//! của bản cũ. Hệ số và trạng thái dùng `f64`: 31 Hz ở tần số mẫu cao có cực sát vòng tròn đơn vị, `f32` không đủ.

use groove_core::settings::{EQ_BAND_COUNT as BAND_COUNT, EQ_BANDS_HZ, EQ_GAIN_LIMIT_DB, EqSettings};
use std::f64::consts::PI;

const PEAKING_Q: f64 = 1.414;

/// Hằng thời gian khi đổi gain, bằng `setTargetAtTime(…, 0.01)` của bản cũ.
const SMOOTHING_TAU_SEC: f64 = 0.01;
/// Tính lại hệ số sau từng này frame khi gain còn đang trượt.
const UPDATE_FRAMES: usize = 32;
/// Cách đích dưới mức này thì nhảy luôn tới đích, không nghe được.
const SNAP_DB: f64 = 0.005;
/// Đuôi của bộ lọc đã về phẳng còn dưới mức này (-140 dB) thì mới bỏ qua được mà không lách tách.
const TAIL_EPSILON: f64 = 1e-7;
/// Trạng thái nhỏ hơn mức này thì về 0, tránh số dưới chuẩn làm chậm CPU khi im lặng.
const DENORMAL_FLOOR: f64 = 1e-30;

/// Nội dung của `AudioCmd::SetEq`: gain thật của từng dải, đã cộng bass boost (luật ở `EqSettings::effective_gains`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EqParams {
    pub gains_db: [f32; BAND_COUNT],
}

impl From<&EqSettings> for EqParams {
    fn from(settings: &EqSettings) -> Self {
        Self { gains_db: settings.effective_gains() }
    }
}

impl EqParams {
    /// Chốt chặn cuối trước biquad: kẹp ±10 dB, giá trị không hữu hạn tính là 0.
    fn clamped_gains_db(&self) -> [f64; BAND_COUNT] {
        self.gains_db
            .map(|gain| if gain.is_finite() { gain.clamp(-EQ_GAIN_LIMIT_DB, EQ_GAIN_LIMIT_DB) as f64 } else { 0.0 })
    }
}

fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// Hệ số đã chia `a0`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    const IDENTITY: Self = Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    /// Dải đầu lowshelf, dải cuối highshelf, giữa peaking.
    fn for_band(band: usize, gain_db: f64, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * EQ_BANDS_HZ[band] as f64 / sample_rate as f64;
        if gain_db == 0.0 || !w0.is_finite() || w0 >= PI {
            return Self::IDENTITY;
        }
        let a = 10f64.powf(gain_db / 40.0);
        let (sin, cos) = w0.sin_cos();
        // [b0, b1, b2, a0, a1, a2] theo Audio EQ Cookbook, cùng dạng với đặc tả Web Audio.
        let raw = if band == 0 || band == BAND_COUNT - 1 {
            // S = 1 nên alpha_S = sin / sqrt(2).
            let k = 2.0 * (sin / std::f64::consts::SQRT_2) * a.sqrt();
            let (p, m) = (a + 1.0, a - 1.0);
            if band == 0 {
                [
                    a * (p - m * cos + k),
                    2.0 * a * (m - p * cos),
                    a * (p - m * cos - k),
                    p + m * cos + k,
                    -2.0 * (m + p * cos),
                    p + m * cos - k,
                ]
            } else {
                [
                    a * (p + m * cos + k),
                    -2.0 * a * (m + p * cos),
                    a * (p + m * cos - k),
                    p - m * cos + k,
                    2.0 * (m - p * cos),
                    p - m * cos - k,
                ]
            }
        } else {
            let alpha = sin / (2.0 * PEAKING_Q);
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a]
        };
        let [b0, b1, b2, a0, a1, a2] = raw;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }

    /// Direct Form I: đổi hệ số giữa chừng không đổi trạng thái (trạng thái là mẫu thật), nên ít quá độ nhất.
    #[inline]
    fn run(&self, h: &mut History, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * h.x1 + self.b2 * h.x2 - self.a1 * h.y1 - self.a2 * h.y2;
        h.x2 = h.x1;
        h.x1 = x;
        h.y2 = h.y1;
        h.y1 = y;
        y
    }

    /// |H|² tại tần số góc `w`.
    fn power_at(&self, w: f64) -> f64 {
        // Dạng theo sin²(w/2) của Cookbook: dạng theo cos(w) mất độ chính xác ở tần số thấp, nơi mẫu số sát 0.
        let phi = (w / 2.0).sin().powi(2);
        let side = |c0: f64, c1: f64, c2: f64| {
            (c0 + c1 + c2).powi(2) - 4.0 * (c0 * c1 + 4.0 * c0 * c2 + c1 * c2) * phi + 16.0 * c0 * c2 * phi * phi
        };
        side(self.b0, self.b1, self.b2) / side(1.0, self.a1, self.a2)
    }
}

/// Đáp ứng của cả chuỗi 10 bộ lọc, chưa tính preamp, đơn vị dB.
fn chain_response_db(gains_db: &[f64; BAND_COUNT], sample_rate: u32, freq_hz: f64) -> f64 {
    let w = 2.0 * PI * freq_hz / sample_rate as f64;
    let power: f64 = gains_db
        .iter()
        .enumerate()
        .map(|(band, &gain)| Biquad::for_band(band, gain, sample_rate).power_at(w))
        .product();
    10.0 * power.log10()
}

/// Preamp tự động (S23): trừ đúng đỉnh đáp ứng thật của chuỗi. Một dải thì bằng `-max(gain dương)` của đặc tả; các dải
/// chồng lấn cộng dồn vượt gain từng dải (toàn +10 dB ra khoảng +15 dB), đỉnh thật mới giữ được không rè. Chỉ giảm, không tăng.
fn auto_preamp_db(gains_db: &[f64; BAND_COUNT], sample_rate: u32) -> f64 {
    if sample_rate == 0 || gains_db.iter().all(|&g| g <= 0.0) {
        return 0.0;
    }
    let nyquist = sample_rate as f64 / 2.0;
    // Lưới 1/6 quãng 8 từ 20 Hz, cộng tâm các dải, DC và Nyquist nơi hai shelf đạt đủ gain.
    let grid = (0..60).map(|k| 20.0 * 2f64.powf(k as f64 / 6.0));
    let points = grid.chain(EQ_BANDS_HZ.map(f64::from)).chain([0.0, nyquist]).filter(|&f| f <= nyquist);
    let peak = points.map(|f| chain_response_db(gains_db, sample_rate, f)).fold(0.0, f64::max);
    -peak
}

#[derive(Clone, Copy, Debug, Default)]
struct History {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

/// EQ 10 dải + preamp cho một luồng PCM xen kẽ `f32`. Cài phẳng thì đi tắt, trả nguyên từng bit.
pub struct Equalizer {
    sample_rate: u32,
    channels: usize,
    target_db: [f64; BAND_COUNT],
    current_db: [f64; BAND_COUNT],
    preamp_target_db: f64,
    preamp_db: f64,
    coeffs: [Biquad; BAND_COUNT],
    /// Trạng thái riêng từng kênh, mỗi kênh đủ 10 dải.
    history: Vec<[History; BAND_COUNT]>,
    /// Cài phẳng và đuôi đã tắt: không lọc, trả nguyên mẫu.
    bypassed: bool,
}

impl Equalizer {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            sample_rate,
            channels,
            target_db: [0.0; BAND_COUNT],
            current_db: [0.0; BAND_COUNT],
            preamp_target_db: 0.0,
            preamp_db: 0.0,
            coeffs: [Biquad::IDENTITY; BAND_COUNT],
            history: vec![[History::default(); BAND_COUNT]; channels],
            bypassed: true,
        }
    }

    /// Đổi cài đặt lúc đang phát: gain trượt tới đích với hằng thời gian 10 ms, không lách tách.
    pub fn set_params(&mut self, params: &EqParams) {
        self.target_db = params.clamped_gains_db();
        self.preamp_target_db = auto_preamp_db(&self.target_db, self.sample_rate);
        if self.bypassed && !self.target_is_flat() {
            self.wake();
        }
    }

    /// Đổi định dạng luồng (sang bài khác tần số mẫu hay số kênh): tính lại hệ số, xoá trạng thái, gain về thẳng đích.
    pub fn set_format(&mut self, sample_rate: u32, channels: usize) {
        if (sample_rate, channels) == (self.sample_rate, self.channels) {
            return;
        }
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.history.resize(channels, [History::default(); BAND_COUNT]);
        self.preamp_target_db = auto_preamp_db(&self.target_db, sample_rate);
        self.reset();
    }

    /// Xoá trạng thái, gain về thẳng đích. Gọi khi tua hoặc đổi bài: âm cũ không được ngân sang đoạn mới.
    pub fn reset(&mut self) {
        self.history.fill([History::default(); BAND_COUNT]);
        self.current_db = self.target_db;
        self.preamp_db = self.preamp_target_db;
        self.refresh_coeffs();
        self.bypassed = self.target_is_flat();
    }

    /// Lọc tại chỗ một khối xen kẽ. Phần lẻ cuối khối không đủ một frame giữ nguyên.
    pub fn process(&mut self, samples: &mut [f32]) {
        if self.channels == 0 {
            return;
        }
        for block in samples.chunks_mut(self.channels * UPDATE_FRAMES) {
            if self.bypassed {
                self.pass_through(block);
            } else {
                self.filter_block(block);
            }
        }
    }

    fn pass_through(&mut self, block: &mut [f32]) {
        for sample in block.iter_mut().filter(|s| !s.is_finite()) {
            *sample = 0.0;
        }
    }

    fn filter_block(&mut self, block: &mut [f32]) {
        let frames = block.len() / self.channels;
        if frames == 0 {
            return;
        }
        let gain_start = db_to_linear(self.preamp_db);
        self.advance_smoothing(frames);
        let gain_end = db_to_linear(self.preamp_db);
        let gain_step = (gain_end - gain_start) / frames as f64;

        for (i, frame) in block.chunks_exact_mut(self.channels).enumerate() {
            let gain = gain_start + gain_step * (i + 1) as f64;
            for (sample, bands) in frame.iter_mut().zip(&mut self.history) {
                // Một mẫu NaN mà lọt vào trạng thái thì kênh đó câm vĩnh viễn.
                let mut value = if sample.is_finite() { *sample as f64 } else { 0.0 };
                for (coeff, h) in self.coeffs.iter().zip(bands.iter_mut()) {
                    value = coeff.run(h, value);
                }
                *sample = (value * gain) as f32;
            }
        }
        self.flush_denormals();
        self.try_enter_bypass();
    }

    fn advance_smoothing(&mut self, frames: usize) {
        let step = 1.0 - (-(frames as f64) / (self.sample_rate as f64 * SMOOTHING_TAU_SEC)).exp();
        for band in 0..BAND_COUNT {
            if self.current_db[band] != self.target_db[band] {
                self.current_db[band] = approach(self.current_db[band], self.target_db[band], step);
                self.coeffs[band] = Biquad::for_band(band, self.current_db[band], self.sample_rate);
            }
        }
        self.preamp_db = approach(self.preamp_db, self.preamp_target_db, step);
    }

    fn refresh_coeffs(&mut self) {
        for (band, coeff) in self.coeffs.iter_mut().enumerate() {
            *coeff = Biquad::for_band(band, self.current_db[band], self.sample_rate);
        }
    }

    fn target_is_flat(&self) -> bool {
        self.target_db.iter().all(|&g| g == 0.0) && self.preamp_target_db == 0.0
    }

    /// Từ đi tắt sang lọc: hệ số lúc này sát đơn vị (x - y = 0 trong trạng thái), nên bắt đầu từ trạng thái rỗng không có cú nhảy.
    fn wake(&mut self) {
        self.history.fill([History::default(); BAND_COUNT]);
        self.bypassed = false;
    }

    fn flush_denormals(&mut self) {
        for h in self.history.iter_mut().flatten() {
            for value in [&mut h.x1, &mut h.x2, &mut h.y1, &mut h.y2] {
                if value.abs() < DENORMAL_FLOOR {
                    *value = 0.0;
                }
            }
        }
    }

    /// Về phẳng rồi vẫn phải đợi đuôi tắt hẳn, cắt ngang đuôi là một cú nhảy mẫu.
    fn try_enter_bypass(&mut self) {
        let settled = self.current_db == self.target_db && self.preamp_db == self.preamp_target_db;
        if settled
            && self.target_is_flat()
            && self
                .history
                .iter()
                .flatten()
                .all(|h| (h.x1 - h.y1).abs() < TAIL_EPSILON && (h.x2 - h.y2).abs() < TAIL_EPSILON)
        {
            self.bypassed = true;
        }
    }
}

fn approach(current: f64, target: f64, step: f64) -> f64 {
    let next = current + (target - current) * step;
    if (target - next).abs() < SNAP_DB { target } else { next }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single_band(band: usize, gain: f64) -> [f64; BAND_COUNT] {
        let mut gains = [0.0; BAND_COUNT];
        gains[band] = gain;
        gains
    }

    fn params(gains: [f32; BAND_COUNT]) -> EqParams {
        EqParams { gains_db: gains }
    }

    /// Đáp ứng đo từ `BiquadFilterNode.getFrequencyResponse` trong Chromium 152 ngày 02/11/2026, làm tròn 4 chữ số.
    /// Các tần số: 20, 31, 63, 125, 1000, 4000, 7000, 10000 Hz. `(dải, gain, tần số mẫu, dB)`.
    #[test]
    fn matches_web_audio_reference() {
        const FREQS: [f64; 8] = [20.0, 31.0, 63.0, 125.0, 1000.0, 4000.0, 7000.0, 10000.0];
        let reference: [(usize, f64, u32, [f64; 8]); 9] = [
            (0, 10.0, 44100, [8.3343, 5.0, 0.6588, 0.0464, 0.0, 0.0, 0.0, 0.0]),
            (0, -7.5, 48000, [-6.3115, -3.75, -0.4593, -0.0319, 0.0, 0.0, 0.0, 0.0]),
            (1, 10.0, 48000, [0.6966, 1.9383, 10.0, 2.0574, 0.0246, 0.0015, 0.0004, 0.0002]),
            (5, -10.0, 44100, [-0.0025, -0.0059, -0.0246, -0.098, -10.0, -0.392, -0.1082, -0.0425]),
            (5, 6.0, 96000, [0.0013, 0.0031, 0.0129, 0.0518, 6.0, 0.2185, 0.0657, 0.0306]),
            (8, 10.0, 22050, [0.0, 0.0, 0.0001, 0.0004, 0.027, 0.5912, 5.0937, 0.7084]),
            (9, 10.0, 44100, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0041, 0.0483, 0.2948]),
            (9, -10.0, 48000, [0.0, 0.0, 0.0, 0.0, 0.0, -0.0071, -0.0803, -0.4466]),
            (9, 4.5, 192000, [0.0, 0.0, 0.0, 0.0, 0.0001, 0.0168, 0.1539, 0.5834]),
        ];
        for (band, gain, rate, expected) in reference {
            for (freq, want) in FREQS.into_iter().zip(expected) {
                let got = chain_response_db(&single_band(band, gain), rate, freq);
                assert!((got - want).abs() < 0.001, "band {band} {gain} dB @{rate}, {freq} Hz: {got} != {want}");
            }
        }
    }

    #[test]
    fn band_at_or_above_nyquist_is_identity_like_web_audio() {
        assert_eq!(Biquad::for_band(8, 10.0, 16000), Biquad::IDENTITY);
        assert_eq!(Biquad::for_band(9, 10.0, 32000), Biquad::IDENTITY);
        assert_eq!(Biquad::for_band(9, 10.0, 22050), Biquad::IDENTITY);
        assert_eq!(Biquad::for_band(0, 10.0, 0), Biquad::IDENTITY);
        assert_ne!(Biquad::for_band(9, 10.0, 44100), Biquad::IDENTITY);
    }

    #[test]
    fn coefficients_depend_on_sample_rate() {
        // Cùng 1 kHz +6 dB: tính theo tần số mẫu nên đỉnh luôn nằm đúng 1 kHz.
        for rate in [22050, 44100, 48000, 96000, 192000] {
            let gains = single_band(5, 6.0);
            assert!((chain_response_db(&gains, rate, 1000.0) - 6.0).abs() < 1e-6, "{rate}");
        }
        assert_ne!(Biquad::for_band(5, 6.0, 44100), Biquad::for_band(5, 6.0, 48000));
    }

    #[test]
    fn filters_are_stable_for_every_rate_and_gain() {
        for rate in [8000, 11025, 22050, 32000, 44100, 48000, 96000, 192000, 384000] {
            for band in 0..BAND_COUNT {
                for gain in [-10.0, -3.0, 0.5, 10.0] {
                    let c = Biquad::for_band(band, gain, rate);
                    // Điều kiện tam giác Jury cho đa thức bậc hai.
                    assert!(
                        c.a2.abs() < 1.0 + 1e-12 && c.a1.abs() < 1.0 + c.a2 + 1e-12,
                        "{band} {gain} @{rate}: {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn gains_from_settings_include_bass_boost_once() {
        let settings = EqSettings {
            gains: [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            bass_boost: 5.0,
            ..EqSettings::default()
        };
        let p = EqParams::from(&settings);
        assert_eq!(p.clamped_gains_db(), [6.0, 5.0, 3.5, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn gains_are_clamped_and_sanitized() {
        let p = params([8.0, 9.5, -10.0, 25.0, -25.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.0]);
        assert_eq!(p.clamped_gains_db(), [8.0, 9.5, -10.0, 10.0, -10.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn preamp_equals_band_gain_for_single_band() {
        for band in 0..BAND_COUNT {
            let got = auto_preamp_db(&single_band(band, 10.0), 48000);
            // Shelf cao ở 48 kHz chưa đạt đủ +10 tại Nyquist nên cao hơn -10 một chút.
            assert!((-10.000001..-9.0).contains(&got), "band {band}: {got}");
        }
        assert!((auto_preamp_db(&single_band(5, 6.0), 48000) + 6.0).abs() < 1e-9);
        assert_eq!(auto_preamp_db(&[-10.0; BAND_COUNT], 48000), 0.0);
        assert_eq!(auto_preamp_db(&[0.0; BAND_COUNT], 48000), 0.0);
    }

    #[test]
    fn preamp_keeps_whole_chain_at_or_below_unity() {
        let curves: [[f64; BAND_COUNT]; 4] = [
            [10.0; BAND_COUNT],
            [10.0, 8.0, 5.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [10.0, -10.0, 10.0, -10.0, 10.0, -10.0, 10.0, -10.0, 10.0, -10.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 10.0, 10.0, 10.0, 0.0, 0.0],
        ];
        for gains in curves {
            for rate in [22050, 44100, 48000, 96000] {
                let preamp = auto_preamp_db(&gains, rate);
                let nyquist = rate as f64 / 2.0;
                let mut freq = 10.0;
                while freq < nyquist {
                    let total = chain_response_db(&gains, rate, freq) + preamp;
                    assert!(total < 0.1, "{gains:?} @{rate}, {freq} Hz: {total}");
                    freq *= 1.01;
                }
            }
        }
        // Chứng minh vì sao không dùng `-max(gain dương)`: toàn +10 dB vượt xa +10 dB.
        let peak = -auto_preamp_db(&[10.0; BAND_COUNT], 48000);
        assert!(peak > 12.0, "{peak}");
    }

    fn sine(freq: f64, rate: u32, amplitude: f64, frames: usize) -> Vec<f32> {
        (0..frames).map(|n| (amplitude * (2.0 * PI * freq * n as f64 / rate as f64).sin()) as f32).collect()
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }

    /// Đo qua `process` thật, không phát tiếng: đáp ứng đo được khớp công thức, gồm cả preamp.
    #[test]
    fn processed_sine_matches_computed_response() {
        let gains = [6.0, -4.0, 3.0, 0.0, 5.0, -8.0, 2.0, 10.0, -6.0, 7.0];
        let p = params(gains);
        let effective = p.clamped_gains_db();
        for rate in [44100, 96000] {
            let preamp = auto_preamp_db(&effective, rate);
            for freq in [40.0, 100.0, 440.0, 1000.0, 3000.0, 6000.0, 12000.0] {
                let mut eq = Equalizer::new(rate, 1);
                eq.set_params(&p);
                eq.reset();
                let mut buf = sine(freq, rate, 0.25, rate as usize);
                let input = rms(&buf[rate as usize / 2..]);
                eq.process(&mut buf);
                let measured = 20.0 * (rms(&buf[rate as usize / 2..]) / input).log10();
                let want = chain_response_db(&effective, rate, freq) + preamp;
                assert!((measured - want).abs() < 0.02, "{freq} Hz @{rate}: {measured} != {want}");
            }
        }
    }

    #[test]
    fn channels_keep_separate_state() {
        let p = params([6.0; BAND_COUNT]);
        let left = sine(440.0, 48000, 0.5, 4800);

        let mut mono = Equalizer::new(48000, 1);
        mono.set_params(&p);
        mono.reset();
        let mut mono_out = left.clone();
        mono.process(&mut mono_out);

        let mut stereo = Equalizer::new(48000, 2);
        stereo.set_params(&p);
        stereo.reset();
        let mut interleaved: Vec<f32> = left.iter().flat_map(|&s| [s, 0.0]).collect();
        stereo.process(&mut interleaved);

        for (i, frame) in interleaved.as_chunks::<2>().0.iter().enumerate() {
            assert_eq!(frame[0], mono_out[i], "frame {i}");
            assert_eq!(frame[1], 0.0, "frame {i}");
        }
    }

    #[test]
    fn flat_is_bit_exact_and_allows_odd_block_sizes() {
        let mut eq = Equalizer::new(48000, 2);
        let input: Vec<f32> = sine(997.0, 48000, 0.9, 1001);
        let mut out = input.clone();
        eq.process(&mut out);
        assert_eq!(out, input);
        assert!(eq.bypassed);
    }

    /// Đạo hàm bậc hai lớn nhất: sin thấp tần gần như bằng 0, cú nhảy mẫu hiện rõ.
    fn max_curvature(samples: &[f32]) -> f64 {
        samples.windows(3).map(|w| (w[0] as f64 - 2.0 * w[1] as f64 + w[2] as f64).abs()).fold(0.0, f64::max)
    }

    #[test]
    fn gain_change_does_not_click() {
        const RATE: u32 = 48000;
        let from = params([-10.0; BAND_COUNT]);
        let to = params([10.0; BAND_COUNT]);
        let run = |hard_jump: bool| {
            let mut eq = Equalizer::new(RATE, 1);
            eq.set_params(&from);
            eq.reset();
            let mut buf = sine(100.0, RATE, 0.25, 9600);
            let (head, tail) = buf.split_at_mut(4800);
            eq.process(head);
            eq.set_params(&to);
            if hard_jump {
                eq.current_db = eq.target_db;
                eq.preamp_db = eq.preamp_target_db;
                eq.refresh_coeffs();
            }
            eq.process(tail);
            max_curvature(&buf[4790..])
        };
        let (smooth, hard) = (run(false), run(true));
        // Test có sức phân biệt: nhảy gain thẳng phải tệ hơn hẳn.
        assert!(hard > smooth * 10.0, "hard {hard}, smooth {smooth}");
        assert!(smooth < 1e-3, "smooth {smooth}");
    }

    #[test]
    fn gain_follows_ten_millisecond_time_constant() {
        const RATE: u32 = 48000;
        let mut eq = Equalizer::new(RATE, 1);
        eq.set_params(&params(single_band_f32(5, -10.0)));
        let mut buf = vec![0.0f32; 480]; // đúng 10 ms
        eq.process(&mut buf);
        // Sau một hằng thời gian còn e⁻¹ quãng đường.
        assert!((eq.current_db[5] + 10.0 * (1.0 - (-1f64).exp())).abs() < 0.01, "{}", eq.current_db[5]);
        let mut buf = vec![0.0f32; 4800];
        eq.process(&mut buf);
        assert_eq!(eq.current_db[5], -10.0);
    }

    fn single_band_f32(band: usize, gain: f32) -> [f32; BAND_COUNT] {
        let mut gains = [0.0; BAND_COUNT];
        gains[band] = gain;
        gains
    }

    #[test]
    fn leaving_and_entering_bypass_is_seamless() {
        const RATE: u32 = 48000;
        // Bật EQ đúng lúc sin đạt đỉnh, nơi cú nhảy mẫu lớn nhất: 12 120 frame = 25,25 chu kỳ. Ngưỡng 20 lần: preamp bắt đầu
        // trượt nên độ dốc đổi (bản chất của `setTargetAtTime`), là gãy độ dốc nhỏ, không phải nhảy mẫu.
        const WAKE_AT: usize = 12120;
        let source = sine(100.0, RATE, 0.25, 96000);
        let baseline = max_curvature(&source);
        let mut buf = source.clone();
        let mut eq = Equalizer::new(RATE, 1);
        let (bypass_head, rest) = buf.split_at_mut(WAKE_AT);
        eq.process(bypass_head);
        assert!(eq.bypassed);

        eq.set_params(&params(single_band_f32(1, 10.0)));
        assert!(!eq.bypassed);
        let (active, rest) = rest.split_at_mut(19200);
        eq.process(active);

        // Về phẳng: trượt, đợi đuôi tắt, rồi mới đi tắt lại.
        eq.set_params(&EqParams::default());
        eq.process(rest);
        assert!(eq.bypassed);
        let wake = max_curvature(&buf[WAKE_AT - 8..WAKE_AT + 64]);
        assert!(wake < baseline * 20.0, "wake {wake} vs {baseline}");
        let leave = max_curvature(&buf[WAKE_AT + 19200..]);
        assert!(leave < baseline * 3.0, "leave {leave} vs {baseline}");
        // Đoạn cuối đã đi tắt thì trùng từng bit với tín hiệu gốc.
        assert_eq!(buf[90000..], source[90000..]);
    }

    #[test]
    fn reset_forgets_previous_audio() {
        let p = params([10.0; BAND_COUNT]);
        let mut used = Equalizer::new(48000, 1);
        used.set_params(&p);
        let mut noise = sine(50.0, 48000, 1.0, 48000);
        used.process(&mut noise);
        used.reset();

        let mut fresh = Equalizer::new(48000, 1);
        fresh.set_params(&p);
        fresh.reset();

        let (mut a, mut b) = (sine(440.0, 48000, 0.5, 2000), sine(440.0, 48000, 0.5, 2000));
        used.process(&mut a);
        fresh.process(&mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn set_format_recomputes_for_new_stream() {
        let p = params(single_band_f32(5, 6.0));
        let mut eq = Equalizer::new(44100, 2);
        eq.set_params(&p);
        eq.set_format(96000, 1);
        assert_eq!(eq.coeffs[5], Biquad::for_band(5, 6.0, 96000));
        assert_eq!(eq.history.len(), 1);
        assert_eq!(eq.current_db, eq.target_db);
        let mut buf = sine(1000.0, 96000, 0.25, 96000);
        let input = rms(&buf[48000..]);
        eq.process(&mut buf);
        assert!((rms(&buf[48000..]) / input - 1.0).abs() < 0.01, "6 dB band, -6 dB preamp");

        // Luồng hỏng không được sập.
        let mut degenerate = Equalizer::new(0, 0);
        degenerate.set_params(&p);
        degenerate.process(&mut [0.5, 0.5]);
        degenerate.set_format(0, 2);
        degenerate.process(&mut [0.5, 0.5]);
    }

    #[test]
    fn bad_samples_do_not_poison_state() {
        let mut eq = Equalizer::new(48000, 1);
        eq.set_params(&params([6.0; BAND_COUNT]));
        let mut buf = sine(440.0, 48000, 0.5, 4800);
        buf[100] = f32::NAN;
        buf[200] = f32::INFINITY;
        buf[300] = f32::NEG_INFINITY;
        eq.process(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
        assert!(rms(&buf[2400..]) > 0.01);

        let mut flat = Equalizer::new(48000, 1);
        let mut buf = [0.1, f32::NAN];
        flat.process(&mut buf);
        assert_eq!(buf, [0.1, 0.0]);
    }

    #[test]
    fn silence_decays_to_exact_zero() {
        let mut eq = Equalizer::new(48000, 2);
        eq.set_params(&params([10.0; BAND_COUNT]));
        let mut music = sine(50.0, 48000, 1.0, 48000);
        eq.process(&mut music);
        let mut silence = vec![0.0f32; 48000 * 30];
        eq.process(&mut silence);
        assert!(eq.history.iter().flatten().all(|h| [h.x1, h.x2, h.y1, h.y2] == [0.0; 4]));
        assert!(silence[silence.len() - 100..].iter().all(|&s| s == 0.0));
    }
}
