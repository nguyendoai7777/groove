//! groove-gd1_02: settings.json.
//!
//! Tải, lưu chạm đĩa: UI thread chỉ gọi `load` trước khi mở cửa sổ, về sau lưu qua `JsonSaver` (N7). Tệp hỏng không làm mất
//! cả cấu hình: trường sai kiểu về mặc định từng trường, tệp không đọc được thì giữ lại thành `.bad`.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::{self, Language};
use crate::paths;

pub const EQ_BAND_COUNT: usize = 10;
/// Tần số giữa từng dải; dải đầu lowshelf, dải cuối highshelf, giữa peaking (đặc tả 1.10).
pub const EQ_BANDS_HZ: [u32; EQ_BAND_COUNT] = [31, 63, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000];
pub const EQ_GAIN_LIMIT_DB: f32 = 10.0;
pub const BASS_BOOST_MAX_DB: f32 = 10.0;
/// Bass boost cộng thêm vào 4 dải đầu theo hệ số này.
const BASS_BOOST_WEIGHTS: [f32; 4] = [1.0, 0.8, 0.5, 0.2];

/// Phím mặc định (đặc tả 1.10). Tên lệnh là chuỗi: kiểm tra và khớp phím thuộc `keymap`.
pub const DEFAULT_KEYBINDINGS: &[(&str, &str)] = &[
    ("ctrl+k", "open_search"),
    ("space", "toggle_play"),
    ("arrowleft", "seek_backward"),
    ("arrowright", "seek_forward"),
    ("alt+arrowleft", "prev_track"),
    ("alt+arrowright", "next_track"),
    ("arrowup", "volume_up"),
    ("arrowdown", "volume_down"),
    ("alt+p", "play_random"),
    ("g m", "go_to_library"),
    ("g p", "go_to_now_playing"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    Light,
    /// Bản cũ chạy tối cố định, nên mặc định giữ cảm giác đó.
    #[default]
    Dark,
    System,
}

/// Trang mở lại khi khởi động; không nhớ tham số xem danh sách (đặc tả 1.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartPage {
    #[default]
    MyMusic,
    NowPlaying,
    Playlists,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqPreset {
    Custom,
    #[default]
    Flat,
    BassStrong,
    BassModerate,
    Pop,
    Rock,
    Electronic,
    Vocal,
    Treble,
}

impl EqPreset {
    pub const ALL: [EqPreset; 9] = [
        EqPreset::Custom,
        EqPreset::Flat,
        EqPreset::BassStrong,
        EqPreset::BassModerate,
        EqPreset::Pop,
        EqPreset::Rock,
        EqPreset::Electronic,
        EqPreset::Vocal,
        EqPreset::Treble,
    ];

    /// Mức của preset; `Custom` không có bảng số, giá trị nằm ở `EqSettings::custom_gains` (M7).
    pub fn gains(self) -> Option<[f32; EQ_BAND_COUNT]> {
        Some(match self {
            EqPreset::Custom => return None,
            EqPreset::Flat => [0.0; EQ_BAND_COUNT],
            EqPreset::BassStrong => [6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            EqPreset::BassModerate => [4.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            EqPreset::Pop => [-1.5, -1.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, -1.0, -1.5],
            EqPreset::Rock => [4.0, 3.0, -1.5, -2.5, -1.0, 1.0, 3.5, 4.5, 5.0, 5.0],
            EqPreset::Electronic => [5.0, 4.0, 1.0, 0.0, -1.0, 2.0, 3.0, 4.0, 5.0, 5.0],
            EqPreset::Vocal => [-3.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 2.0, 1.0, 0.0],
            EqPreset::Treble => [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 7.0, 9.0],
        })
    }

    fn is_bass(self) -> bool {
        matches!(self, EqPreset::BassStrong | EqPreset::BassModerate)
    }
}

/// EQ người dùng chỉnh. `gains` là giá trị thanh trượt, chưa tính bass boost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EqSettings {
    pub gains: [f32; EQ_BAND_COUNT],
    pub bass_boost: f32,
    pub preset: EqPreset,
    /// Giá trị tay gần nhất: chọn lại `Custom` thì về đây, không về toàn 0 như bản cũ (M7).
    pub custom_gains: [f32; EQ_BAND_COUNT],
}

impl EqSettings {
    pub fn select_preset(&mut self, preset: EqPreset) {
        self.preset = preset;
        self.gains = preset.gains().unwrap_or(self.custom_gains);
    }

    /// Kéo một dải: thành `Custom`, giữ các dải còn lại. `band` ngoài khoảng thì bỏ qua.
    pub fn set_band(&mut self, band: usize, gain_db: f32) {
        let Some(slot) = self.gains.get_mut(band) else { return };
        *slot = clamp_gain(gain_db);
        self.preset = EqPreset::Custom;
        self.custom_gains = self.gains;
    }

    /// Kéo bass boost: thành `Custom`, trừ khi preset đang là loại Bass.
    pub fn set_bass_boost(&mut self, boost_db: f32) {
        self.bass_boost = clamp_finite(boost_db, 0.0, BASS_BOOST_MAX_DB);
        if self.preset != EqPreset::Custom && !self.preset.is_bass() {
            self.preset = EqPreset::Custom;
            self.custom_gains = self.gains;
        }
    }

    /// Mức đưa vào DSP: `gains` cộng bass boost, kẹp ±10 dB.
    pub fn effective_gains(&self) -> [f32; EQ_BAND_COUNT] {
        let mut out = self.gains;
        for (gain, weight) in out.iter_mut().zip(BASS_BOOST_WEIGHTS) {
            *gain = clamp_gain(*gain + self.bass_boost * weight);
        }
        out
    }

    fn sanitize(&mut self) {
        self.gains = self.gains.map(clamp_gain);
        self.custom_gains = self.custom_gains.map(clamp_gain);
        self.bass_boost = clamp_finite(self.bass_boost, 0.0, BASS_BOOST_MAX_DB);
    }
}

fn clamp_gain(gain_db: f32) -> f32 {
    clamp_finite(gain_db, -EQ_GAIN_LIMIT_DB, EQ_GAIN_LIMIT_DB)
}

// JSON không chứa được NaN nhưng giá trị từ thanh trượt đi qua đây; NaN lọt vào biquad làm câm cả kênh.
fn clamp_finite(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() { value.clamp(min, max) } else { 0.0_f32.clamp(min, max) }
}

/// Một dòng phím tắt, cùng dạng `[{key, command}]` của bản cũ.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    pub command: String,
}

pub fn default_keybindings() -> Vec<KeyBinding> {
    DEFAULT_KEYBINDINGS
        .iter()
        .map(|&(key, command)| KeyBinding { key: key.to_owned(), command: command.to_owned() })
        .collect()
}

/// Vị trí cửa sổ (đặc tả 5.7): pixel logic toàn cục. `width`, `height` là cỡ lúc chưa phóng to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPlacement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
    #[serde(default)]
    /// `u64::from(gpui::DisplayId)`.
    pub display_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `None`: chưa chọn, theo OS.
    pub language: Option<Language>,
    pub theme: ThemeChoice,
    pub volume: u8,
    pub is_muted: bool,
    /// Mức âm lượng trước khi tắt tiếng (tắt tiếng đưa `volume` về 0); bỏ tắt tiếng trả về mức này. `None` khi không nhớ gì.
    pub volume_before_mute: Option<u8>,
    pub seek_step: u32,
    pub volume_step: u8,
    pub eq: EqSettings,
    pub last_active_route: StartPage,
    /// `None`: dùng `DEFAULT_KEYBINDINGS`, để phím mới của bản sau tới được người chưa từng sửa.
    pub custom_keybindings: Option<Vec<KeyBinding>>,
    pub side_nav_collapsed: bool,
    pub window: Option<WindowPlacement>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            language: None,
            theme: ThemeChoice::default(),
            volume: 75,
            is_muted: false,
            volume_before_mute: None,
            seek_step: 5,
            volume_step: 2,
            eq: EqSettings::default(),
            last_active_route: StartPage::default(),
            custom_keybindings: None,
            side_nav_collapsed: false,
            window: None,
        }
    }
}

impl Settings {
    /// Đọc `paths::settings_file`, không lỗi: có gì bất thường thì ghi log và về mặc định.
    pub fn load() -> Settings {
        match paths::settings_file() {
            Ok(path) => Settings::load_from(&path),
            Err(err) => {
                log::warn!("settings path: {err}");
                Settings::default()
            }
        }
    }

    pub fn load_from(path: &Path) -> Settings {
        let mut settings: Settings = load_json(path);
        settings.sanitize();
        settings
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&paths::settings_file()?)
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        write_json_atomic(path, self)
    }

    /// Đưa giá trị về khoảng hợp lệ của đặc tả 1.10.
    pub fn sanitize(&mut self) {
        self.volume = self.volume.min(100);
        self.volume_before_mute = self.volume_before_mute.map(|v| v.min(100));
        self.seek_step = self.seek_step.max(1);
        self.volume_step = self.volume_step.clamp(1, 100);
        self.eq.sanitize();
    }

    pub fn effective_language(&self) -> Language {
        self.language.unwrap_or_else(Language::detect)
    }

    /// Áp ngôn ngữ cho toàn tiến trình; gọi khi mở app và mỗi lần đổi `language`.
    pub fn apply_language(&self) {
        i18n::set_language(self.effective_language());
    }

    pub fn keybindings(&self) -> Vec<KeyBinding> {
        self.custom_keybindings.clone().unwrap_or_else(default_keybindings)
    }
}

/// Đọc JSON, không lỗi. Thiếu tệp là mặc định; tệp hỏng đổi tên thành `<tên>.bad` để lần lưu sau không xoá mất.
pub(crate) fn load_json<T: Serialize + DeserializeOwned + Default>(path: &Path) -> T {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            if err.kind() != io::ErrorKind::NotFound {
                log::warn!("read {}: {err}", path.display());
            }
            return T::default();
        }
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(fields)) => merge_lenient(fields),
        Ok(_) => {
            set_aside(path, "not a JSON object");
            T::default()
        }
        Err(err) => {
            set_aside(path, &err.to_string());
            T::default()
        }
    }
}

fn set_aside(path: &Path, reason: &str) {
    let mut bad = path.as_os_str().to_owned();
    bad.push(".bad");
    log::warn!("{} is corrupt ({reason}), kept as .bad", path.display());
    if let Err(err) = std::fs::rename(path, PathBuf::from(bad)) {
        log::warn!("set aside {}: {err}", path.display());
    }
}

/// Đắp từng khoá của tệp lên giá trị mặc định; khoá sai kiểu thì giữ mặc định của khoá đó, không bỏ cả tệp. Chỉ chạy lúc mở
/// app, trên vài chục khoá.
fn merge_lenient<T: Serialize + DeserializeOwned + Default>(fields: serde_json::Map<String, Value>) -> T {
    let Ok(Value::Object(mut merged)) = serde_json::to_value(T::default()) else { return T::default() };
    for (key, value) in fields {
        let previous = merged.insert(key.clone(), value);
        if let Err(err) = serde_json::from_value::<T>(Value::Object(merged.clone())) {
            log::warn!("ignored JSON key {key}: {err}");
            match previous {
                Some(previous) => merged.insert(key, previous),
                None => merged.remove(&key),
            };
        }
    }
    serde_json::from_value(Value::Object(merged)).unwrap_or_default()
}

/// Ghi tệp tạm cùng thư mục + đổi tên: mất điện giữa chừng không để lại tệp dở. Tạo thư mục cha nếu thiếu.
pub(crate) fn write_json_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> io::Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    write_text_atomic(path, &text)
}

fn write_text_atomic(path: &Path, text: &str) -> io::Result<()> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.{}.tmp", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    let tmp = PathBuf::from(tmp);

    let written = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

enum SaverMsg {
    Save(String),
    Flush(Sender<()>),
}

/// Ghi JSON ở luồng nền có debounce: nhiều lần `save` liên tiếp chỉ ghi bản mới nhất (S12). `flush` và `Drop` ghi nốt rồi mới
/// về, nên gọi khi thoát app.
pub struct JsonSaver {
    tx: Option<Sender<SaverMsg>>,
    worker: Option<JoinHandle<()>>,
}

impl JsonSaver {
    pub fn spawn(path: PathBuf, debounce: Duration) -> io::Result<JsonSaver> {
        let (tx, rx) = mpsc::channel();
        let worker = thread::Builder::new().name("groove-json-saver".into()).spawn(move || {
            let mut pending: Option<(String, Instant)> = None;
            loop {
                let received = match &pending {
                    Some((_, due)) => rx.recv_timeout(due.saturating_duration_since(Instant::now())),
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match received {
                    // Giữ hạn của bản đầu: `save` liên tục (kéo thanh trượt) vẫn ghi đều, không bị đẩy lùi mãi.
                    Ok(SaverMsg::Save(text)) => {
                        let due = pending.take().map_or_else(|| Instant::now() + debounce, |(_, due)| due);
                        pending = Some((text, due));
                    }
                    Ok(SaverMsg::Flush(done)) => {
                        write_pending(&path, &mut pending);
                        let _ = done.send(());
                    }
                    Err(RecvTimeoutError::Timeout) => write_pending(&path, &mut pending),
                    Err(RecvTimeoutError::Disconnected) => {
                        write_pending(&path, &mut pending);
                        return;
                    }
                }
            }
        })?;
        Ok(JsonSaver { tx: Some(tx), worker: Some(worker) })
    }

    /// Chụp giá trị ngay trên luồng gọi, không chạm đĩa.
    pub fn save<T: Serialize + ?Sized>(&self, value: &T) {
        match serde_json::to_string_pretty(value) {
            Ok(text) => {
                if let Some(tx) = &self.tx {
                    let _ = tx.send(SaverMsg::Save(text));
                }
            }
            Err(err) => log::warn!("serialize JSON: {err}"),
        }
    }

    /// Chờ ghi xong mọi thứ đã `save`. Chặn: không gọi từ UI thread ngoài lúc thoát.
    pub fn flush(&self) {
        let (done_tx, done_rx) = mpsc::channel();
        if let Some(tx) = &self.tx
            && tx.send(SaverMsg::Flush(done_tx)).is_ok()
        {
            let _ = done_rx.recv();
        }
    }
}

fn write_pending(path: &Path, pending: &mut Option<(String, Instant)>) {
    if let Some((text, _)) = pending.take()
        && let Err(err) = write_text_atomic(path, &text)
    {
        log::warn!("write {}: {err}", path.display());
    }
}

impl Drop for JsonSaver {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("nested").join("settings.json")
    }

    #[test]
    fn defaults_match_the_old_app() {
        let s = Settings::default();
        assert_eq!((s.volume, s.is_muted, s.seek_step, s.volume_step), (75, false, 5, 2));
        assert_eq!(s.eq.gains, [0.0; 10]);
        assert_eq!(s.eq.preset, EqPreset::Flat);
        assert_eq!(s.last_active_route, StartPage::MyMusic);
        assert!(!s.side_nav_collapsed);
    }

    #[test]
    fn volume_before_mute_survives_a_restart_and_old_files_lack_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        let s = Settings { volume: 0, is_muted: true, volume_before_mute: Some(64), ..Settings::default() };
        s.save_to(&path).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"volumeBeforeMute\""));
        assert_eq!(Settings::load_from(&path).volume_before_mute, Some(64));
        std::fs::write(&path, r#"{"volume": 30, "isMuted": true}"#).unwrap();
        let old = Settings::load_from(&path);
        assert_eq!((old.volume, old.is_muted, old.volume_before_mute), (30, true, None));
    }

    #[test]
    fn missing_file_gives_defaults_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        assert_eq!(Settings::load_from(&path), Settings::default());
        assert!(!path.exists());
    }

    #[test]
    fn round_trip_keeps_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        let mut s = Settings { language: Some(Language::Vi), volume: 40, is_muted: true, ..Settings::default() };
        s.theme = ThemeChoice::System;
        s.seek_step = 10;
        s.volume_step = 7;
        s.eq.set_band(3, 2.5);
        s.eq.set_bass_boost(4.0);
        s.last_active_route = StartPage::Playlists;
        s.custom_keybindings = Some(vec![KeyBinding { key: "ctrl+p".into(), command: "toggle_play".into() }]);
        s.side_nav_collapsed = true;
        s.window = Some(WindowPlacement {
            x: 10.0,
            y: -20.0,
            width: 800.0,
            height: 600.0,
            maximized: true,
            display_id: Some(3),
        });
        s.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), s);
    }

    #[test]
    fn json_keys_are_camel_case_and_enums_snake_case() {
        let mut s = Settings { language: Some(Language::En), ..Settings::default() };
        s.eq.select_preset(EqPreset::BassStrong);
        s.last_active_route = StartPage::NowPlaying;
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["isMuted"], false);
        assert_eq!(json["seekStep"], 5);
        assert_eq!(json["volumeStep"], 2);
        assert_eq!(json["language"], "en");
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["lastActiveRoute"], "now_playing");
        assert_eq!(json["eq"]["preset"], "bass_strong");
        assert!(json["eq"]["bassBoost"].is_number());
        assert!(json["eq"]["customGains"].is_array());
        assert!(json.get("sideNavCollapsed").is_some());
        assert!(json.get("customKeybindings").is_some());
    }

    #[test]
    fn missing_keys_take_their_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"volume": 30, "somethingFromTheFuture": 1}"#).unwrap();
        let s = Settings::load_from(&path);
        assert_eq!(s.volume, 30);
        assert_eq!(s.seek_step, 5);
    }

    #[test]
    fn a_wrongly_typed_key_only_loses_that_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"volume": "loud", "theme": "sepia", "seekStep": 9}"#).unwrap();
        let s = Settings::load_from(&path);
        assert_eq!(s.volume, 75);
        assert_eq!(s.theme, ThemeChoice::Dark);
        assert_eq!(s.seek_step, 9);
    }

    #[test]
    fn corrupt_file_gives_defaults_and_is_kept_aside() {
        for corrupt in ["{ not json", "[1, 2]", ""] {
            let dir = tempfile::tempdir().unwrap();
            let path = temp_file(&dir);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, corrupt).unwrap();
            assert_eq!(Settings::load_from(&path), Settings::default());
            assert!(!path.exists());
            assert_eq!(std::fs::read_to_string(path.with_extension("json.bad")).unwrap(), corrupt);
        }
    }

    #[test]
    fn out_of_range_values_are_pulled_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"volume": 200, "seekStep": 0, "volumeStep": 0,
                "eq": {"gains": [50, -50, 0, 0, 0, 0, 0, 0, 0, 0], "bassBoost": 99}}"#,
        )
        .unwrap();
        let s = Settings::load_from(&path);
        assert_eq!((s.volume, s.seek_step, s.volume_step), (100, 1, 1));
        assert_eq!((s.eq.gains[0], s.eq.gains[1]), (10.0, -10.0));
        assert_eq!(s.eq.bass_boost, 10.0);
    }

    #[test]
    fn saving_leaves_no_temp_files_and_replaces_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        Settings::default().save_to(&path).unwrap();
        Settings { volume: 1, ..Settings::default() }.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path).volume, 1);
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn preset_table_matches_the_old_app() {
        assert_eq!(EqPreset::ALL.len(), 9);
        assert_eq!(EqPreset::Custom.gains(), None);
        assert_eq!(EqPreset::Treble.gains().unwrap()[9], 9.0);
        assert_eq!(EqPreset::Rock.gains().unwrap()[3], -2.5);
        for preset in EqPreset::ALL {
            for gain in preset.gains().unwrap_or_default() {
                assert!(gain.abs() <= EQ_GAIN_LIMIT_DB);
            }
        }
    }

    #[test]
    fn dragging_a_band_becomes_custom_and_keeps_the_other_bands() {
        let mut eq = EqSettings::default();
        eq.select_preset(EqPreset::Pop);
        eq.set_band(0, 20.0);
        assert_eq!(eq.preset, EqPreset::Custom);
        assert_eq!(eq.gains[0], 10.0);
        assert_eq!(eq.gains[4], 4.0);
        eq.set_band(99, 1.0);
        assert_eq!(eq.gains.len(), EQ_BAND_COUNT);
    }

    #[test]
    fn choosing_custom_again_restores_the_hand_values() {
        // M7: bản cũ đặt toàn 0.
        let mut eq = EqSettings::default();
        eq.set_band(2, 3.5);
        eq.select_preset(EqPreset::Rock);
        assert_eq!(eq.gains[2], -1.5);
        eq.select_preset(EqPreset::Custom);
        assert_eq!(eq.gains[2], 3.5);
    }

    #[test]
    fn bass_boost_keeps_bass_presets_only() {
        let mut eq = EqSettings::default();
        eq.select_preset(EqPreset::BassModerate);
        eq.set_bass_boost(3.0);
        assert_eq!(eq.preset, EqPreset::BassModerate);
        eq.select_preset(EqPreset::Pop);
        eq.set_bass_boost(3.0);
        assert_eq!(eq.preset, EqPreset::Custom);
        assert_eq!(eq.gains, EqPreset::Pop.gains().unwrap());
    }

    #[test]
    fn effective_gains_add_weighted_boost_and_clamp() {
        let mut eq = EqSettings::default();
        eq.select_preset(EqPreset::BassStrong);
        eq.set_bass_boost(10.0);
        let out = eq.effective_gains();
        assert_eq!(out[0], 10.0); // 6 + 10 bị kẹp
        assert_eq!(out[1], 10.0); // 5 + 8 bị kẹp
        assert_eq!(out[2], 8.0); // 3 + 5
        assert_eq!(out[3], 3.0); // 1 + 2
        assert_eq!(out[4], 0.0);
        // Đường cong không tính phần này.
        assert_eq!(eq.gains[0], 6.0);
    }

    #[test]
    fn non_finite_input_cannot_reach_the_dsp() {
        let mut eq = EqSettings::default();
        eq.set_band(0, f32::NAN);
        eq.set_bass_boost(f32::INFINITY);
        assert_eq!(eq.gains[0], 0.0);
        assert_eq!(eq.bass_boost, 0.0);
    }

    #[test]
    fn keybindings_default_until_customized() {
        let mut s = Settings::default();
        let defaults = s.keybindings();
        assert_eq!(defaults.len(), 11);
        assert!(defaults.iter().any(|b| b.key == "g p" && b.command == "go_to_now_playing"));
        s.custom_keybindings = Some(Vec::new());
        assert!(s.keybindings().is_empty());
    }

    #[test]
    fn apply_language_uses_the_choice_or_the_os() {
        let s = Settings { language: Some(Language::Vi), ..Settings::default() };
        assert_eq!(s.effective_language(), Language::Vi);
        s.apply_language();
        assert_eq!(i18n::language(), Language::Vi);
        assert_eq!(Settings::default().effective_language(), Language::detect());
    }

    #[test]
    fn saver_coalesces_writes_and_keeps_the_latest() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        let saver = JsonSaver::spawn(path.clone(), Duration::from_secs(3600)).unwrap();
        for volume in 1..=5 {
            saver.save(&Settings { volume, ..Settings::default() });
        }
        assert!(!path.exists(), "debounce chưa hết thì chưa được ghi");
        saver.flush();
        assert_eq!(Settings::load_from(&path).volume, 5);
    }

    #[test]
    fn saver_writes_after_the_debounce() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        let saver = JsonSaver::spawn(path.clone(), Duration::from_millis(10)).unwrap();
        saver.save(&Settings { volume: 9, ..Settings::default() });
        let deadline = Instant::now() + Duration::from_secs(10);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(Settings::load_from(&path).volume, 9);
    }

    #[test]
    fn dropping_the_saver_writes_what_is_pending() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_file(&dir);
        let saver = JsonSaver::spawn(path.clone(), Duration::from_secs(3600)).unwrap();
        saver.save(&Settings { volume: 11, ..Settings::default() });
        drop(saver);
        assert_eq!(Settings::load_from(&path).volume, 11);
    }
}
