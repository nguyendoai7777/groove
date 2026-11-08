//! groove-gd2_03: hộp "Save as new" — cắt một bài ra tệp mới (chủ dự án 08/11/2026: "cắt nhanh theo duration và export nhiều
//! loại định dạng như: current, mp3, m4r").
//!
//! Điểm đầu / cuối: ô `m:ss` + thanh trượt, thanh đoạn đã chọn nằm dưới. Định dạng: `groove_audio::export::plan` cho biết cách
//! làm (cắt không mã lại / mã lại) hoặc vì sao khoá (OS chưa có bộ mã hoá, D-2). Mặc định lưu cạnh tệp gốc, tên
//! `<gốc> (cut)` không trùng tệp có sẵn; "Chọn…" mở hộp lưu của OS. Xuất chạy ở luồng riêng (`ExportJob`, N7), có tiến độ và
//! huỷ; xong thì toast, tuỳ chọn thêm bài mới vào thư viện (`LibraryCtl::add_file`).
//!
//! Một entity cho mỗi view chủ: `LibraryCtl` (menu bài của danh sách, vẽ ở lớp phủ trang thư viện) và `NowPlaying` (menu hàng
//! đợi), như hộp lyrics / timeline.

use std::path::{Path, PathBuf};
use std::time::Duration;

use groove_audio::decoder::TrackDecoder;
use groove_audio::export::{
    self, ExportError, ExportFormat, ExportJob, ExportRequest, MIN_CLIP_SEC, Method, RINGTONE_MAX_SEC, Unavailable,
};
use groove_core::model::{Song, TrackRef};
use groove_core::t;
use groove_library::store::Library;
use materiant::button::Button;
use materiant::checkbox::Checkbox;
use materiant::field::input::TextField;
use materiant::gpui_kit::base::Disableable;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled,
    Task, Window, div, px, relative,
};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::progress::ProgressBar;
use materiant::radio::{RadioGroup, RadioItem};
use materiant::slider::Slider;
use materiant::theme::ActiveTheme;
use materiant::toast::{Toast, ToastKind, ToastSpec};

use crate::library_ctl;

const DIALOG_WIDTH: f32 = 560.;
const TIME_FIELD_WIDTH: f32 = 96.;
/// Nhịp đọc tiến độ của luồng xuất.
const POLL: Duration = Duration::from_millis(100);
/// Bước của thanh trượt (giây).
const SLIDER_STEP: f32 = 0.1;

// ---- Hàm thuần ----

/// `"65"`, `"1:05"`, `"1:05.5"`, `"1:02:03"` ⇒ giây. Âm, rỗng, sai dạng ⇒ `None`.
pub fn parse_time(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let (last, whole) = parts.split_last()?;
    let seconds: f64 = last.parse().ok().filter(|s: &f64| s.is_finite() && *s >= 0.0)?;
    if !whole.is_empty() && seconds >= 60.0 {
        return None;
    }
    let mut total = 0.0;
    for part in whole {
        let n: u32 = part.parse().ok()?;
        total = total * 60.0 + f64::from(n);
    }
    let digits_ok = parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit() || c == '.'));
    digits_ok.then_some(total * 60.0 + seconds)
}

/// Giây ⇒ `m:ss`, thêm một chữ số lẻ khi không tròn giây (`1:05.5`); từ một giờ `h:mm:ss`.
pub fn format_time(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    let (whole, frac) = (tenths / 10, tenths % 10);
    let (h, m, s) = (whole / 3600, whole / 60 % 60, whole % 60);
    let base = if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") };
    if frac == 0 { base } else { format!("{base}.{frac}") }
}

/// Tên (không đuôi) chưa có trong `folder` với đuôi nào trong `extensions`: `<gốc> (cut)`, `<gốc> (cut 2)`…
pub fn unique_stem(folder: &Path, original: &str, extensions: &[String], exists: impl Fn(&Path) -> bool) -> String {
    (1..)
        .map(|n| if n == 1 { format!("{original} (cut)") } else { format!("{original} (cut {n})") })
        .find(|stem| extensions.iter().all(|ext| !exists(&folder.join(format!("{stem}.{ext}")))))
        .unwrap_or_else(|| format!("{original} (cut)"))
}

/// Lỗi của đoạn chọn, hoặc `None` khi lưu được.
pub fn range_error(start: Option<f64>, end: Option<f64>, duration: f64) -> Option<&'static str> {
    let (Some(start), Some(end)) = (start, end) else { return Some(t!("save_as.time_invalid")) };
    if start > duration + 0.05 || end > duration + 0.05 {
        return Some(t!("save_as.time_invalid"));
    }
    (end - start < MIN_CLIP_SEC).then_some(t!("save_as.order_invalid"))
}

pub fn error_text(err: ExportError) -> &'static str {
    match err {
        ExportError::SameAsSource => t!("save_as.error.same_as_source"),
        ExportError::Exists => t!("save_as.error.exists"),
        ExportError::EmptyRange => t!("save_as.error.empty"),
        ExportError::Unavailable(_) => t!("save_as.error.unavailable"),
        ExportError::Source(_) => t!("save_as.error.source"),
        ExportError::Encoder => t!("save_as.error.encoder"),
        ExportError::Write => t!("save_as.error.write"),
        ExportError::Cancelled => "",
    }
}

fn plan_note(plan: Result<Method, Unavailable>) -> &'static str {
    match plan {
        Ok(method) if method.is_lossless_cut() => t!("save_as.note.copy"),
        Ok(_) => t!("save_as.note.encode"),
        Err(Unavailable::NoEncoderOnThisOs) => t!("save_as.note.no_encoder_os"),
        Err(Unavailable::NoEncoderForSource) => t!("save_as.note.no_encoder_source"),
    }
}

/// Định dạng mặc định: giữ gốc nếu được, không thì định dạng đầu tiên làm được.
fn default_format(source: &Path) -> Option<ExportFormat> {
    ExportFormat::ALL.into_iter().find(|f| export::plan(source, *f).is_ok())
}

// ---- Hộp thoại ----

/// Bài đang cắt, đọc ở nền lúc mở.
#[derive(Clone, Debug)]
struct Source {
    path: PathBuf,
    title: String,
    duration: f64,
    /// `<gốc> (cut)` chưa trùng tệp nào trong thư mục gốc.
    stem: String,
}

enum Load {
    Loading,
    Failed,
    Ready(Source),
}

pub struct SaveAsDialog {
    handle: DialogHandle,
    library: Option<Library>,
    load: Load,
    start: f64,
    end: f64,
    start_field: Entity<TextField>,
    end_field: Entity<TextField>,
    name_field: Entity<TextField>,
    format: Option<ExportFormat>,
    folder: PathBuf,
    /// Đích người dùng đã đồng ý ghi đè ở hộp lưu của OS.
    overwrite: Option<PathBuf>,
    add_to_library: bool,
    job: Option<ExportJob>,
    progress: f32,
    error: Option<&'static str>,
    _load_task: Option<Task<()>>,
    _poll_task: Option<Task<()>>,
}

impl SaveAsDialog {
    pub fn new(library: Option<Library>, window: &mut Window, cx: &mut Context<Self>) -> SaveAsDialog {
        let this = cx.entity().downgrade();
        let time_field = |id: &'static str, label: &'static str, end: bool, window: &mut Window, cx: &mut App| {
            let this = this.clone();
            cx.new(|cx| {
                TextField::new(id, window, cx)
                    .label(label)
                    .label_weight(FontWeight::NORMAL)
                    .placeholder(t!("save_as.time_hint"))
                    .on_change(move |value: &SharedString, _, cx| {
                        let value = value.to_string();
                        this.update(cx, |this, cx| this.typed_time(end, &value, cx)).ok();
                    })
            })
        };
        let start_field = time_field("save-as-start", t!("save_as.start"), false, window, cx);
        let end_field = time_field("save-as-end", t!("save_as.end"), true, window, cx);
        let name_field = cx.new(|cx| {
            TextField::new("save-as-name", window, cx).label(t!("save_as.name")).label_weight(FontWeight::NORMAL)
        });
        SaveAsDialog {
            handle: DialogHandle::new(cx),
            library,
            load: Load::Loading,
            start: 0.0,
            end: 0.0,
            start_field,
            end_field,
            name_field,
            format: None,
            folder: PathBuf::new(),
            overwrite: None,
            add_to_library: true,
            job: None,
            progress: 0.0,
            error: None,
            _load_task: None,
            _poll_task: None,
        }
    }

    /// Mở cho `track`: hiện hộp ngay, đọc đường dẫn / độ dài / tên trống ở nền. Đang lưu dở thì chỉ đưa hộp lên.
    pub fn open(&mut self, track: TrackRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.job.is_some() {
            self.handle.open(window, cx);
            return;
        }
        self.load = Load::Loading;
        self.error = None;
        self.progress = 0.0;
        self.overwrite = None;
        for field in [&self.start_field, &self.end_field, &self.name_field] {
            field.update(cx, |field, cx| {
                field.set_value("", window, cx);
                field.set_error(None, cx);
            });
        }
        self.handle.open(window, cx);
        let library = self.library.clone();
        self._load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let song = match &track {
                TrackRef::External(path) => Some(Song::external(path)),
                TrackRef::Library(_) => match library {
                    Some(library) => {
                        library.songs_for_tracks(vec![track.clone()]).await.ok().and_then(|s| s.into_iter().next())
                    }
                    None => None,
                },
            };
            let source = match song {
                Some(song) => cx.background_spawn(async move { read_source(song) }).await,
                None => None,
            };
            this.update_in(cx, |this, window, cx| this.loaded(source, window, cx)).ok();
        }));
        cx.notify();
    }

    fn loaded(&mut self, source: Option<Source>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = source else {
            self.load = Load::Failed;
            cx.notify();
            return;
        };
        self.start = 0.0;
        self.end = source.duration;
        self.format = default_format(&source.path);
        self.folder = source.path.parent().map(Path::to_path_buf).unwrap_or_default();
        let (start, end, stem) = (format_time(self.start), format_time(self.end), source.stem.clone());
        self.start_field.update(cx, |f, cx| f.set_value(start, window, cx));
        self.end_field.update(cx, |f, cx| f.set_value(end, window, cx));
        self.name_field.update(cx, |f, cx| f.set_value(stem, window, cx));
        self.load = Load::Ready(source);
        cx.notify();
    }

    fn source(&self) -> Option<&Source> {
        match &self.load {
            Load::Ready(source) => Some(source),
            _ => None,
        }
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    #[cfg(test)]
    pub fn is_ready(&self) -> bool {
        self.source().is_some()
    }

    #[cfg(test)]
    pub fn save_for_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save(window, cx);
    }

    #[cfg(test)]
    pub fn set_range_for_test(&mut self, start: f64, end: f64, format: ExportFormat) {
        (self.start, self.end, self.format) = (start, end, Some(format));
    }

    // ---- Nhập ----

    fn typed_time(&mut self, end: bool, text: &str, cx: &mut Context<Self>) {
        let Some(duration) = self.source().map(|s| s.duration) else { return };
        let field = if end { self.end_field.clone() } else { self.start_field.clone() };
        match parse_time(text).filter(|t| *t <= duration + 0.05) {
            Some(value) => {
                let value = value.min(duration);
                if end {
                    self.end = value
                } else {
                    self.start = value
                }
                field.update(cx, |f, cx| f.set_error(None, cx));
            }
            None => field.update(cx, |f, cx| f.set_error(Some(t!("save_as.time_invalid").into()), cx)),
        }
        cx.notify();
    }

    fn slid(&mut self, end: bool, value: f32, window: &mut Window, cx: &mut Context<Self>) {
        let value = f64::from(value);
        let field = if end {
            self.end = value;
            self.end_field.clone()
        } else {
            self.start = value;
            self.start_field.clone()
        };
        field.update(cx, |f, cx| {
            f.set_value(format_time(value), window, cx);
            f.set_error(None, cx);
        });
        cx.notify();
    }

    fn pick_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(format) = self.format else { return };
        let Some(source) = self.source() else { return };
        let ext = export::extension(&source.path, format);
        let suggested = format!("{}.{ext}", self.name_field.read(cx).value(cx));
        let picked = cx.prompt_for_new_path(&self.folder, Some(&suggested));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else { return };
            this.update_in(cx, |this, window, cx| {
                if let Some(folder) = path.parent() {
                    this.folder = folder.to_path_buf();
                }
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                this.name_field.update(cx, |f, cx| {
                    f.set_value(stem, window, cx);
                    f.set_error(None, cx);
                });
                // Hộp của OS đã hỏi ghi đè; chỉ áp khi đuôi khớp định dạng đang chọn.
                this.overwrite = Some(path);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- Lưu ----

    fn destination(&self, cx: &App) -> Option<PathBuf> {
        let source = self.source()?;
        let name = self.name_field.read(cx).value(cx).trim().to_owned();
        if name.is_empty() {
            return None;
        }
        Some(self.folder.join(format!("{name}.{}", export::extension(&source.path, self.format?))))
    }

    fn can_save(&self, cx: &App) -> bool {
        let Some(source) = self.source() else { return false };
        !self.busy()
            && self.format.is_some_and(|f| export::plan(&source.path, f).is_ok())
            && range_error(Some(self.start), Some(self.end), source.duration).is_none()
            && self.destination(cx).is_some()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save(cx) {
            return;
        }
        let (Some(source), Some(dest), Some(format)) = (self.source().cloned(), self.destination(cx), self.format)
        else {
            return;
        };
        let request = ExportRequest {
            source: source.path,
            overwrite: self.overwrite.as_ref() == Some(&dest),
            dest,
            start_sec: self.start,
            end_sec: self.end,
            format,
        };
        let job = match ExportJob::spawn(request) {
            Ok(job) => job,
            Err(err) => {
                log::error!("save as: cannot start the export thread: {err}");
                self.error = Some(t!("save_as.error.encoder"));
                cx.notify();
                return;
            }
        };
        self.job = Some(job);
        self.progress = 0.0;
        self.error = None;
        self.name_field.update(cx, |f, cx| f.set_error(None, cx));
        self._poll_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let done = this.update_in(cx, |this, window, cx| this.poll(window, cx));
                if !matches!(done, Ok(false)) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// `true` khi lượt xuất đã xong.
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(job) = &self.job else { return true };
        self.progress = job.progress();
        let Some(result) = job.try_finish() else {
            cx.notify();
            return false;
        };
        self.job = None;
        self.finished(result, window, cx);
        true
    }

    fn finished(&mut self, result: Result<PathBuf, ExportError>, window: &mut Window, cx: &mut Context<Self>) {
        match result {
            Ok(path) => {
                self.handle.close(window, cx);
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                match library_ctl::ctl(cx).filter(|_| self.add_to_library) {
                    Some(ctl) => ctl.update(cx, |ctl, cx| ctl.add_file(path, name, cx)),
                    None => {
                        Toast::push(window, cx, ToastSpec::new(ToastKind::Success, t!("save_as.saved", name = name)));
                    }
                }
            }
            Err(ExportError::Cancelled) => {}
            Err(ExportError::Exists) => {
                self.name_field.update(cx, |f, cx| f.set_error(Some(t!("save_as.error.exists").into()), cx));
            }
            Err(err) => self.error = Some(error_text(err)),
        }
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.job {
            Some(job) => job.cancel(),
            None => self.handle.close(window, cx),
        }
    }

    // ---- Vẽ ----

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors().clone();
        let source = match &self.load {
            Load::Loading => return div().text_color(c.muted).child(t!("save_as.loading")).into_any_element(),
            Load::Failed => return div().text_color(c.danger).child(t!("save_as.unreadable")).into_any_element(),
            Load::Ready(source) => source.clone(),
        };
        let busy = self.busy();
        let duration = source.duration.max(MIN_CLIP_SEC) as f32;
        let time_row = |id: &'static str, field: &Entity<TextField>, value: f64, end: bool| {
            div()
                .flex()
                .items_end()
                .gap_3()
                .child(div().w(px(TIME_FIELD_WIDTH)).flex_none().child(field.clone()))
                .child(
                    Slider::new(id)
                        .min(0.)
                        .max(duration)
                        .step(SLIDER_STEP)
                        .value(value as f32)
                        .disabled(busy)
                        .accessibility_label(if end { t!("save_as.end") } else { t!("save_as.start") })
                        .on_change(cx.listener(move |this, v: &f32, window, cx| this.slid(end, *v, window, cx)))
                        .flex_1(),
                )
        };
        let (a, b) = ((self.start / source.duration).clamp(0.0, 1.0), (self.end / source.duration).clamp(0.0, 1.0));
        let range_bar =
            div().relative().h(px(6.)).w_full().rounded_full().bg(c.border).overflow_hidden().when(b > a, |bar| {
                bar.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(a as f32))
                        .w(relative((b - a) as f32))
                        .bg(c.primary),
                )
            });
        let length = (self.end - self.start).max(0.0);
        let range_note = match range_error(Some(self.start), Some(self.end), source.duration) {
            Some(err) => div().text_sm().text_color(c.danger).child(err),
            None => div().text_sm().text_color(c.muted).child(t!("save_as.length", length = format_time(length))),
        };
        let ringtone_warning = (self.format == Some(ExportFormat::M4r) && length > RINGTONE_MAX_SEC).then(|| {
            div().text_sm().text_color(c.warning).child(t!("save_as.ringtone_long", length = format_time(length)))
        });

        let current_ext = export::extension(&source.path, ExportFormat::Current).to_uppercase();
        let plans: Vec<_> = ExportFormat::ALL.iter().map(|f| export::plan(&source.path, *f)).collect();
        let items = ExportFormat::ALL.iter().zip(&plans).map(|(format, plan)| {
            let label: SharedString = match format {
                ExportFormat::Current => t!("save_as.format_label.current", ext = current_ext.clone()).into(),
                ExportFormat::Mp3 => t!("save_as.format_label.mp3").into(),
                ExportFormat::M4r => t!("save_as.format_label.m4r").into(),
            };
            RadioItem::new(label).disabled(busy || plan.is_err())
        });
        let selected = self.format.and_then(|f| ExportFormat::ALL.iter().position(|x| *x == f));
        let selected_note = selected.map(|ix| plan_note(plans[ix]));
        let locked: Vec<AnyElement> = ExportFormat::ALL
            .iter()
            .zip(&plans)
            .filter_map(|(format, plan)| {
                let reason = plan_note(*plan);
                plan.is_err().then(|| {
                    let name = match format {
                        ExportFormat::Current => current_ext.clone(),
                        ExportFormat::Mp3 => "MP3".to_owned(),
                        ExportFormat::M4r => "M4R".to_owned(),
                    };
                    div().text_sm().text_color(c.muted).child(format!("{name}: {reason}")).into_any_element()
                })
            })
            .collect();
        let formats = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_sm().text_color(c.muted).child(t!("save_as.format")))
            .child(RadioGroup::new("save-as-format").items(items).selected(selected).on_change(cx.listener(
                |this, ix: &usize, _, cx| {
                    this.format = ExportFormat::ALL.get(*ix).copied();
                    cx.notify();
                },
            )))
            .children(selected_note.map(|note| div().text_sm().text_color(c.muted).child(note)))
            .children(locked);

        let ext = self.format.map(|f| export::extension(&source.path, f)).unwrap_or_default();
        let location = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_sm().text_color(c.muted).child(t!("save_as.location")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    // Đường dẫn dài xuống dòng, không cắt "…".
                    .child(div().flex_1().min_w_0().text_sm().child(self.folder.to_string_lossy().into_owned()))
                    .child(
                        Button::new("save-as-pick", t!("save_as.change_location"))
                            .outlined()
                            .disabled(busy)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.pick_location(window, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(self.name_field.clone()))
                    .child(div().pb(px(6.)).text_color(c.muted).child(format!(".{ext}"))),
            );

        let add = Checkbox::new("save-as-add")
            .checked(self.add_to_library)
            .label(t!("save_as.add_to_library"))
            .disabled(busy)
            .on_change(cx.listener(|this, state: &materiant::checkbox::CheckState, _, cx| {
                this.add_to_library = state.is_marked();
                cx.notify();
            }));

        let progress = busy.then(|| {
            div().flex().flex_col().gap_1().child(ProgressBar::new("save-as-progress").value(self.progress)).child(
                div()
                    .text_sm()
                    .text_color(c.muted)
                    .child(t!("save_as.saving", percent = (self.progress * 100.0).round() as u32)),
            )
        });

        div()
            .flex()
            .flex_col()
            .gap_4()
            // Tên bài dài xuống dòng (không "…").
            .child(div().text_color(c.muted).child(source.title.clone()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(time_row("save-as-start-slider", &self.start_field, self.start, false))
                    .child(time_row("save-as-end-slider", &self.end_field, self.end, true))
                    .child(range_bar)
                    .child(range_note)
                    .children(ringtone_warning),
            )
            .child(formats)
            .child(location)
            .child(add)
            .children(progress)
            .children(self.error.map(|err| div().text_sm().text_color(c.danger).child(err)))
            .into_any_element()
    }
}

/// Nền: độ dài (DB có thì dùng, không thì đọc tệp) và tên trống cạnh tệp gốc. Tệp đọc không được ⇒ `None`.
fn read_source(song: Song) -> Option<Source> {
    let path = song.path.clone();
    let duration = if song.duration_sec > 0.0 {
        song.duration_sec
    } else {
        // Decoder có thể panic với tệp độc (S25).
        std::panic::catch_unwind(|| TrackDecoder::open(&path).map(|d| d.duration_sec())).ok()?.ok()?
    };
    if duration <= 0.0 || !path.is_file() {
        return None;
    }
    let original = path.file_stem()?.to_string_lossy().into_owned();
    let folder = path.parent()?.to_path_buf();
    let extensions: Vec<String> = ExportFormat::ALL.iter().map(|f| export::extension(&path, *f)).collect();
    let stem = unique_stem(&folder, &original, &extensions, |p| p.exists());
    let title = song.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| song.filename.clone());
    Some(Source { path, title, duration, stem })
}

impl Render for SaveAsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let can_save = self.can_save(cx);
        let body = self.body(cx);
        div().child(
            Dialog::new("save-as-dialog", &self.handle)
                .title(t!("save_as.title"))
                .title_weight(FontWeight::NORMAL)
                .max_width(px(DIALOG_WIDTH))
                .divided(true)
                // Đang lưu: chỉ nút Huỷ đóng được (huỷ lượt xuất).
                .close_on_escape(!busy)
                .close_on_backdrop(!busy)
                .actions([
                    Button::new("save-as-cancel", t!("save_as.cancel"))
                        .text()
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel(window, cx)))
                        .into_any_element(),
                    Button::new("save-as-save", t!("save_as.save"))
                        .filled()
                        .loading(busy)
                        .disabled(!can_save)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx)))
                        .into_any_element(),
                ])
                .child(body),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_accepts_seconds_and_clock_forms() {
        assert_eq!(parse_time("65"), Some(65.0));
        assert_eq!(parse_time("1:05"), Some(65.0));
        assert_eq!(parse_time(" 1:05.5 "), Some(65.5));
        assert_eq!(parse_time("1:02:03"), Some(3723.0));
        assert_eq!(parse_time("0:00"), Some(0.0));
        for bad in ["", "1:5x", "-3", "1:60", ":05", "1::05", "1:2:3:4", "a"] {
            assert_eq!(parse_time(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn format_time_round_trips() {
        assert_eq!(format_time(65.0), "1:05");
        assert_eq!(format_time(65.44), "1:05.4");
        assert_eq!(format_time(3723.0), "1:02:03");
        assert_eq!(format_time(-1.0), "0:00");
        for secs in [0.0, 9.5, 59.9, 61.0, 754.3] {
            assert_eq!(parse_time(&format_time(secs)), Some(secs));
        }
    }

    #[test]
    fn unique_stem_skips_taken_names_for_every_extension() {
        let folder = Path::new("nhac");
        let exts = ["mp3".to_owned(), "m4r".to_owned()];
        let taken = [folder.join("Bài (cut).m4r"), folder.join("Bài (cut 2).mp3")];
        let stem = unique_stem(folder, "Bài", &exts, |p| taken.iter().any(|t| t == p));
        assert_eq!(stem, "Bài (cut 3)");
        assert_eq!(unique_stem(folder, "Bài", &exts, |_| false), "Bài (cut)");
    }

    #[test]
    fn range_error_blocks_empty_reversed_and_out_of_song() {
        groove_core::i18n::set_language(groove_core::i18n::Language::En);
        assert_eq!(range_error(Some(0.0), Some(10.0), 10.0), None);
        assert_eq!(range_error(Some(5.0), Some(5.0), 10.0), Some(t!("save_as.order_invalid")));
        assert_eq!(range_error(Some(6.0), Some(5.0), 10.0), Some(t!("save_as.order_invalid")));
        assert_eq!(range_error(None, Some(5.0), 10.0), Some(t!("save_as.time_invalid")));
        assert_eq!(range_error(Some(0.0), Some(12.0), 10.0), Some(t!("save_as.time_invalid")));
    }

    #[test]
    fn every_export_error_has_text() {
        groove_core::i18n::set_language(groove_core::i18n::Language::Vi);
        for err in [
            ExportError::SameAsSource,
            ExportError::Exists,
            ExportError::EmptyRange,
            ExportError::Unavailable(Unavailable::NoEncoderOnThisOs),
            ExportError::Source(groove_audio::decoder::AudioErrorKind::Corrupt),
            ExportError::Encoder,
            ExportError::Write,
        ] {
            assert!(!error_text(err).is_empty(), "{err:?}");
        }
    }
}
