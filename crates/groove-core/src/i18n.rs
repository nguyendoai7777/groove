//! Catalog chữ hiển thị `vi` / `en` (`i18n/<ngôn ngữ>/<vùng>.toml` ở gốc repo, nhúng vào exe lúc build).
//!
//! Mỗi vùng một tệp, mỗi task UI sở hữu một vùng nên làm song song không đụng nhau. Khoá trong tệp viết không tiền tố,
//! tra bằng `t!("<vùng>.<khoá>")`; bảng lồng thành phần trước dấu chấm, biến `{name}`. Thiếu khoá thì lấy tiếng Anh,
//! thiếu nữa thì trả chính khoá.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

/// Ngôn ngữ giao diện. Giá trị trong `settings.json`: `vi`, `en`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Vi,
    En,
}

impl Language {
    /// Theo locale của OS khi người dùng chưa chọn: `vi*` là tiếng Việt, còn lại tiếng Anh.
    pub fn detect() -> Language {
        sys_locale::get_locale().map_or(Language::En, |locale| Language::from_locale(&locale))
    }

    fn from_locale(locale: &str) -> Language {
        if locale.get(..2).is_some_and(|prefix| prefix.eq_ignore_ascii_case("vi")) {
            Language::Vi
        } else {
            Language::En
        }
    }
}

/// Mọi vùng, một chỗ duy nhất. Task sau chỉ sửa tệp `.toml` của vùng mình.
macro_rules! catalog {
    ($lang:literal) => {
        catalog!(@areas $lang, "common", "shell", "player", "library", "song_list", "actions", "metadata",
            "song_dialogs", "now_playing", "palette", "settings", "platform")
    };
    (@areas $lang:literal, $($area:literal),+) => {
        &[$(($area, include_str!(concat!("../../../i18n/", $lang, "/", $area, ".toml")))),+]
    };
}

const VI: &[(&str, &str)] = catalog!("vi");
const EN: &[(&str, &str)] = catalog!("en");

// Chỉ phân tích catalog của ngôn ngữ thật sự dùng; chữ sống suốt tiến trình nên `t` trả `&'static str`, không cấp phát.
static VI_TEXT: LazyLock<HashMap<String, String>> = LazyLock::new(|| parse(VI));
static EN_TEXT: LazyLock<HashMap<String, String>> = LazyLock::new(|| parse(EN));

const LANG_UNSET: u8 = 0;
const LANG_VI: u8 = 1;
const LANG_EN: u8 = 2;
static CURRENT: AtomicU8 = AtomicU8::new(LANG_UNSET);

/// Đổi ngôn ngữ cho toàn tiến trình; view tự vẽ lại ở lần `cx.notify()` kế tiếp.
pub fn set_language(lang: Language) {
    CURRENT.store(if lang == Language::Vi { LANG_VI } else { LANG_EN }, Ordering::Relaxed);
}

/// Ngôn ngữ đang dùng; chưa ai đặt thì theo OS.
pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        LANG_VI => Language::Vi,
        LANG_EN => Language::En,
        _ => {
            let detected = Language::detect();
            set_language(detected);
            detected
        }
    }
}

/// Chữ của khoá theo ngôn ngữ hiện tại. Thường gọi qua `t!`.
pub fn t(key: &'static str) -> &'static str {
    lookup(language(), key)
}

fn lookup(lang: Language, key: &'static str) -> &'static str {
    let primary = (lang == Language::Vi).then(|| VI_TEXT.get(key)).flatten();
    primary.or_else(|| EN_TEXT.get(key)).map_or(key, String::as_str)
}

/// `t` rồi thay `{name}` bằng giá trị. Thường gọi qua `t!("khoá", name = giá_trị)`.
pub fn t_args(key: &'static str, args: &[(&str, &str)]) -> String {
    substitute(t(key), args)
}

fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// `t!("common.save")` trả `&'static str`; `t!("library.imported", count = n)` trả `String`.
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::t($key)
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::t_args($key, &[$((stringify!($name), ($value).to_string().as_str())),+])
    };
}

fn parse(areas: &[(&str, &str)]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (area, src) in areas {
        let table: toml::Table = src.parse().unwrap_or_else(|e| panic!("i18n area `{area}` is not valid TOML: {e}"));
        flatten(area, &table, &mut out);
    }
    out
}

fn flatten(prefix: &str, table: &toml::Table, out: &mut HashMap<String, String>) {
    for (k, v) in table {
        let key = format!("{prefix}.{k}");
        match v {
            toml::Value::String(s) => {
                out.insert(key, s.clone());
            }
            toml::Value::Table(t) => flatten(&key, t, out),
            other => panic!("i18n key `{key}` must be a string, got {}", other.type_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn placeholders(s: &str) -> BTreeSet<&str> {
        s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(name, _)| name)).collect()
    }

    #[test]
    fn both_languages_have_the_same_keys_and_placeholders() {
        let keys = |m: &HashMap<String, String>| m.keys().cloned().collect::<BTreeSet<_>>();
        assert_eq!(keys(&VI_TEXT), keys(&EN_TEXT));
        for (key, vi) in VI_TEXT.iter() {
            assert_eq!(placeholders(vi), placeholders(&EN_TEXT[key]), "placeholders of `{key}`");
        }
    }

    #[test]
    fn nothing_is_written_all_caps() {
        // D-13: viết hoa chữ cái đầu dòng. Từ viết tắt ngắn (OK, EQ) thì được.
        for (key, text) in VI_TEXT.iter().chain(EN_TEXT.iter()) {
            let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
            let all_caps = letters.len() > 4 && letters.iter().all(|c| c.is_uppercase());
            assert!(!all_caps, "`{key}` is all caps: {text}");
        }
    }

    #[test]
    fn lookup_falls_back_to_the_key() {
        assert_eq!(lookup(Language::Vi, "common.cancel"), "Huỷ");
        assert_eq!(lookup(Language::En, "common.cancel"), "Cancel");
        assert_eq!(lookup(Language::Vi, "no.such.key"), "no.such.key");
    }

    #[test]
    fn nested_tables_become_dotted_keys() {
        let mut out = HashMap::new();
        flatten("area", &"a = \"1\"\n[b]\nc = \"2\"".parse().unwrap(), &mut out);
        assert_eq!(out["area.a"], "1");
        assert_eq!(out["area.b.c"], "2");
    }

    #[test]
    fn substitute_replaces_named_values() {
        assert_eq!(substitute("{n} / {m}", &[("n", "1"), ("m", "2")]), "1 / 2");
    }

    #[test]
    fn macro_formats_any_display_value() {
        // Khoá không tồn tại thì trả chính khoá, không có biến để thay: đủ để soát macro biên dịch và chạy.
        assert_eq!(crate::t!("no.such.key"), "no.such.key");
        assert_eq!(crate::t!("no.such.key", count = 3), "no.such.key");
    }

    #[test]
    fn locale_prefix_picks_language() {
        assert_eq!(Language::from_locale("vi-VN"), Language::Vi);
        assert_eq!(Language::from_locale("VI"), Language::Vi);
        assert_eq!(Language::from_locale("en-US"), Language::En);
        assert_eq!(Language::from_locale(""), Language::En);
    }

    #[test]
    fn language_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&Language::Vi).unwrap(), "\"vi\"");
        assert_eq!(serde_json::from_str::<Language>("\"en\"").unwrap(), Language::En);
    }
}
