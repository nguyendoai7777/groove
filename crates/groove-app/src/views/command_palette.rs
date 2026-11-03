//! groove-gd1_23: bảng lệnh (đặc tả 1.9).
//!
//! Dialog 600 px phía trên cửa sổ, mở bằng `ShellEvent::OpenSearch` (ô tìm, `Ctrl+K`). Ô nhập, nhãn chế độ (Tìm kiếm /
//! Chế độ lệnh), danh sách nhóm theo loại, chân trang gợi ý phím.
//!
//! - **Trống**: ba thao tác nhanh `commands::QUICK_ACTIONS`, không tiêu đề nhóm (chủ dự án bỏ); chọn thì chạy rồi đóng.
//! - **Có chữ**: tìm sau 250 ms, tối đa 50 bài. `Library::search` bỏ lượt cũ (`Superseded`, im lặng), kết quả có `seq` nhỏ
//!   hơn cái đang hiện cũng bỏ (S18). Chọn bài là `commands::play_search_result`, **không đóng** bảng.
//! - **Bắt đầu bằng `/`**: lọc `cli::COMMANDS`; Tab điền lệnh; Enter đi qua `cli::resolve` (dòng đang chọn thắng chữ gõ,
//!   S21) rồi `commands::run_cli`. Chạy được thì đóng; `/help` giữ bảng và điền `/`; lỗi hay không có bài thì báo ngay
//!   dưới ô nhập, không đóng.
//! - ↑ ↓ quay vòng và bỏ qua dòng tiêu đề nhóm; rê chuột dời con trỏ; Esc đóng (do `Dialog`).
//!
//! Phím ↑ ↓ Tab Enter đi qua bộ chặn phím của GPUI chứ không qua ô nhập: ô nhập một dòng giữ ↑ ↓ cho con trỏ chữ và Tab cho
//! vòng focus, bộ chặn chạy trước chúng.

use std::rc::Rc;
use std::time::Duration;

use groove_core::cli::{self, CliCommand, CliError, CliSpec};
use groove_core::keymap::Command;
use groove_core::model::Song;
use groove_core::t;
use groove_library::actor::LibraryError;
use materiant::badge::Badge;
use materiant::field::input::{FieldEvent, TextField};
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, AnyWindowHandle, App, Context, Entity, Focusable, FontWeight, IntoElement, Keystroke, ParentElement,
    Render, Styled, Subscription, Task, WeakEntity, Window, div, px,
};
use materiant::icon::Icon;
use materiant::kbd::Kbd;
use materiant::list::{Align, VirtualList, VirtualListHandle};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::theme::ActiveTheme;

use crate::app::{self, ShellEvent, Slot};
use crate::commands::{self, CliOutcome, QUICK_ACTIONS};
use crate::icons::GrooveIcon;
use crate::player::player;
use crate::views::playing_bars::PlayingBars;
use crate::views::song_row::display_title;

/// Bề ngang hộp (đặc tả 1.9).
const DIALOG_WIDTH: f32 = 600.;
/// Lề trong của bảng (ô tìm, danh sách, chân bảng); danh sách đặt nó bằng `padding_x` để thumb sát mép hộp (D-16).
const EDGE: f32 = 16.;
/// Cao hàng ô tìm (launcher fastcast: 56 px).
const SEARCH_HEIGHT: f32 = 56.;
/// Cao tối thiểu một dòng (tiêu đề nhóm cao đúng bằng); tên bài / mô tả dài xuống dòng thì dòng cao thêm.
const ROW_HEIGHT: f32 = 44.;
/// Cao tối đa của danh sách (bản cũ `max-h-cmd-list-max-h`).
const LIST_MAX_HEIGHT: f32 = 352.;
/// Chờ sau lần gõ cuối rồi mới tìm (đặc tả 1.9).
const DEBOUNCE: Duration = Duration::from_millis(250);
/// Số bài tối đa (đặc tả 1.9).
const SEARCH_LIMIT: usize = 50;
/// Số lần thử lại khi lượt tìm bị lượt khác (ngoài bảng này) đè.
const SEARCH_ATTEMPTS: usize = 2;

/// Một dòng của danh sách phẳng.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Header(&'static str),
    Quick(Command),
    /// Chỉ số trong `songs`.
    Song(usize),
    Cli(&'static CliSpec),
}

impl Row {
    fn selectable(self) -> bool {
        !matches!(self, Row::Header(_))
    }
}

/// Dòng chọn được kế tiếp theo hướng `step` (+1 xuống, -1 lên), quay vòng, bỏ qua tiêu đề nhóm.
fn step_selection(rows: &[Row], from: Option<usize>, step: isize) -> Option<usize> {
    let len = rows.len() as isize;
    if len == 0 || !rows.iter().any(|row| row.selectable()) {
        return None;
    }
    let mut ix = match from {
        Some(ix) => ix as isize,
        None if step > 0 => -1,
        None => len,
    };
    loop {
        ix = (ix + step).rem_euclid(len);
        if rows[ix as usize].selectable() {
            return Some(ix as usize);
        }
    }
}

fn first_selectable(rows: &[Row]) -> Option<usize> {
    rows.iter().position(|row| row.selectable())
}

/// Nội dung đã tra chữ của một dòng, để danh sách ảo dựng không phải mượn `self`.
enum RowView {
    Header(&'static str),
    Item { icon: GrooveIcon, title: String, description: String, trailing: Trailing },
}

enum Trailing {
    None,
    Keys(String),
    /// Bài đang chọn của player; `true` là đang phát.
    Bars(bool),
}

fn quick_texts(command: Command) -> (&'static str, &'static str) {
    match command {
        Command::PlayRandom => (t!("palette.quick.play_random"), t!("palette.quick.play_random_desc")),
        Command::GoToLibrary => (t!("palette.quick.go_to_library"), t!("palette.quick.go_to_library_desc")),
        Command::GoToNowPlaying => (t!("palette.quick.go_to_now_playing"), t!("palette.quick.go_to_now_playing_desc")),
        // Chỉ ba lệnh trong `QUICK_ACTIONS` tới được đây.
        other => (other.name(), ""),
    }
}

fn cli_description(spec: &CliSpec) -> &'static str {
    match spec.name {
        "/play" => t!("palette.cli.play"),
        "/playall" => t!("palette.cli.playall"),
        "/queue" => t!("palette.cli.queue"),
        "/pause" => t!("palette.cli.pause"),
        "/next" => t!("palette.cli.next"),
        "/prev" => t!("palette.cli.prev"),
        "/volume" => t!("palette.cli.volume"),
        "/theme" => t!("palette.cli.theme"),
        "/help" => t!("palette.cli.help"),
        _ => "",
    }
}

/// Lời báo cho lỗi đọc lệnh, kèm cách gỡ.
fn cli_error_message(error: &CliError) -> String {
    match error {
        CliError::NotACommand => String::new(),
        CliError::Unknown(name) => t!("palette.error.unknown", name = name),
        CliError::MissingArg(name) => t!("palette.error.missing_arg", name = name),
        CliError::InvalidVolume(value) => t!("palette.error.invalid_volume", value = value),
        CliError::InvalidTheme(value) => t!("palette.error.invalid_theme", value = value),
    }
}

/// `Bài hát • <nghệ sĩ> • <album>`, bỏ phần trống.
fn song_description(song: &Song) -> String {
    let mut parts = vec![t!("palette.song_kind").to_owned()];
    for part in [&song.artist, &song.album].into_iter().flatten() {
        let part = part.trim();
        if !part.is_empty() {
            parts.push(part.to_owned());
        }
    }
    parts.join(" • ")
}

type Runner = Rc<dyn Fn(Command, &mut App)>;
type Labeler = Rc<dyn Fn(Command, &App) -> Option<String>>;

pub struct CommandPalette {
    handle: DialogHandle,
    field: Entity<TextField>,
    list: VirtualListHandle,
    query: String,
    songs: Vec<Song>,
    /// `seq` của kết quả đang hiện: kết quả cũ hơn bị bỏ.
    shown_seq: u64,
    searching: bool,
    rows: Vec<Row>,
    selected: Option<usize>,
    /// Lời báo dưới ô nhập (lệnh sai, không có bài).
    notice: Option<String>,
    search_task: Option<Task<()>>,
    cli_task: Option<Task<()>>,
    /// Chạy một thao tác nhanh. Mặc định `commands::run`; test thay bằng bản ghi lại.
    runner: Runner,
    /// Nhãn phím của một thao tác nhanh. Mặc định `commands::key_label`.
    labeler: Labeler,
    _subscriptions: Vec<Subscription>,
}

impl CommandPalette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_hooks(Rc::new(commands::run), Rc::new(commands::key_label), window, cx)
    }

    fn with_hooks(runner: Runner, labeler: Labeler, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| {
            TextField::new("palette-input", window, cx)
                .placeholder(t!("palette.placeholder"))
                .leading_icon(materiant::icon::IconName::Search)
                .flat(true)
                .clearable(true)
        });
        let mut subscriptions = vec![cx.subscribe_in(&field, window, |this, _, event: &FieldEvent, window, cx| {
            if let FieldEvent::Change(value) = event {
                this.on_input(value.to_string(), window, cx);
            }
        })];
        // Thanh bars và dòng "đang phát" theo player; bảng đóng thì không vẽ gì, không dựng lại.
        subscriptions.push(cx.observe(&player(cx), |this, _, cx| {
            if this.handle.is_open() {
                cx.notify();
            }
        }));
        let this = cx.entity().downgrade();
        subscriptions.push(cx.intercept_keystrokes(move |event, window, cx| {
            if let Some(this) = this.upgrade() {
                let handled = this.update(cx, |this, cx| this.on_keystroke(&event.keystroke, window, cx));
                if handled {
                    cx.stop_propagation();
                }
            }
        }));
        let mut palette = CommandPalette {
            handle: DialogHandle::new(cx),
            field,
            list: VirtualListHandle::new(),
            query: String::new(),
            songs: Vec::new(),
            shown_seq: 0,
            searching: false,
            rows: Vec::new(),
            selected: None,
            notice: None,
            search_task: None,
            cli_task: None,
            runner,
            labeler,
            _subscriptions: subscriptions,
        };
        palette.rebuild();
        palette
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    #[cfg(test)]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Mở với ô trống, con trỏ ở dòng đầu, focus vào ô nhập.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle.is_open() {
            self.focus_input(window, cx);
            return;
        }
        self.search_task = None;
        self.cli_task = None;
        self.songs.clear();
        self.shown_seq = 0;
        self.searching = false;
        self.notice = None;
        self.query.clear();
        self.field.update(cx, |field, cx| field.set_value("", window, cx));
        self.rebuild();
        self.handle.open(window, cx);
        self.focus_input(window, cx);
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_task = None;
        self.songs = Vec::new();
        self.cli_task = None;
        self.handle.close(window, cx);
        cx.notify();
    }

    fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.field.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }

    fn is_cli(&self) -> bool {
        cli::is_cli(&self.query)
    }

    /// Dựng lại danh sách phẳng theo chữ đang gõ, đặt con trỏ về dòng đầu chọn được.
    fn rebuild(&mut self) {
        self.rows.clear();
        if self.query.trim().is_empty() {
            self.rows.extend(QUICK_ACTIONS.iter().map(|command| Row::Quick(*command)));
        } else if self.is_cli() {
            let specs = cli::filter(&self.query);
            if !specs.is_empty() {
                self.rows.push(Row::Header(t!("palette.group.commands")));
                self.rows.extend(specs.into_iter().map(Row::Cli));
            }
        } else if !self.songs.is_empty() {
            self.rows.push(Row::Header(t!("palette.group.songs")));
            self.rows.extend((0..self.songs.len()).map(Row::Song));
        }
        self.selected = first_selectable(&self.rows);
        // Cùng số dòng mà chữ khác (gõ thêm chữ) ⇒ đo lại cao dòng.
        self.list.remeasure();
        self.list.scroll_to(0, Align::Start);
    }

    fn on_input(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.query = value;
        self.notice = None;
        self.search_task = None;
        let text = self.query.trim().to_owned();
        if text.is_empty() || self.is_cli() {
            self.songs.clear();
            self.shown_seq = 0;
            self.searching = false;
            self.rebuild();
        } else {
            self.searching = true;
            self.search_task = Some(self.spawn_search(text, window, cx));
        }
        cx.notify();
    }

    fn spawn_search(&self, query: String, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        let library = app::try_library(cx).cloned();
        cx.spawn_in(window, async move |this: WeakEntity<Self>, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let result = match library {
                Some(library) => {
                    let mut result = Err(LibraryError::Superseded);
                    for _ in 0..SEARCH_ATTEMPTS {
                        result = library.search(query.clone(), SEARCH_LIMIT).await;
                        if !matches!(result, Err(LibraryError::Superseded)) {
                            break;
                        }
                    }
                    result
                }
                None => Err(LibraryError::Closed),
            };
            let _ = this.update(cx, |this, cx| this.apply_search(result, cx));
        })
    }

    fn apply_search(
        &mut self,
        result: Result<groove_library::store::SearchResults, LibraryError>,
        cx: &mut Context<Self>,
    ) {
        self.searching = false;
        match result {
            Ok(found) if found.seq >= self.shown_seq => {
                self.shown_seq = found.seq;
                self.songs = found.songs;
                self.rebuild();
            }
            // Kết quả cũ hơn cái đang hiện (S18).
            Ok(_) => {}
            Err(LibraryError::Superseded) => {}
            Err(error) => {
                log::warn!("palette search: {error}");
                self.songs.clear();
                self.rebuild();
                self.notice = Some(t!("palette.search_failed").to_owned());
            }
        }
        cx.notify();
    }

    /// Trả `true` nếu phím đã dùng (bộ chặn dừng lan truyền).
    fn on_keystroke(&mut self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.handle.is_open() || keystroke.modifiers.modified() {
            return false;
        }
        match keystroke.key.as_str() {
            "down" => self.move_selection(1, cx),
            "up" => self.move_selection(-1, cx),
            "tab" if self.is_cli() => self.complete(window, cx),
            "enter" => self.submit(window, cx),
            _ => return false,
        }
        true
    }

    fn move_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        if let Some(ix) = step_selection(&self.rows, self.selected, step) {
            self.set_selected(ix);
            cx.notify();
        }
    }

    fn set_selected(&mut self, ix: usize) {
        self.selected = Some(ix);
        self.list.scroll_to(ix, Align::Nearest);
    }

    /// Tab: điền lệnh đang chọn vào ô.
    fn complete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Row::Cli(spec)) = self.selected.map(|ix| self.rows[ix]) {
            let text = spec.completion();
            self.field.update(cx, |field, cx| field.set_value(text.clone(), window, cx));
            self.on_input(text, window, cx);
        }
    }

    fn selected_row(&self) -> Option<Row> {
        self.selected.and_then(|ix| self.rows.get(ix).copied())
    }

    /// Enter, hoặc nhấp dòng đang chọn.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_cli() {
            let selected = match self.selected_row() {
                Some(Row::Cli(spec)) => Some(spec),
                _ => None,
            };
            match cli::resolve(&self.query, selected) {
                Ok(command) => self.run_cli(command, window, cx),
                Err(error) => {
                    self.notice = Some(cli_error_message(&error));
                    cx.notify();
                }
            }
            return;
        }
        match self.selected_row() {
            Some(Row::Quick(command)) => {
                (self.runner)(command, cx);
                self.close(window, cx);
            }
            // Chọn bài không đóng bảng (đặc tả 1.9).
            Some(Row::Song(ix)) => {
                if let Some(song) = self.songs.get(ix) {
                    commands::play_search_result(song.track.clone(), cx);
                }
            }
            _ => {}
        }
    }

    fn run_cli(&mut self, command: CliCommand, window: &mut Window, cx: &mut Context<Self>) {
        let is_play_all = command == CliCommand::PlayAll;
        let task = commands::run_cli(command, cx);
        self.cli_task = Some(cx.spawn_in(window, async move |this: WeakEntity<Self>, cx| {
            let outcome = task.await;
            let _ = this.update_in(cx, |this, window, cx| match outcome {
                CliOutcome::Done => this.close(window, cx),
                CliOutcome::ShowHelp => {
                    this.field.update(cx, |field, cx| field.set_value("/", window, cx));
                    this.on_input("/".to_owned(), window, cx);
                }
                CliOutcome::NoMatch => {
                    let message = if is_play_all { t!("palette.playall_empty") } else { t!("palette.not_found") };
                    this.notice = Some(message.to_owned());
                    cx.notify();
                }
            });
        }));
    }

    /// Rê chuột dời con trỏ.
    fn hover(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.selected != Some(ix) && self.rows.get(ix).is_some_and(|row| row.selectable()) {
            self.selected = Some(ix);
            cx.notify();
        }
    }

    fn row_views(&self, cx: &App) -> Vec<RowView> {
        let current = player(cx).read(cx).current_track().cloned();
        let playing = player(cx).read(cx).is_playing();
        self.rows
            .iter()
            .map(|row| match *row {
                Row::Header(title) => RowView::Header(title),
                Row::Quick(command) => {
                    let (title, description) = quick_texts(command);
                    let trailing = (self.labeler)(command, cx).map_or(Trailing::None, Trailing::Keys);
                    RowView::Item {
                        icon: GrooveIcon::Action,
                        title: title.to_owned(),
                        description: description.to_owned(),
                        trailing,
                    }
                }
                Row::Song(ix) => {
                    let song = &self.songs[ix];
                    let trailing =
                        if current.as_ref() == Some(&song.track) { Trailing::Bars(playing) } else { Trailing::None };
                    RowView::Item {
                        icon: GrooveIcon::Song,
                        title: display_title(song),
                        description: song_description(song),
                        trailing,
                    }
                }
                Row::Cli(spec) => RowView::Item {
                    icon: GrooveIcon::Command,
                    title: spec.name.to_owned(),
                    description: cli_description(spec).to_owned(),
                    trailing: Trailing::Keys("Tab".to_owned()),
                },
            })
            .collect()
    }

    fn render_list(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors().clone();
        let message = self.empty_message();
        if self.rows.is_empty() {
            return div()
                .py_8()
                .flex()
                .justify_center()
                .text_center()
                .text_size(px(14.))
                .text_color(c.muted)
                .child(message.unwrap_or_default())
                .into_any_element();
        }
        let views = Rc::new(self.row_views(cx));
        let entity = cx.entity();
        let on_select = {
            let entity = entity.clone();
            move |ix: &usize, window: &mut Window, cx: &mut App| {
                let ix = *ix;
                entity.update(cx, |this, cx| {
                    if !this.rows.get(ix).is_some_and(|row| row.selectable()) {
                        return;
                    }
                    // Nhấp vào dòng đang chọn (rê chuột đã chọn nó) là chạy; dòng khác thì chỉ chọn.
                    let run = this.selected == Some(ix);
                    this.set_selected(ix);
                    this.focus_input(window, cx);
                    if run {
                        this.submit(window, cx);
                    }
                    cx.notify();
                });
            }
        };
        let list = VirtualList::new("palette-list", views.len(), px(ROW_HEIGHT), move |ix, _, _| {
            let view = &views[ix];
            let hover_entity = entity.clone();
            match view {
                RowView::Header(title) => div()
                    .px_3()
                    .pb_1()
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .flex()
                    .items_end()
                    .text_size(px(12.))
                    .text_color(c.muted)
                    .child(*title)
                    .into_any_element(),
                RowView::Item { icon, title, description, trailing } => div()
                    .id(("palette-row", ix))
                    .px_3()
                    .py_1p5()
                    .w_full()
                    .min_h(px(ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .gap_3()
                    .on_mouse_move(move |_, _, cx| hover_entity.update(cx, |this, cx| this.hover(ix, cx)))
                    .child(Icon::new(*icon).size(px(16.)).color(c.muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(14.)).child(title.clone()))
                            .child(div().text_size(px(12.)).text_color(c.muted).child(description.clone())),
                    )
                    .child(match trailing {
                        Trailing::None => div().into_any_element(),
                        Trailing::Keys(keys) => Kbd::new(keys).into_any_element(),
                        Trailing::Bars(active) => PlayingBars::new(*active).into_any_element(),
                    })
                    .into_any_element(),
            }
        })
        .track(&self.list)
        .selected(self.selected)
        .on_select(on_select)
        .on_activate(|_, _, _| {})
        .padding_x(px(EDGE))
        // Cao theo dòng đã đo (tên dài xuống dòng), tối đa `LIST_MAX_HEIGHT` rồi cuộn.
        .fit_content()
        .max_h(px(LIST_MAX_HEIGHT));
        let _ = window;
        list.into_any_element()
    }

    /// Chữ khi không có dòng nào.
    fn empty_message(&self) -> Option<String> {
        if let Some(notice) = &self.notice {
            return Some(notice.clone());
        }
        if self.searching {
            return Some(t!("palette.searching").to_owned());
        }
        if self.is_cli() {
            return Some(t!("palette.error.unknown", name = self.query.split_whitespace().next().unwrap_or("/")));
        }
        Some(t!("palette.no_results", query = self.query.trim()))
    }

    fn footer(&self, cx: &App) -> AnyElement {
        let c = cx.theme().colors();
        let hint = |keys: &'static str, label: &'static str| {
            div().flex().items_center().gap_1p5().child(Kbd::new(keys)).child(label)
        };
        let dot = || div().opacity(0.5).child("•");
        let left = div().flex().items_center().gap_2().child(hint("↑ ↓", t!("palette.footer.move"))).child(dot());
        let left = if self.is_cli() {
            left.child(hint("Tab", t!("palette.footer.choose_command")))
                .child(dot())
                .child(hint("Enter", t!("palette.footer.run")))
        } else {
            left.child(hint("Enter", t!("palette.footer.choose")))
        };
        div()
            .pt_3()
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(11.))
            .text_color(c.muted)
            .child(left)
            .child(hint("Esc", t!("palette.footer.close")))
            .into_any_element()
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Chỉ chế độ lệnh có nhãn; chế độ tìm kiếm không còn nút "Search" (G-6: chữ độ đậm normal).
        let mode = self.is_cli().then(|| Badge::new(t!("palette.mode_cli")).font_weight(FontWeight::NORMAL));
        let border = cx.theme().colors().border;
        let notice = self.notice.clone().filter(|_| !self.rows.is_empty());
        let danger = cx.theme().colors().danger;
        let list = self.render_list(window, cx);
        let footer = self.footer(cx);
        // D-16: hộp không đệm (`flush_body`), danh sách chạm hai mép hộp, lề `EDGE` nằm trong danh sách (`padding_x`); ô tìm,
        // dòng báo, chân bảng tự lùi cùng số đó.
        Dialog::new("command-palette", &self.handle)
            .max_width(px(DIALOG_WIDTH))
            .align_top(true)
            .divided(true)
            .flush_body(true)
            .child(
                div()
                    .flex()
                    .flex_col()
                    // Ô tìm kiểu launcher fastcast: liền, icon ở đầu, gạch ngăn dưới ô.
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px(px(EDGE))
                            .pr(px(EDGE + 4.))
                            .h(px(SEARCH_HEIGHT))
                            .border_b_1()
                            .border_color(border)
                            .child(div().flex_1().min_w_0().child(self.field.clone()))
                            .children(mode),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .py(px(EDGE))
                            .children(
                                notice.map(|text| div().px(px(EDGE)).text_size(px(12.)).text_color(danger).child(text)),
                            )
                            .child(list)
                            .child(div().px(px(EDGE)).child(footer)),
                    )
                    .into_any_element(),
            )
    }
}

/// Dựng bảng vào `Slot::CommandPalette` và mở khi `ShellEvent::OpenSearch`. Gọi một lần trong `app::run`.
pub fn install(window: AnyWindowHandle, cx: &mut App) {
    let result = window.update(cx, |_, window, cx| {
        let palette = cx.new(|cx| CommandPalette::new(window, cx));
        app::install_slot(Slot::CommandPalette, palette.clone(), cx);
        let shell = app::shell(cx);
        window
            .subscribe(&shell, cx, move |_, event: &ShellEvent, window, cx| {
                if *event == ShellEvent::OpenSearch {
                    palette.update(cx, |palette, cx| palette.open(window, cx));
                }
            })
            .detach();
    });
    if let Err(err) = result {
        log::error!("cannot install the command palette: {err}");
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::Path;
    use std::time::Instant;

    use gpui::{TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::model::TrackRef;
    use groove_library::db::migrate::{CoverRecord, CoverSink};
    use groove_library::db::queries::{self, SongUpsert};
    use groove_library::fixtures::{self, AudioKind};
    use groove_library::store::{Library, LibraryConfig};
    use materiant::gpui_kit::base::Root;
    use tempfile::TempDir;

    use super::*;
    use crate::player::{self, PlayerDeps};

    /// C7 — ↑ ↓ quay vòng và bỏ qua tiêu đề nhóm.
    #[test]
    fn selection_wraps_and_skips_headers() {
        let rows = [Row::Header("a"), Row::Song(0), Row::Song(1), Row::Header("b"), Row::Song(2)];
        assert_eq!(first_selectable(&rows), Some(1));
        assert_eq!(step_selection(&rows, Some(1), 1), Some(2));
        assert_eq!(step_selection(&rows, Some(2), 1), Some(4));
        assert_eq!(step_selection(&rows, Some(4), 1), Some(1));
        assert_eq!(step_selection(&rows, Some(1), -1), Some(4));
        assert_eq!(step_selection(&rows, None, 1), Some(1));
        assert_eq!(step_selection(&rows, None, -1), Some(4));
        assert_eq!(step_selection(&[Row::Header("a")], None, 1), None);
        assert_eq!(step_selection(&[], None, 1), None);
    }

    /// Mô tả bài bỏ phần trống.
    #[test]
    fn song_description_skips_empty_parts() {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        let mut song = Song::external("a.mp3");
        assert_eq!(song_description(&song), "Bài hát");
        song.artist = Some("Ca sĩ".to_owned());
        song.album = Some("  ".to_owned());
        assert_eq!(song_description(&song), "Bài hát • Ca sĩ");
        song.album = Some("Album".to_owned());
        assert_eq!(song_description(&song), "Bài hát • Ca sĩ • Album");
    }

    fn no_covers() -> Box<dyn CoverSink + Send> {
        Box::new(|_: &[u8]| -> anyhow::Result<CoverRecord> { anyhow::bail!("no covers in this test") })
    }

    fn add_song(library: &Library, path: &Path, title: &str) -> i64 {
        let (path, title) = (path.to_owned(), title.to_owned());
        library
            .writer()
            .transaction(move |conn| {
                let upsert = SongUpsert {
                    path,
                    folder_id: 1,
                    album_id: None,
                    title: Some(title),
                    artist: Some("Ca sĩ".to_owned()),
                    album_artist: None,
                    duration_sec: 1.0,
                    track_no: None,
                    disc_no: None,
                    lyrics: None,
                    timeline: None,
                    cover_hash: None,
                    file_mtime: None,
                    file_size: None,
                    tags_read_at: None,
                };
                conn.execute("INSERT OR IGNORE INTO folders (id, name, path) VALUES (1, 'f', 'f')", [])
                    .map_err(queries::QueryError::from)?;
                Ok(queries::upsert_song(conn, &upsert)?.id)
            })
            .wait()
            .unwrap()
    }

    struct Rig<'a> {
        cx: &'a mut VisualTestContext,
        palette: Entity<CommandPalette>,
        ran: Rc<RefCell<Vec<Command>>>,
        songs: Vec<TrackRef>,
        _dir: TempDir,
    }

    impl Rig<'_> {
        fn draw(&mut self) {
            self.cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            });
        }

        fn press(&mut self, keys: &str) {
            self.cx.simulate_keystrokes(keys);
            self.cx.run_until_parked();
            self.draw();
        }

        fn type_text(&mut self, text: &str) {
            self.cx.simulate_input(text);
            self.cx.run_until_parked();
            self.draw();
        }

        /// Qua hạn debounce rồi chờ kết quả tìm ở luồng nền.
        fn settle(&mut self) {
            self.cx.executor().advance_clock(DEBOUNCE);
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                self.cx.run_until_parked();
                if !self.palette.read_with(self.cx, |p, _| p.searching) {
                    break;
                }
                assert!(Instant::now() < deadline, "hết giờ chờ tìm");
                std::thread::sleep(Duration::from_millis(5));
            }
            self.draw();
        }

        fn open(&mut self) {
            let palette = self.palette.clone();
            self.cx.update(|window, cx| palette.update(cx, |p, cx| p.open(window, cx)));
            self.draw();
        }

        fn state<R>(&mut self, read: impl FnOnce(&CommandPalette, &App) -> R) -> R {
            let palette = self.palette.clone();
            self.cx.update(|_, cx| read(palette.read(cx), cx))
        }

        fn is_open(&mut self) -> bool {
            self.state(|p, _| p.is_open())
        }

        fn rows(&mut self) -> Vec<Row> {
            self.state(|p, _| p.rows.clone())
        }

        fn current(&mut self) -> Option<TrackRef> {
            self.cx.update(|_, cx| player(cx).read(cx).current_track().cloned())
        }
    }

    fn rig(cx: &mut TestAppContext) -> Rig<'_> {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        cx.executor().allow_parking();
        let dir = TempDir::new().unwrap();
        let config = LibraryConfig { target: dir.path().join("library.db"), legacy: None };
        let library = Library::open(config, no_covers()).unwrap();
        let wav = |name: &str| fixtures::write_sample(dir.path(), name, AudioKind::Wav).unwrap();
        let songs: Vec<TrackRef> = ["Alpha", "Beta", "Gamma"]
            .iter()
            .map(|title| TrackRef::Library(add_song(&library, &wav(&title.to_lowercase()), title)))
            .collect();
        let ran: Rc<RefCell<Vec<Command>>> = Rc::default();
        let recorder = ran.clone();
        cx.update(|cx| {
            materiant::init(cx);
            app::set_library(library.clone(), cx);
            let deps = PlayerDeps {
                library: Some(library),
                output: Box::new(FakeOutput::new(48_000, 2)),
                options: EngineOptions {
                    stall_timeout: Duration::from_secs(3600),
                    reopen_interval: Duration::from_millis(1),
                    reopen_attempts: 1,
                    ..EngineOptions::default()
                },
                session_file: None,
            };
            player::install_for_test(deps, cx);
        });
        let window = cx.add_window(|window, cx| {
            let palette = cx.new(|cx| {
                CommandPalette::with_hooks(
                    Rc::new(move |command, _| recorder.borrow_mut().push(command)),
                    Rc::new(|_, _| Some("Alt+P".to_owned())),
                    window,
                    cx,
                )
            });
            Root::new(palette, window, cx)
        });
        let palette =
            window.update(cx, |root, _, _| root.view().clone().downcast::<CommandPalette>().unwrap()).unwrap();
        let cx = VisualTestContext::from_window(window.into(), cx).into_mut();
        cx.simulate_resize(materiant::gpui_kit::size(px(800.), px(600.)));
        let mut rig = Rig { cx, palette, ran, songs, _dir: dir };
        rig.draw();
        rig.open();
        rig
    }

    /// C1 — mở ra: ba thao tác nhanh, con trỏ ở dòng đầu; Enter chạy thao tác rồi đóng.
    #[gpui::test]
    fn empty_shows_quick_actions_and_runs_one(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        assert_eq!(
            rig.rows(),
            [Row::Quick(Command::PlayRandom), Row::Quick(Command::GoToLibrary), Row::Quick(Command::GoToNowPlaying)]
        );
        assert_eq!(rig.state(|p, _| p.selected), Some(0));
        rig.press("down");
        rig.press("enter");
        assert_eq!(*rig.ran.borrow(), [Command::GoToLibrary]);
        assert!(!rig.is_open());
    }

    /// C2 — ↑ ↓ quay vòng, tới đủ ba thao tác rồi về đầu; ↑ ở đầu xuống cuối.
    #[gpui::test]
    fn arrows_wrap(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        for expected in [1, 2, 0] {
            rig.press("down");
            assert_eq!(rig.state(|p, _| p.selected), Some(expected));
        }
        rig.press("up");
        assert_eq!(rig.state(|p, _| p.selected), Some(2));
    }

    /// C3 — gõ chữ: tìm sau debounce, Enter phát bài và KHÔNG đóng bảng.
    #[gpui::test]
    fn typing_searches_and_choosing_a_song_keeps_it_open(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("beta");
        // Chưa qua 250 ms: chưa có kết quả.
        assert!(rig.state(|p, _| p.searching && p.songs.is_empty()));
        rig.settle();
        assert_eq!(rig.state(|p, _| p.songs.len()), 1);
        assert_eq!(rig.rows().len(), 2);
        rig.press("enter");
        let beta = rig.songs[1].clone();
        assert_eq!(rig.current(), Some(beta));
        assert!(rig.is_open());
    }

    /// C4 — gõ nhanh: chỉ lượt cuối được tìm, kết quả là của lượt cuối.
    #[gpui::test]
    fn fast_typing_keeps_only_the_last_search(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("alp");
        rig.cx.executor().advance_clock(DEBOUNCE / 2);
        rig.press("backspace backspace backspace");
        rig.type_text("gamma");
        rig.settle();
        let titles: Vec<_> = rig.state(|p, _| p.songs.iter().map(|s| s.title.clone().unwrap()).collect());
        assert_eq!(titles, ["Gamma"]);
    }

    /// C5 — không có bài: báo, Enter không làm gì.
    #[gpui::test]
    fn no_results_message(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("zzzz");
        rig.settle();
        assert!(rig.rows().is_empty());
        assert_eq!(rig.state(|p, _| p.empty_message()), Some(t!("palette.no_results", query = "zzzz")));
        rig.press("enter");
        assert!(rig.is_open());
        assert_eq!(rig.current(), None);
    }

    /// C6 — `/`: lọc lệnh, Tab điền lệnh, chế độ lệnh.
    #[gpui::test]
    fn slash_filters_commands_and_tab_completes(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("/vo");
        assert_eq!(rig.rows().len(), 2);
        assert!(rig.state(|p, _| p.is_cli()));
        rig.press("tab");
        assert_eq!(rig.state(|p, cx| p.field.read(cx).value(cx).to_string()), "/volume ");
        assert_eq!(rig.state(|p, _| p.query().to_owned()), "/volume ");
    }

    /// C7 — Enter chạy lệnh đang chọn (S21) rồi đóng; `/n` là `/next`.
    #[gpui::test]
    fn enter_runs_the_selected_command_and_closes(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("/volume 30");
        rig.press("enter");
        rig.cx.run_until_parked();
        assert!(!rig.is_open());
        rig.open();
        rig.type_text("/n");
        rig.press("enter");
        rig.cx.run_until_parked();
        assert!(!rig.is_open());
    }

    /// C8 — lệnh sai: báo cách gỡ, không đóng.
    #[gpui::test]
    fn bad_command_shows_a_notice_and_stays(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("/volume");
        rig.press("enter");
        assert!(rig.is_open());
        assert_eq!(rig.state(|p, _| p.notice.clone()), Some(t!("palette.error.missing_arg", name = "/volume")));
        rig.press("backspace");
        assert_eq!(rig.state(|p, _| p.notice.clone()), None, "gõ tiếp thì lời báo biến mất");
        rig.type_text("e x");
        rig.press("enter");
        assert_eq!(rig.state(|p, _| p.notice.clone()), Some(t!("palette.error.invalid_volume", value = "x")));
    }

    /// C9 — `/help` giữ bảng, điền `/`, hiện đủ lệnh.
    #[gpui::test]
    fn help_keeps_the_palette_and_lists_commands(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("/help");
        rig.press("enter");
        assert!(rig.is_open());
        assert_eq!(rig.state(|p, _| p.query().to_owned()), "/");
        assert_eq!(rig.rows().len(), 1 + cli::COMMANDS.len());
    }

    /// C10 — `/play` không có bài khớp: báo không đóng.
    #[gpui::test]
    fn cli_without_match_reports(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("/play khong co bai nay");
        rig.press("enter");
        let deadline = Instant::now() + Duration::from_secs(15);
        while rig.state(|p, _| p.notice.is_none()) {
            assert!(Instant::now() < deadline, "hết giờ chờ");
            rig.cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rig.is_open());
        assert_eq!(rig.state(|p, _| p.notice.clone()), Some(t!("palette.not_found").to_owned()));
    }

    /// C11 — Esc đóng; mở lại thì ô trống, danh sách về thao tác nhanh.
    #[gpui::test]
    fn escape_closes_and_reopen_is_fresh(cx: &mut TestAppContext) {
        let mut rig = rig(cx);
        rig.type_text("beta");
        rig.settle();
        rig.press("escape");
        assert!(!rig.is_open());
        rig.open();
        assert_eq!(rig.state(|p, _| p.query().to_owned()), "");
        assert_eq!(rig.rows().len(), 3);
        assert_eq!(rig.state(|p, cx| p.field.read(cx).value(cx).to_string()), "");
    }
}
