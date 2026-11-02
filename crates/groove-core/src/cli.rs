//! groove-gd1_22: lệnh dạng /play của bảng lệnh.
//!
//! Chỉ đọc chữ, không chạy gì: `commands::run_cli` ở app chạy. S21: Enter chạy lệnh đang chọn, không chạy chữ đang gõ, nên `/n`
//! + Enter là `/next`; xem [`resolve`].

use crate::settings::ThemeChoice;

/// Đối số của một lệnh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliArg {
    None,
    Optional,
    Required,
}

/// Một dòng trong danh sách lệnh. Mô tả và gợi ý đối số do bảng lệnh tra catalog theo `name`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliSpec {
    pub name: &'static str,
    pub arg: CliArg,
}

impl CliSpec {
    /// Chữ điền vào ô khi bấm Tab. Lệnh có đối số thì kèm khoảng trắng để gõ tiếp.
    pub fn completion(&self) -> String {
        match self.arg {
            CliArg::None => self.name.to_owned(),
            CliArg::Optional | CliArg::Required => format!("{} ", self.name),
        }
    }
}

/// Mọi lệnh, theo thứ tự hiện (đặc tả 1.9).
pub const COMMANDS: [CliSpec; 9] = [
    CliSpec { name: "/play", arg: CliArg::Optional },
    CliSpec { name: "/playall", arg: CliArg::None },
    CliSpec { name: "/queue", arg: CliArg::Required },
    CliSpec { name: "/pause", arg: CliArg::None },
    CliSpec { name: "/next", arg: CliArg::None },
    CliSpec { name: "/prev", arg: CliArg::None },
    CliSpec { name: "/volume", arg: CliArg::Required },
    CliSpec { name: "/theme", arg: CliArg::Optional },
    CliSpec { name: "/help", arg: CliArg::None },
];

/// Lệnh đã đọc xong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliCommand {
    /// Không tên thì tiếp tục phát.
    Play(Option<String>),
    PlayAll,
    Queue(String),
    Pause,
    Next,
    Prev,
    Volume(u8),
    /// Không đối số thì đổi qua lại sáng, tối.
    Theme(Option<ThemeChoice>),
    Help,
}

/// Vì sao chữ đang gõ không chạy được. Bảng lệnh hiện lời báo và không đóng.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliError {
    /// Không bắt đầu bằng `/`.
    NotACommand,
    Unknown(String),
    MissingArg(&'static str),
    InvalidVolume(String),
    InvalidTheme(String),
}

/// Chữ trong ô có phải chế độ lệnh không.
pub fn is_cli(input: &str) -> bool {
    input.trim_start().starts_with('/')
}

/// Tách tên lệnh (chữ thường) và đối số.
fn split(input: &str) -> Option<(String, &str)> {
    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }
    let (name, args) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
    Some((name.to_lowercase(), args.trim()))
}

/// Lệnh khớp tên đang gõ. Có đối số vẫn lọc theo tên, để `/play abc` còn hiện dòng `/play`.
pub fn filter(input: &str) -> Vec<&'static CliSpec> {
    match split(input) {
        Some((name, _)) => COMMANDS.iter().filter(|spec| spec.name.contains(name.as_str())).collect(),
        None => Vec::new(),
    }
}

fn find(name: &str) -> Option<&'static CliSpec> {
    COMMANDS.iter().find(|spec| spec.name == name)
}

/// Đọc đúng chữ đang gõ, không có dòng chọn.
pub fn parse(input: &str) -> Result<CliCommand, CliError> {
    resolve(input, None)
}

/// Lệnh cho phím Enter (S21): dòng đang chọn thắng tên đang gõ, đối số lấy từ chữ đang gõ.
pub fn resolve(input: &str, selected: Option<&CliSpec>) -> Result<CliCommand, CliError> {
    let (name, args) = split(input).ok_or(CliError::NotACommand)?;
    let spec = selected.or_else(|| find(&name)).ok_or(CliError::Unknown(name))?;
    build(spec, args)
}

fn build(spec: &CliSpec, args: &str) -> Result<CliCommand, CliError> {
    let optional = (!args.is_empty()).then(|| args.to_owned());
    Ok(match spec.name {
        "/play" => CliCommand::Play(optional),
        "/playall" => CliCommand::PlayAll,
        "/queue" => CliCommand::Queue(optional.ok_or(CliError::MissingArg(spec.name))?),
        "/pause" => CliCommand::Pause,
        "/next" => CliCommand::Next,
        "/prev" => CliCommand::Prev,
        "/volume" => {
            if args.is_empty() {
                return Err(CliError::MissingArg(spec.name));
            }
            let volume: i64 = args.parse().map_err(|_| CliError::InvalidVolume(args.to_owned()))?;
            CliCommand::Volume(volume.clamp(0, 100) as u8)
        }
        "/theme" => CliCommand::Theme(match args.to_lowercase().as_str() {
            "" => None,
            "dark" => Some(ThemeChoice::Dark),
            "light" => Some(ThemeChoice::Light),
            "system" => Some(ThemeChoice::System),
            _ => return Err(CliError::InvalidTheme(args.to_owned())),
        }),
        "/help" => CliCommand::Help,
        other => return Err(CliError::Unknown(other.to_owned())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(input: &str) -> Vec<&'static str> {
        filter(input).into_iter().map(|spec| spec.name).collect()
    }

    fn spec(name: &str) -> &'static CliSpec {
        find(name).unwrap()
    }

    #[test]
    fn every_spec_builds_or_asks_for_its_argument() {
        for spec in &COMMANDS {
            let result = parse(spec.name);
            match spec.arg {
                CliArg::Required => assert_eq!(result, Err(CliError::MissingArg(spec.name))),
                _ => assert!(result.is_ok(), "{}", spec.name),
            }
        }
    }

    #[test]
    fn parses_each_command() {
        assert_eq!(parse("/play"), Ok(CliCommand::Play(None)));
        assert_eq!(
            parse("  /PLAY  Em của ngày  hôm qua "),
            Ok(CliCommand::Play(Some("Em của ngày  hôm qua".to_owned())))
        );
        assert_eq!(parse("/playall"), Ok(CliCommand::PlayAll));
        assert_eq!(parse("/queue abc"), Ok(CliCommand::Queue("abc".to_owned())));
        assert_eq!(parse("/pause"), Ok(CliCommand::Pause));
        assert_eq!(parse("/next"), Ok(CliCommand::Next));
        assert_eq!(parse("/prev"), Ok(CliCommand::Prev));
        assert_eq!(parse("/help"), Ok(CliCommand::Help));
    }

    #[test]
    fn volume_is_clamped_and_checked() {
        assert_eq!(parse("/volume 50"), Ok(CliCommand::Volume(50)));
        assert_eq!(parse("/volume 150"), Ok(CliCommand::Volume(100)));
        assert_eq!(parse("/volume -5"), Ok(CliCommand::Volume(0)));
        assert_eq!(parse("/volume to"), Err(CliError::InvalidVolume("to".to_owned())));
    }

    #[test]
    fn theme_argument() {
        assert_eq!(parse("/theme"), Ok(CliCommand::Theme(None)));
        assert_eq!(parse("/theme Light"), Ok(CliCommand::Theme(Some(ThemeChoice::Light))));
        assert_eq!(parse("/theme system"), Ok(CliCommand::Theme(Some(ThemeChoice::System))));
        assert_eq!(parse("/theme pink"), Err(CliError::InvalidTheme("pink".to_owned())));
    }

    #[test]
    fn not_a_command_or_unknown() {
        assert!(!is_cli("abc"));
        assert!(is_cli(" /abc"));
        assert_eq!(parse("abc"), Err(CliError::NotACommand));
        assert_eq!(parse("/abc"), Err(CliError::Unknown("/abc".to_owned())));
    }

    #[test]
    fn filter_follows_the_typed_name() {
        assert_eq!(names("/").len(), COMMANDS.len());
        assert_eq!(names("/pl"), ["/play", "/playall"]);
        assert_eq!(names("/play abc"), ["/play", "/playall"]);
        assert_eq!(names("/volume 5"), ["/volume"]);
        assert_eq!(names("/zzz"), Vec::<&str>::new());
        assert_eq!(names("abc"), Vec::<&str>::new());
    }

    /// S21: Enter ở `/n` chạy lệnh đang chọn, không chạy chữ đang gõ.
    #[test]
    fn enter_runs_the_selected_command() {
        let first = filter("/n")[0];
        assert_eq!(resolve("/n", Some(first)), Ok(CliCommand::Next));
        // Không dòng nào chọn được: chữ gõ là lệnh lạ, không âm thầm làm gì.
        assert_eq!(resolve("/n", None), Err(CliError::Unknown("/n".to_owned())));
        // Dời con trỏ xuống dòng khác thì dòng đó thắng cả tên gõ đủ.
        assert_eq!(resolve("/play", Some(spec("/playall"))), Ok(CliCommand::PlayAll));
    }

    #[test]
    fn selected_command_takes_the_typed_argument() {
        assert_eq!(resolve("/vol 30", Some(spec("/volume"))), Ok(CliCommand::Volume(30)));
        assert_eq!(resolve("/pl abc", Some(spec("/play"))), Ok(CliCommand::Play(Some("abc".to_owned()))));
        assert_eq!(resolve("/vol", Some(spec("/volume"))), Err(CliError::MissingArg("/volume")));
    }

    #[test]
    fn tab_fills_the_name() {
        assert_eq!(spec("/play").completion(), "/play ");
        assert_eq!(spec("/volume").completion(), "/volume ");
        assert_eq!(spec("/theme").completion(), "/theme ");
        assert_eq!(spec("/next").completion(), "/next");
    }
}
