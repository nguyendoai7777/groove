//! groove-gd1_22: phím tắt, khớp chuỗi phím.
//!
//! Không UI: app đổi phím GPUI thành [`Chord`], nói phím rơi vào [`KeyScope`] nào, phần còn lại ở đây. Bảng mặc định nằm ở
//! `settings::DEFAULT_KEYBINDINGS`, không chép lại.
//!
//! Luật khớp (đặc tả 1.10): tiền tố `ctrl`, `alt`, `shift`, `meta` tính là `ctrl`; chuỗi tối đa 2 phím, quên sau 800 ms; khớp
//! chuỗi trước, khớp phím đơn sau. Tên phím của bản cũ (`arrowleft`) và của GPUI (`left`) là một.
//!
//! M5: tệp cài đặt có thể chứa bất cứ gì, nên [`Keymap::from_bindings`] bỏ dòng sai và vẫn chạy; [`validate`] liệt kê từng dòng
//! sai để hộp Settings báo.

use std::fmt;
use std::time::{Duration, Instant};

use crate::settings::KeyBinding;

/// Hết khoảng này kể từ phím trước thì quên phím đầu của chuỗi.
pub const SEQUENCE_TIMEOUT: Duration = Duration::from_millis(800);
/// Số phím tối đa của một chuỗi.
pub const MAX_SEQUENCE: usize = 2;

/// Lệnh gán phím được. Một dispatcher chung với media session và taskbar (đặc tả 3, `commands::run`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    OpenSearch,
    TogglePlay,
    SeekBackward,
    SeekForward,
    PrevTrack,
    NextTrack,
    VolumeUp,
    VolumeDown,
    PlayRandom,
    GoToLibrary,
    GoToNowPlaying,
}

impl Command {
    /// Mọi lệnh hợp lệ, cũng là danh sách gợi ý trong ô JSON phím tắt.
    pub const ALL: [Command; 11] = [
        Command::OpenSearch,
        Command::TogglePlay,
        Command::SeekBackward,
        Command::SeekForward,
        Command::PrevTrack,
        Command::NextTrack,
        Command::VolumeUp,
        Command::VolumeDown,
        Command::PlayRandom,
        Command::GoToLibrary,
        Command::GoToNowPlaying,
    ];

    /// Tên trong `customKeybindings`.
    pub const fn name(self) -> &'static str {
        match self {
            Command::OpenSearch => "open_search",
            Command::TogglePlay => "toggle_play",
            Command::SeekBackward => "seek_backward",
            Command::SeekForward => "seek_forward",
            Command::PrevTrack => "prev_track",
            Command::NextTrack => "next_track",
            Command::VolumeUp => "volume_up",
            Command::VolumeDown => "volume_down",
            Command::PlayRandom => "play_random",
            Command::GoToLibrary => "go_to_library",
            Command::GoToNowPlaying => "go_to_now_playing",
        }
    }

    pub fn from_name(name: &str) -> Option<Command> {
        Command::ALL.into_iter().find(|command| command.name() == name)
    }

    /// Chạy cả khi đang gõ trong ô nhập (M4): chỉ mở bảng lệnh.
    pub const fn works_in_text_input(self) -> bool {
        matches!(self, Command::OpenSearch)
    }
}

/// Vì sao một chuỗi phím không đọc được.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyParseError {
    /// Chuỗi rỗng.
    Empty,
    /// Hơn [`MAX_SEQUENCE`] phím.
    TooLong,
    /// Chỉ có phím bổ trợ, hoặc thiếu tên phím (`ctrl+`).
    MissingKey,
    /// Tiền tố không phải `ctrl`, `alt`, `shift`, `meta`.
    UnknownModifier(String),
}

/// Một phím kèm phím bổ trợ, đã chuẩn hoá: chữ thường, `left` thành `arrowleft`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    ctrl: bool,
    alt: bool,
    shift: bool,
    key: String,
}

impl Chord {
    /// Từ sự kiện phím. `None` khi chỉ là phím bổ trợ: nhấn Shift không được làm quên phím đầu của chuỗi.
    pub fn from_event(key: &str, ctrl: bool, alt: bool, shift: bool) -> Option<Chord> {
        let key = canonical_key(key);
        (!is_modifier_key(&key)).then_some(Chord { ctrl, alt, shift, key })
    }

    /// `ctrl+alt+k`, thứ tự tiền tố nào cũng được.
    pub fn parse(text: &str) -> Result<Chord, KeyParseError> {
        let text = text.trim().to_lowercase();
        // `+` cũng là một phím: `+`, `ctrl++`.
        let (modifiers, key) = if text == "+" {
            ("", "+")
        } else if let Some(modifiers) = text.strip_suffix("++") {
            (modifiers, "+")
        } else {
            text.rsplit_once('+').unwrap_or(("", &text))
        };
        let (mut ctrl, mut alt, mut shift) = (false, false, false);
        for modifier in modifiers.split('+').filter(|_| !modifiers.is_empty()) {
            match modifier {
                "ctrl" | "meta" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                other => return Err(KeyParseError::UnknownModifier(other.to_owned())),
            }
        }
        if key.is_empty() {
            return Err(KeyParseError::MissingKey);
        }
        Chord::from_event(key, ctrl, alt, shift).ok_or(KeyParseError::MissingKey)
    }

    /// Nhãn cho `Kbd`: `Ctrl+K`, `Alt+←`, `Space`.
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(4);
        if self.ctrl {
            parts.push("Ctrl".to_owned());
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        parts.push(key_label(&self.key));
        parts.join("+")
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [(self.ctrl, "ctrl+"), (self.alt, "alt+"), (self.shift, "shift+")] {
            if on {
                f.write_str(name)?;
            }
        }
        f.write_str(&self.key)
    }
}

fn canonical_key(key: &str) -> String {
    let key = key.to_lowercase();
    match key.as_str() {
        "left" => "arrowleft".to_owned(),
        "right" => "arrowright".to_owned(),
        "up" => "arrowup".to_owned(),
        "down" => "arrowdown".to_owned(),
        " " | "spacebar" => "space".to_owned(),
        "esc" => "escape".to_owned(),
        _ => key,
    }
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "" | "ctrl" | "control" | "alt" | "shift" | "meta" | "cmd" | "platform" | "super" | "win" | "fn" | "function"
    )
}

fn key_label(key: &str) -> String {
    let arrow = match key {
        "arrowleft" => Some("←"),
        "arrowright" => Some("→"),
        "arrowup" => Some("↑"),
        "arrowdown" => Some("↓"),
        _ => None,
    };
    if let Some(arrow) = arrow {
        return arrow.to_owned();
    }
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Một đến [`MAX_SEQUENCE`] phím, cách nhau bằng khoảng trắng: `g m`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeySequence(Vec<Chord>);

impl KeySequence {
    pub fn parse(text: &str) -> Result<KeySequence, KeyParseError> {
        let chords = text.split_whitespace().map(Chord::parse).collect::<Result<Vec<_>, _>>()?;
        match chords.len() {
            0 => Err(KeyParseError::Empty),
            1..=MAX_SEQUENCE => Ok(KeySequence(chords)),
            _ => Err(KeyParseError::TooLong),
        }
    }

    pub fn chords(&self) -> &[Chord] {
        &self.0
    }

    /// Nhãn cho `Kbd`: `G M`, `Ctrl+K`.
    pub fn label(&self) -> String {
        self.0.iter().map(Chord::label).collect::<Vec<_>>().join(" ")
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (ix, chord) in self.0.iter().enumerate() {
            if ix > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{chord}")?;
        }
        Ok(())
    }
}

/// Bảng phím đã kiểm: dòng sai bị bỏ, phím trùng thì dòng đầu thắng như bản cũ.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap {
    entries: Vec<(KeySequence, Command)>,
}

impl Keymap {
    pub fn from_bindings(bindings: &[KeyBinding]) -> Keymap {
        let mut entries: Vec<(KeySequence, Command)> = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let (Ok(sequence), Some(command)) =
                (KeySequence::parse(&binding.key), Command::from_name(&binding.command))
            else {
                continue;
            };
            if !entries.iter().any(|(existing, _)| *existing == sequence) {
                entries.push((sequence, command));
            }
        }
        Keymap { entries }
    }

    fn lookup(&self, chords: &[Chord]) -> Option<Command> {
        self.entries.iter().find(|(sequence, _)| sequence.chords() == chords).map(|&(_, command)| command)
    }

    /// Phím đầu tiên gán cho lệnh, để hiện gợi ý.
    pub fn sequence_for(&self, command: Command) -> Option<&KeySequence> {
        self.entries.iter().find(|&&(_, c)| c == command).map(|(sequence, _)| sequence)
    }
}

/// Phím vừa nhấn rơi vào đâu (M4). App tính từ ngữ cảnh GPUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyScope {
    /// Không ai giữ phím này: mọi phím tắt chạy.
    Global,
    /// Đang gõ trong ô nhập: chỉ lệnh [`Command::works_in_text_input`], và phím đơn.
    TextInput,
    /// Dialog đang mở, hoặc thành phần đang focus giữ phím này: không phím tắt nào chạy.
    Blocked,
}

/// Nhớ phím đầu của chuỗi.
#[derive(Clone, Debug, Default)]
pub struct KeyMatcher {
    history: Vec<Chord>,
    last_press: Option<Instant>,
}

impl KeyMatcher {
    /// Cho một phím vào, trả lệnh nếu khớp. `now` do bên gọi đưa để test được thời gian.
    pub fn press(&mut self, keymap: &Keymap, chord: Chord, scope: KeyScope, now: Instant) -> Option<Command> {
        match scope {
            KeyScope::Blocked => {
                self.reset();
                None
            }
            KeyScope::TextInput => {
                self.reset();
                keymap.lookup(std::slice::from_ref(&chord)).filter(|command| command.works_in_text_input())
            }
            KeyScope::Global => {
                if self.last_press.is_some_and(|last| now.duration_since(last) >= SEQUENCE_TIMEOUT) {
                    self.history.clear();
                }
                if self.history.len() == MAX_SEQUENCE {
                    self.history.remove(0);
                }
                self.history.push(chord);
                self.last_press = Some(now);
                let single = &self.history[self.history.len() - 1..];
                let matched = keymap.lookup(&self.history).or_else(|| keymap.lookup(single));
                if matched.is_some() {
                    self.reset();
                }
                matched
            }
        }
    }

    pub fn reset(&mut self) {
        self.history.clear();
        self.last_press = None;
    }
}

/// Một dòng sai trong danh sách phím (M5). `index` tính từ 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyIssue {
    UnknownCommand {
        index: usize,
        command: String,
    },
    InvalidKey {
        index: usize,
        key: String,
        error: KeyParseError,
    },
    /// Cùng phím với dòng `first`, dù viết khác (`left`, `arrowleft`). Dòng đầu thắng.
    DuplicateKey {
        index: usize,
        first: usize,
        key: String,
    },
    /// Chuỗi không bao giờ tới được vì phím đầu của nó đã là phím đơn ở dòng `prefix`.
    UnreachableSequence {
        index: usize,
        prefix: usize,
        key: String,
    },
}

impl KeyIssue {
    pub fn index(&self) -> usize {
        match *self {
            KeyIssue::UnknownCommand { index, .. }
            | KeyIssue::InvalidKey { index, .. }
            | KeyIssue::DuplicateKey { index, .. }
            | KeyIssue::UnreachableSequence { index, .. } => index,
        }
    }
}

/// Soát danh sách phím, trả mọi dòng sai theo thứ tự dòng.
pub fn validate(bindings: &[KeyBinding]) -> Vec<KeyIssue> {
    let mut issues = Vec::new();
    let mut seen: Vec<(usize, KeySequence)> = Vec::with_capacity(bindings.len());
    for (index, binding) in bindings.iter().enumerate() {
        if Command::from_name(&binding.command).is_none() {
            issues.push(KeyIssue::UnknownCommand { index, command: binding.command.clone() });
        }
        match KeySequence::parse(&binding.key) {
            Err(error) => issues.push(KeyIssue::InvalidKey { index, key: binding.key.clone(), error }),
            Ok(sequence) => {
                if let Some(&(first, _)) = seen.iter().find(|(_, other)| *other == sequence) {
                    issues.push(KeyIssue::DuplicateKey { index, first, key: sequence.to_string() });
                } else {
                    seen.push((index, sequence));
                }
            }
        }
    }
    for (index, sequence) in &seen {
        let chords = sequence.chords();
        if chords.len() < 2 {
            continue;
        }
        if let Some((prefix, _)) = seen.iter().find(|(_, other)| other.chords() == &chords[..1]) {
            issues.push(KeyIssue::UnreachableSequence { index: *index, prefix: *prefix, key: sequence.to_string() });
        }
    }
    issues.sort_by_key(KeyIssue::index);
    issues
}

/// Lỗi hình dạng của ô JSON phím tắt (đặc tả 1.10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyJsonError {
    Empty,
    /// Không phải JSON. `line` tính từ 1.
    Syntax {
        line: usize,
        column: usize,
    },
    NotArray,
    /// Dòng `index` (từ 0) thiếu `key` hoặc `command`, hay chúng rỗng, không phải chuỗi.
    MissingField {
        index: usize,
    },
}

/// Đọc ô JSON `[{key, command}]`. Chỉ soát hình dạng; [`validate`] soát nội dung.
pub fn parse_json(text: &str) -> Result<Vec<KeyBinding>, KeyJsonError> {
    if text.trim().is_empty() {
        return Err(KeyJsonError::Empty);
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| KeyJsonError::Syntax { line: err.line(), column: err.column() })?;
    let items = value.as_array().ok_or(KeyJsonError::NotArray)?;
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let field = |name: &str| item.get(name).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
            match (field("key"), field("command")) {
                (Some(key), Some(command)) => Ok(KeyBinding { key: key.to_owned(), command: command.to_owned() }),
                _ => Err(KeyJsonError::MissingField { index }),
            }
        })
        .collect()
}

/// Văn bản cho ô JSON: nút "Đặt lại mặc định", mở hộp Settings.
pub fn to_json(bindings: &[KeyBinding]) -> String {
    serde_json::to_string_pretty(bindings).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{DEFAULT_KEYBINDINGS, default_keybindings};

    fn binding(key: &str, command: &str) -> KeyBinding {
        KeyBinding { key: key.to_owned(), command: command.to_owned() }
    }

    fn chord(text: &str) -> Chord {
        Chord::parse(text).unwrap()
    }

    fn keymap() -> Keymap {
        Keymap::from_bindings(&default_keybindings())
    }

    struct Clock(Instant);

    impl Clock {
        fn after(&mut self, ms: u64) -> Instant {
            self.0 += Duration::from_millis(ms);
            self.0
        }
    }

    fn clock() -> Clock {
        Clock(Instant::now())
    }

    #[test]
    fn every_default_binding_is_valid_and_reachable() {
        assert!(validate(&default_keybindings()).is_empty());
        let map = keymap();
        assert_eq!(map.entries.len(), DEFAULT_KEYBINDINGS.len());
        for command in Command::ALL {
            assert!(map.sequence_for(command).is_some(), "{} không có phím mặc định", command.name());
            assert_eq!(Command::from_name(command.name()), Some(command));
        }
    }

    #[test]
    fn chord_normalizes_order_case_meta_and_key_names() {
        assert_eq!(chord("Shift+CTRL+k"), chord("ctrl+shift+k"));
        assert_eq!(chord("meta+k"), chord("ctrl+k"));
        assert_eq!(chord("left"), chord("arrowleft"));
        assert_eq!(chord("alt+right").to_string(), "alt+arrowright");
        assert_eq!(chord("ctrl++").to_string(), "ctrl++");
        assert_eq!(chord("+").to_string(), "+");
        assert_eq!(Chord::from_event("Up", false, false, false), Some(chord("arrowup")));
        assert_eq!(Chord::from_event(" ", false, false, false), Some(chord("space")));
    }

    #[test]
    fn modifier_alone_is_not_a_chord() {
        for key in ["shift", "control", "alt", "platform", "function", ""] {
            assert_eq!(Chord::from_event(key, false, false, false), None, "{key}");
        }
    }

    #[test]
    fn bad_sequences_are_rejected_with_a_reason() {
        assert_eq!(KeySequence::parse("  "), Err(KeyParseError::Empty));
        assert_eq!(KeySequence::parse("a b c"), Err(KeyParseError::TooLong));
        assert_eq!(KeySequence::parse("ctrl+"), Err(KeyParseError::MissingKey));
        assert_eq!(KeySequence::parse("ctrl"), Err(KeyParseError::MissingKey));
        assert_eq!(KeySequence::parse("hyper+k"), Err(KeyParseError::UnknownModifier("hyper".to_owned())));
    }

    #[test]
    fn labels_are_for_people() {
        let label = |text: &str| KeySequence::parse(text).unwrap().label();
        assert_eq!(label("ctrl+k"), "Ctrl+K");
        assert_eq!(label("g m"), "G M");
        assert_eq!(label("alt+arrowleft"), "Alt+←");
        assert_eq!(label("space"), "Space");
    }

    /// Luật khớp đặc tả 1.10: chuỗi trong 800 ms.
    #[test]
    fn sequence_matches_within_timeout() {
        let (map, mut time, mut matcher) = (keymap(), clock(), KeyMatcher::default());
        assert_eq!(matcher.press(&map, chord("g"), KeyScope::Global, time.after(0)), None);
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(799)), Some(Command::GoToLibrary));
        // Đã khớp thì quên: `m` tiếp theo không khớp lại.
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(1)), None);
    }

    #[test]
    fn sequence_is_forgotten_after_timeout() {
        let (map, mut time, mut matcher) = (keymap(), clock(), KeyMatcher::default());
        matcher.press(&map, chord("g"), KeyScope::Global, time.after(0));
        assert_eq!(matcher.press(&map, chord("p"), KeyScope::Global, time.after(800)), None);
    }

    /// Lịch sử trượt: phím lạc trước đó không làm hỏng chuỗi; phím đơn vẫn chạy khi đang dở chuỗi.
    #[test]
    fn history_slides_and_single_key_still_works_mid_sequence() {
        let (map, mut time, mut matcher) = (keymap(), clock(), KeyMatcher::default());
        matcher.press(&map, chord("x"), KeyScope::Global, time.after(0));
        matcher.press(&map, chord("g"), KeyScope::Global, time.after(10));
        assert_eq!(matcher.press(&map, chord("p"), KeyScope::Global, time.after(10)), Some(Command::GoToNowPlaying));

        matcher.press(&map, chord("g"), KeyScope::Global, time.after(10));
        assert_eq!(matcher.press(&map, chord("space"), KeyScope::Global, time.after(10)), Some(Command::TogglePlay));
        // Phím đơn khớp thì quên cả `g` trước nó.
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(10)), None);
    }

    /// Khớp chuỗi trước, phím đơn sau.
    #[test]
    fn sequence_wins_over_single() {
        let map = Keymap::from_bindings(&[binding("m", "toggle_play"), binding("g m", "go_to_library")]);
        let (mut time, mut matcher) = (clock(), KeyMatcher::default());
        matcher.press(&map, chord("g"), KeyScope::Global, time.after(0));
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(10)), Some(Command::GoToLibrary));
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(10)), Some(Command::TogglePlay));
    }

    /// M4: đang gõ trong ô nhập chỉ `Ctrl+K` chạy, chữ gõ không dở chuỗi.
    #[test]
    fn text_input_only_runs_open_search() {
        let (map, mut time, mut matcher) = (keymap(), clock(), KeyMatcher::default());
        for key in ["space", "arrowleft", "alt+p"] {
            assert_eq!(matcher.press(&map, chord(key), KeyScope::TextInput, time.after(1)), None, "{key}");
        }
        assert_eq!(matcher.press(&map, chord("ctrl+k"), KeyScope::TextInput, time.after(1)), Some(Command::OpenSearch));
        matcher.press(&map, chord("g"), KeyScope::TextInput, time.after(1));
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(1)), None);
    }

    /// M4: dialog mở thì không phím nào chạy, kể cả `Ctrl+K`; phím bị chặn làm quên chuỗi đang dở.
    #[test]
    fn blocked_scope_runs_nothing_and_breaks_the_sequence() {
        let (map, mut time, mut matcher) = (keymap(), clock(), KeyMatcher::default());
        assert_eq!(matcher.press(&map, chord("ctrl+k"), KeyScope::Blocked, time.after(1)), None);
        matcher.press(&map, chord("g"), KeyScope::Global, time.after(1));
        matcher.press(&map, chord("arrowup"), KeyScope::Blocked, time.after(1));
        assert_eq!(matcher.press(&map, chord("m"), KeyScope::Global, time.after(1)), None);
    }

    /// M5: lệnh lạ bị báo, không qua kiểm tra như bản cũ.
    #[test]
    fn unknown_command_is_reported() {
        let issues = validate(&[binding("ctrl+k", "open_search"), binding("ctrl+j", "launch_rocket")]);
        assert_eq!(issues, [KeyIssue::UnknownCommand { index: 1, command: "launch_rocket".to_owned() }]);
    }

    /// M5: phím trùng bị báo, kể cả viết khác nhau.
    #[test]
    fn duplicate_key_is_reported_across_spellings() {
        let issues = validate(&[binding("left", "seek_backward"), binding("arrowleft", "prev_track")]);
        assert_eq!(issues, [KeyIssue::DuplicateKey { index: 1, first: 0, key: "arrowleft".to_owned() }]);
    }

    #[test]
    fn invalid_key_and_unreachable_sequence_are_reported() {
        let issues =
            validate(&[binding("a b c", "toggle_play"), binding("g m", "go_to_library"), binding("g", "play_random")]);
        assert_eq!(
            issues,
            [
                KeyIssue::InvalidKey { index: 0, key: "a b c".to_owned(), error: KeyParseError::TooLong },
                KeyIssue::UnreachableSequence { index: 1, prefix: 2, key: "g m".to_owned() },
            ]
        );
    }

    /// Tệp cài đặt sửa tay: dòng sai bị bỏ, dòng đúng vẫn chạy, phím trùng dòng đầu thắng.
    #[test]
    fn keymap_survives_bad_lines() {
        let map = Keymap::from_bindings(&[
            binding("ctrl+j", "launch_rocket"),
            binding("a b c", "toggle_play"),
            binding("space", "toggle_play"),
            binding("space", "play_random"),
        ]);
        assert_eq!(map.entries.len(), 1);
        let mut matcher = KeyMatcher::default();
        assert_eq!(matcher.press(&map, chord("space"), KeyScope::Global, Instant::now()), Some(Command::TogglePlay));
        assert_eq!(map.sequence_for(Command::PlayRandom), None);
    }

    #[test]
    fn json_shape_errors() {
        assert_eq!(parse_json("  "), Err(KeyJsonError::Empty));
        assert!(matches!(parse_json("[{"), Err(KeyJsonError::Syntax { .. })));
        assert_eq!(parse_json("{}"), Err(KeyJsonError::NotArray));
        assert_eq!(
            parse_json(r#"[{"key":"a","command":"toggle_play"},{"key":"b"}]"#),
            Err(KeyJsonError::MissingField { index: 1 })
        );
        assert_eq!(parse_json(r#"[{"key":"","command":"x"}]"#), Err(KeyJsonError::MissingField { index: 0 }));
        assert_eq!(parse_json(r#"[{"key":1,"command":"x"}]"#), Err(KeyJsonError::MissingField { index: 0 }));
    }

    #[test]
    fn json_round_trips_defaults() {
        let defaults = default_keybindings();
        assert_eq!(parse_json(&to_json(&defaults)).unwrap(), defaults);
    }
}
