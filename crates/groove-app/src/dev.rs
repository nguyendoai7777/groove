//! Tham số chạy thử, chép từ `materiant/examples/common/dev.rs`. `tools/run.ps1` gói sẵn; cửa sổ thử chỉ mở ở màn phụ.
//!
//!   --display primary | secondary | <số thứ tự>   màn mở cửa sổ (mặc định: màn OS chọn)
//!   --background                                  hiện cửa sổ mà không kích hoạt, app đang dùng giữ focus
//!   --keys "down ctrl-t down@200,150 wait@500 up@200,150 shot@ten"
//!       phím, chuột trái (`down@x,y`, `up@`, `move@`, `drag@`, toạ độ trong cửa sổ), nghỉ (`wait@ms`), xin chụp
//!       (`shot@ten`, in `SHOT ten`); in `KEY <bước>` mỗi bước, cuối cùng `KEYS DONE`. `dbl@x,y`: lần nhấn thứ hai của
//!       nhấp đúp (`click_count` 2); `state@ten` in `STATE ten maximized=… bounds=…` của cửa sổ thật.

use std::time::Duration;

use materiant::gpui_kit::{
    AnyWindowHandle, App, AppContext, DisplayId, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PlatformInput, point, px,
};

/// Màn mở cửa sổ: `--display primary | secondary | <số thứ tự>`.
#[derive(Clone, Debug)]
pub enum DisplayChoice {
    Primary,
    Secondary,
    Index(usize),
}

impl DisplayChoice {
    pub fn parse(arg: &str) -> Option<Self> {
        match arg {
            "primary" => Some(Self::Primary),
            "secondary" => Some(Self::Secondary),
            n => n.parse().ok().map(Self::Index),
        }
    }

    pub fn resolve(&self, cx: &App) -> Option<DisplayId> {
        let primary = cx.primary_display().map(|d| d.id());
        let displays = cx.displays();
        match self {
            Self::Primary => primary,
            Self::Secondary => displays.iter().map(|d| d.id()).find(|id| Some(*id) != primary),
            Self::Index(i) => displays.get(*i).map(|d| d.id()),
        }
    }
}

/// Một bước của `--keys`: phím (`ctrl-t`), chuột trái tại toạ độ trong cửa sổ (`down@200,150`, `up@…`, `move@…`), hay
/// nghỉ (`wait@1000` ms), hay xin chụp ảnh (`shot@ten`, in `SHOT ten` — `tools/run.ps1` chụp `ten.png`).
/// Sau phím / `shot@` nghỉ 300 ms; sau chuột không nghỉ (tự thêm `wait@`).
pub enum Simulated {
    Key(Keystroke),
    Mouse(PlatformInput),
    Wait(u64),
    Shot(String),
    State(String),
}

impl Simulated {
    pub fn parse(token: &str) -> Result<Simulated, String> {
        let Some((kind, arg)) = token.split_once('@') else {
            return Keystroke::parse(Box::leak(token.to_owned().into_boxed_str()))
                .map(Simulated::Key)
                .map_err(|e| e.to_string());
        };
        if kind == "shot" {
            // Tên thành tên tệp ảnh ⇒ chỉ chữ, số, `-`, `_`.
            if arg.is_empty() || !arg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err("shot@<name>: letters, digits, - or _".into());
            }
            return Ok(Simulated::Shot(arg.to_owned()));
        }
        if kind == "state" {
            return Ok(Simulated::State(arg.to_owned()));
        }
        if kind == "wait" {
            return arg.parse().map(Simulated::Wait).map_err(|e| format!("wait@<ms>: {e}"));
        }
        let (x, y) = arg
            .split_once(',')
            .and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))
            .ok_or("expected <kind>@<x>,<y>")?;
        let position = point(px(x), px(y));
        let (button, modifiers) = (MouseButton::Left, Modifiers::default());
        Ok(Simulated::Mouse(match kind {
            "down" | "dbl" => PlatformInput::MouseDown(MouseDownEvent {
                button,
                position,
                modifiers,
                click_count: if kind == "dbl" { 2 } else { 1 },
                first_mouse: false,
            }),
            "up" => PlatformInput::MouseUp(MouseUpEvent { button, position, modifiers, click_count: 1 }),
            "move" => PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers }),
            "drag" => PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: Some(button), modifiers }),
            _ => return Err(format!("unknown `{kind}@` — down | dbl | up | move | drag | wait | shot | state")),
        }))
    }
}

/// Tham số dòng lệnh chung đã đọc.
pub struct DevArgs {
    pub display: Option<DisplayChoice>,
    pub background: bool,
    pub keys: Vec<(String, Simulated)>,
}

impl DevArgs {
    pub fn parse(args: &[String]) -> DevArgs {
        let display = arg_value(args, "--display").map(|v| {
            DisplayChoice::parse(v)
                .unwrap_or_else(|| exit_usage(format!("--display `{v}`: primary | secondary | <index>")))
        });
        let keys = arg_value(args, "--keys")
            .map(|seq| {
                seq.split_whitespace()
                    .map(|k| {
                        let step = Simulated::parse(k).unwrap_or_else(|e| exit_usage(format!("--keys `{k}`: {e}")));
                        (k.to_owned(), step)
                    })
                    .collect()
            })
            .unwrap_or_default();
        DevArgs { display, background: args.iter().any(|a| a == "--background"), keys }
    }

    /// Màn đã chọn. Chọn mà không thấy thì thoát, không rơi về màn mặc định: đó có thể là màn người dùng đang làm việc.
    pub fn display_id(&self, cx: &App) -> Option<DisplayId> {
        self.display
            .as_ref()
            .map(|d| d.resolve(cx).unwrap_or_else(|| exit_usage(format!("--display {d:?}: not found"))))
    }
}

pub fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

pub fn exit_usage(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

/// Chạy chuỗi `--keys` vào cửa sổ `handle` (không qua OS ⇒ không giành focus, không đụng con trỏ thật).
pub fn replay(handle: AnyWindowHandle, keys: Vec<(String, Simulated)>, cx: &mut App) {
    if keys.is_empty() {
        return;
    }
    cx.spawn(async move |cx| {
        // Chờ khung đầu vẽ xong rồi mới gõ.
        cx.background_executor().timer(Duration::from_millis(800)).await;
        for (name, input) in keys {
            println!("KEY {name}");
            let pause = match input {
                Simulated::Key(key) => {
                    let _ = cx.update_window(handle, |_, window, cx| window.dispatch_keystroke(key, cx));
                    300
                }
                Simulated::Mouse(event) => {
                    let _ = cx.update_window(handle, |_, window, cx| window.dispatch_event(event, cx));
                    0
                }
                Simulated::Wait(ms) => ms,
                Simulated::State(name) => {
                    let _ = cx.update_window(handle, |_, window, _| {
                        println!("STATE {name} maximized={} bounds={:?}", window.is_maximized(), window.bounds());
                    });
                    0
                }
                Simulated::Shot(name) => {
                    // `tools/run.ps1` thấy dòng này thì chụp; chờ cho kịp chụp trước bước kế.
                    println!("SHOT {name}");
                    300
                }
            };
            cx.background_executor().timer(Duration::from_millis(pause)).await;
        }
        println!("KEYS DONE");
    })
    .detach();
}
