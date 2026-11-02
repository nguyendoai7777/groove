//! groove-gd1_19: dialog sửa metadata 900 px (đặc tả 1.8).
//!
//! Mở: `MetadataDialog::open(ops, track, scope, window, cx)` trả `Entity<MetadataDialog>`; chủ (danh sách bài, menu bài,
//! thanh phát) giữ entity và vẽ nó như một con trong cây của mình (`Dialog` của materiant khai báo, không mở bằng code),
//! nghe `MetadataDialogEvent::Saved` để nạp lại danh sách và `Closed` để bỏ entity. `scope` có khi mở từ một mục
//! (album, thư mục) nhiều bài: hiện ô "áp cho cả mục".
//!
//! Thẻ đọc từ tệp qua `MetadataOps::read` (không từ DB), lưu qua `save` rồi `save_bulk`. UI không chạm đĩa: mọi việc
//! `await` trên luồng của thư viện (N7). Mỗi lỗi của `MetadataError` có câu riêng; báo cáo lô nêu bài nào lỗi, bài
//! nào chưa làm. Lưu xong gọi `refresh_current` của player để lời, bìa, tiêu đề của bài đang phát đọc lại.

use groove_core::model::{CategoryRef, TrackRef};
use groove_core::t;
use groove_library::actor::LibraryError;
use groove_library::metadata_ops::{
    BulkReport, FileNameError, MetadataError, MetadataOps, MetadataResult, MetadataView, SavedTrack, validate_file_name,
};
use groove_library::tags::{TagError, TagFields, WriteMode};
use materiant::button::Button;
use materiant::checkbox::{CheckState, Checkbox};
use materiant::field::FieldEvent;
use materiant::gpui_kit::prelude::*;
use materiant::gpui_kit::{
    AnyElement, App, Context, Entity, EventEmitter, IntoElement, KeyDownEvent, ParentElement, Render, Styled, Window,
    div, px,
};
use materiant::icon::{Icon, IconName};
use materiant::overlay::dialog::{Dialog, DialogHandle};
use materiant::select::{Select, SelectItem};
use materiant::tabs::{TabItem, Tabs};
use materiant::theme::{ActiveTheme, Neutral};

use super::cover_pane::{CoverPane, CoverPaneEvent};
use super::fields::{
    Field, Form, apply_grouping_fix, edit_for_track, group_edit, name_error_text, needs_grouping_fix,
    strip_known_extension,
};
use super::raw_frames::{RawFrames, junk_count};

/// Dialog mở từ một mục có nhiều bài (đặc tả 1.8: ô áp cho cả nhóm chỉ hiện khi mục có trên một bài).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupScope {
    pub category: CategoryRef,
    pub songs: usize,
}

#[derive(Clone, Debug)]
pub enum MetadataDialogEvent {
    /// Bài đang mở đã ghi xong (tệp, và DB nếu là bài thư viện). Nạp lại danh sách khi nhận.
    Saved,
    /// Dialog đã đóng (Esc, nút, hoặc lưu xong): chủ bỏ entity.
    Closed,
}

enum Phase {
    Loading,
    Ready,
    Failed(String),
}

const MODES: [WriteMode; 3] = [WriteMode::CleanKeepKnown, WriteMode::Clean, WriteMode::Merge];
const TAB_COMMON: usize = 0;
const TAB_DETAILS: usize = 1;
const TAB_RAW: usize = 2;

/// Chữ báo cáo lô.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkText {
    pub headline: String,
    pub problems: Vec<String>,
}

fn file_label(path: &std::path::Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// Báo cáo lô thành chữ: bao nhiêu bài đã lưu, từng bài lỗi kèm lý do, số bài chưa làm, bìa nhóm.
pub fn describe_bulk(report: &BulkReport) -> BulkText {
    let mut problems = Vec::new();
    if !report.failed.is_empty() {
        problems.push(t!("metadata.bulk.failed_title", count = report.failed.len()));
        for failure in &report.failed {
            let (path, reason) = (file_label(&failure.path), error_text(&failure.error));
            problems.push(if failure.error.file_written() {
                t!("metadata.bulk.failed_item_written", path = path, reason = reason)
            } else {
                t!("metadata.bulk.failed_item", path = path, reason = reason)
            });
        }
    }
    if !report.not_started.is_empty() {
        problems.push(t!("metadata.bulk.not_started", count = report.not_started.len()));
    }
    if report.group_cover_failed {
        problems.push(t!("metadata.bulk.group_cover_failed").to_owned());
    }
    BulkText { headline: t!("metadata.bulk.done", count = report.done.len()), problems }
}

pub fn bulk_has_problems(report: &BulkReport) -> bool {
    !report.failed.is_empty() || !report.not_started.is_empty() || report.group_cover_failed
}

fn tag_error_text(error: &TagError) -> String {
    let path = |p: &std::path::Path| p.display().to_string();
    match error {
        TagError::NotFound(p) => t!("metadata.error.tag_not_found", path = path(p)),
        TagError::ReadOnly(p) => t!("metadata.error.tag_read_only", path = path(p)),
        TagError::UnsupportedFormat(p) => t!("metadata.error.tag_unsupported_format", path = path(p)),
        TagError::UnsupportedCover => t!("metadata.error.tag_unsupported_cover").to_owned(),
        TagError::CoverTooLarge => t!("metadata.error.tag_cover_too_large").to_owned(),
        TagError::Read { path: p, message } => t!("metadata.error.tag_read", path = path(p), message = message),
        TagError::Write { path: p, message } => t!("metadata.error.tag_write", path = path(p), message = message),
        TagError::Verify { path: p, field } => t!("metadata.error.tag_verify", path = path(p), field = field),
        TagError::Replace { path: p, .. } => t!("metadata.error.tag_replace", path = path(p)),
        TagError::Panicked(p) => t!("metadata.error.tag_panicked", path = path(p)),
        TagError::Io { path: p, source } => t!("metadata.error.tag_io", path = path(p), message = source),
    }
}

fn library_error_text(error: &LibraryError) -> String {
    match error {
        LibraryError::Closed => t!("metadata.error.library_closed").to_owned(),
        other => t!("metadata.error.library", message = other),
    }
}

/// Câu riêng cho từng biến thể `MetadataError`. Câu của `RenameFailed`, `DatabaseAfterWrite` nói rõ thẻ đã ghi.
pub fn error_text(error: &MetadataError) -> String {
    match error {
        MetadataError::InvalidName(name) => name_error_text(name),
        MetadataError::TargetExists(path) => t!("metadata.error.target_exists", path = file_label(path)),
        MetadataError::SongNotFound(_) => t!("metadata.error.song_not_found").to_owned(),
        MetadataError::Tag(tag) => tag_error_text(tag),
        MetadataError::RenameFailed { source, .. } => t!("metadata.error.rename_failed", message = source),
        MetadataError::DatabaseAfterWrite(_) => t!("metadata.error.database_after_write").to_owned(),
        MetadataError::Library(library) => library_error_text(library),
    }
}

/// Lỗi tên tệp khi gõ, theo luật của `MetadataOps` (không chạm đĩa).
fn name_problem(stem: &str, extension: &str, current: &str) -> Option<FileNameError> {
    if stem.trim().is_empty() {
        return Some(FileNameError::Empty);
    }
    validate_file_name(&format!("{}{extension}", stem.trim()), current).err()
}

pub struct MetadataDialog {
    ops: MetadataOps,
    track: TrackRef,
    scope: Option<GroupScope>,
    handle: DialogHandle,
    phase: Phase,
    view: Option<MetadataView>,
    form: Option<Form>,
    cover: Option<Entity<CoverPane>>,
    compilation: bool,
    tab: usize,
    mode: WriteMode,
    apply_group: bool,
    saving: bool,
    /// Đã có ít nhất một lần lưu xong (nút Huỷ thành Đóng).
    saved: bool,
    name_error: Option<String>,
    error: Option<String>,
    bulk: Option<BulkText>,
    closed_sent: bool,
}

impl EventEmitter<MetadataDialogEvent> for MetadataDialog {}

impl MetadataDialog {
    /// Mở dialog và bắt đầu đọc thẻ từ tệp. Chủ vẽ entity trả về trong cây của mình.
    pub fn open(
        ops: MetadataOps,
        track: TrackRef,
        scope: Option<GroupScope>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<MetadataDialog> {
        cx.new(|cx| MetadataDialog::new(ops, track, scope, window, cx))
    }

    fn new(
        ops: MetadataOps,
        track: TrackRef,
        scope: Option<GroupScope>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> MetadataDialog {
        let handle = DialogHandle::new(cx);
        handle.open(window, cx);
        let (read_ops, read_track) = (ops.clone(), track.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = read_ops.read(read_track).await;
            let _ = this.update_in(cx, |this, window, cx| this.loaded(result, window, cx));
        })
        .detach();
        MetadataDialog {
            ops,
            track,
            scope,
            handle,
            phase: Phase::Loading,
            view: None,
            form: None,
            cover: None,
            compilation: false,
            tab: TAB_COMMON,
            mode: WriteMode::default(),
            apply_group: false,
            saving: false,
            saved: false,
            name_error: None,
            error: None,
            bulk: None,
            closed_sent: false,
        }
    }

    fn loaded(&mut self, result: MetadataResult<MetadataView>, window: &mut Window, cx: &mut Context<Self>) {
        let view = match result {
            Ok(view) => view,
            Err(error) => {
                self.phase = Phase::Failed(error_text(&error));
                cx.notify();
                return;
            }
        };
        let form = Form::new(&view, window, cx);
        cx.subscribe_in(&form.filename, window, |this, _, event: &FieldEvent, window, cx| {
            if let FieldEvent::Change(text) = event {
                this.filename_typed(text.as_ref(), window, cx);
            }
        })
        .detach();
        for (_, entity) in &form.fields {
            cx.subscribe(entity, |_, _, _: &FieldEvent, cx| cx.notify()).detach();
        }
        let original_cover = view.cover.clone();
        let cover = cx.new(|cx| CoverPane::new(original_cover, cx));
        cx.subscribe(&cover, |_, _, _: &CoverPaneEvent, cx| cx.notify()).detach();
        self.compilation = view.fields.compilation.unwrap_or(false);
        self.cover = Some(cover);
        self.form = Some(form);
        self.view = Some(view);
        self.phase = Phase::Ready;
        cx.notify();
    }

    /// Gõ ở ô tên: đuôi quen thuộc tự cắt (đuôi thật ở chip riêng), rồi kiểm tên.
    fn filename_typed(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        if let Some(stripped) = strip_known_extension(text) {
            form.filename.update(cx, |field, cx| field.set_value(stripped, window, cx));
        }
        self.revalidate_name(cx);
    }

    fn revalidate_name(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let stem = form.stem(cx);
        self.name_error = name_problem(&stem, &form.extension, &form.current_name).map(|error| name_error_text(&error));
        cx.notify();
    }

    // ---- Đọc trạng thái, cho test và chủ ----

    pub fn track(&self) -> &TrackRef {
        &self.track
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready)
    }

    #[cfg(test)]
    pub fn load_error(&self) -> Option<&str> {
        match &self.phase {
            Phase::Failed(text) => Some(text),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn is_saving(&self) -> bool {
        self.saving
    }

    #[cfg(test)]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub fn name_error(&self) -> Option<&str> {
        self.name_error.as_deref()
    }

    #[cfg(test)]
    pub fn bulk_report(&self) -> Option<&BulkText> {
        self.bulk.as_ref()
    }

    #[cfg(test)]
    pub fn view_data(&self) -> Option<&MetadataView> {
        self.view.as_ref()
    }

    #[cfg(test)]
    pub fn cover_pane(&self) -> Option<&Entity<CoverPane>> {
        self.cover.as_ref()
    }

    #[cfg(test)]
    pub fn mode(&self) -> WriteMode {
        self.mode
    }

    #[cfg(test)]
    pub fn applies_to_group(&self) -> bool {
        self.apply_group
    }

    /// Ô "áp cho cả mục" chỉ có khi mục có trên một bài.
    pub fn can_apply_to_group(&self) -> bool {
        self.scope.is_some_and(|scope| scope.songs > 1)
    }

    // ---- Sửa ----

    pub fn set_mode(&mut self, mode: WriteMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub fn set_apply_group(&mut self, apply: bool, cx: &mut Context<Self>) {
        self.apply_group = apply && self.can_apply_to_group();
        cx.notify();
    }

    pub fn set_compilation(&mut self, value: bool, cx: &mut Context<Self>) {
        self.compilation = value;
        cx.notify();
    }

    pub fn set_field(&mut self, field: Field, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = &self.form {
            form.set_value(field, value, window, cx);
            cx.notify();
        }
    }

    #[cfg(test)]
    /// Gõ vào ô tên như người dùng (kể cả cắt đuôi, kiểm tên).
    pub fn type_filename(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        form.filename.update(cx, |field, cx| field.set_value(text.to_owned(), window, cx));
        self.filename_typed(text, window, cx);
    }

    /// Album có nhưng nghệ sĩ album trống (đang hiện cảnh báo).
    pub fn grouping_warning(&self, cx: &App) -> bool {
        self.current_fields(cx).is_some_and(|fields| needs_grouping_fix(&fields))
    }

    /// Fix grouping: nghệ sĩ album = album, bật tuyển tập, bật áp cho nhóm nếu có.
    pub fn fix_grouping(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut fields) = self.current_fields(cx) else { return };
        apply_grouping_fix(&mut fields);
        self.set_field(Field::AlbumArtist, fields.album_artist.as_deref().unwrap_or_default(), window, cx);
        self.compilation = true;
        self.apply_group = self.can_apply_to_group();
        cx.notify();
    }

    fn current_fields(&self, cx: &App) -> Option<TagFields> {
        self.form.as_ref().map(|form| form.collect(self.compilation, cx))
    }

    // ---- Lưu ----

    pub fn can_save(&self) -> bool {
        self.is_ready() && !self.saving && self.name_error.is_none()
    }

    /// Lưu bài đang mở rồi, nếu bật, áp cho cả nhóm. Xong thì đóng; còn lỗi thì giữ dialog để người dùng đọc.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save() {
            return;
        }
        let (Some(form), Some(view), Some(cover)) = (&self.form, &self.view, &self.cover) else { return };
        let fields = form.collect(self.compilation, cx);
        let cover_edit = cover.read(cx).edit();
        let edit = edit_for_track(&fields, cover_edit.clone());
        let new_name = form.new_file_name(cx);
        let mode = self.mode;
        let group = (self.apply_group && self.can_apply_to_group()).then_some(self.scope).flatten().map(|scope| {
            let bulk_mode = if mode == WriteMode::Merge { WriteMode::Merge } else { WriteMode::CleanKeepKnown };
            (scope.category, group_edit(&view.fields, &fields, cover_edit), bulk_mode)
        });
        let (ops, track) = (self.ops.clone(), self.track.clone());
        self.saving = true;
        self.error = None;
        self.bulk = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let saved = ops.save(track, edit, mode, new_name).await;
            let bulk = match (&saved, group) {
                (Ok(_), Some((category, edit, bulk_mode))) => Some(ops.save_bulk(category, edit, bulk_mode).await),
                _ => None,
            };
            let _ = this.update_in(cx, |this, window, cx| this.finished(saved, bulk, window, cx));
        })
        .detach();
    }

    fn finished(
        &mut self,
        saved: MetadataResult<SavedTrack>,
        bulk: Option<MetadataResult<BulkReport>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        if let Err(error) = saved {
            if error.file_written() {
                // Thẻ đã vào tệp: bài đang phát phải đọc lại dù tên chưa đổi hoặc DB chưa theo.
                self.saved = true;
                refresh_player(cx);
            }
            self.error = Some(error_text(&error));
            cx.notify();
            return;
        }
        self.saved = true;
        let mut clean = true;
        match bulk {
            Some(Ok(report)) => {
                clean = !bulk_has_problems(&report);
                self.bulk = Some(describe_bulk(&report));
            }
            Some(Err(error)) => {
                clean = false;
                self.error = Some(error_text(&error));
            }
            None => {}
        }
        refresh_player(cx);
        cx.emit(MetadataDialogEvent::Saved);
        if clean {
            self.handle.close(window, cx);
        }
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.handle.close(window, cx);
        cx.notify();
    }

    /// Dialog bị đóng bằng đường nào cũng phát `Closed` đúng một lần (kể cả Esc, do `Dialog` tự đóng handle).
    fn note_closed(&mut self, cx: &mut Context<Self>) {
        if self.handle.is_open() || self.closed_sent {
            return;
        }
        self.closed_sent = true;
        let this = cx.entity();
        cx.defer(move |cx| this.update(cx, |_, cx| cx.emit(MetadataDialogEvent::Closed)));
    }

    // ---- Vẽ ----

    fn title(&self) -> String {
        match self.view.as_ref().map(|view| junk_count(&view.raw_frames)).filter(|&count| count > 0) {
            Some(count) => t!("metadata.title_junk", count = count),
            None => t!("metadata.title").to_owned(),
        }
    }

    fn field_pair(&self, a: Field, b: Field) -> Option<AnyElement> {
        let form = self.form.as_ref()?;
        Some(
            div()
                .flex()
                .gap_3()
                .child(div().flex_1().child(form.field(a).clone()))
                .child(div().flex_1().child(form.field(b).clone()))
                .into_any_element(),
        )
    }

    fn render_common(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (form, cover) = (self.form.as_ref()?, self.cover.as_ref()?);
        let c = cx.theme().colors().clone();
        let chip = div()
            .self_end()
            .h(px(32.))
            .px_3()
            .flex()
            .items_center()
            .rounded_md()
            .bg(c.layer(Neutral::N200))
            .text_color(c.muted)
            .child(form.extension.clone());
        let name_row = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(form.filename.clone()))
                    .when(!form.extension.is_empty(), |row| row.child(chip)),
            )
            .when_some(self.name_error.clone(), |col, text| col.child(inline_note(text, c.danger, IconName::Warning)));
        let warning = self.grouping_warning(cx).then(|| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(inline_note(t!("metadata.grouping.warning").to_owned(), c.warning, IconName::Warning))
                .child(
                    Button::new("metadata-fix-grouping", t!("metadata.grouping.fix"))
                        .outlined()
                        .warning()
                        .size(materiant::button::Size::Sm)
                        .on_click(cx.listener(|this, _, window, cx| this.fix_grouping(window, cx))),
                )
        });
        let fields = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_3()
            .child(name_row)
            .child(form.field(Field::Title).clone())
            .child(form.field(Field::Artist).clone())
            .child(form.field(Field::Album).clone())
            .child(form.field(Field::AlbumArtist).clone())
            .children(warning)
            .children(self.field_pair(Field::TrackNo, Field::TrackTotal))
            .children(self.field_pair(Field::DiscNo, Field::DiscTotal));
        Some(div().flex().gap_5().items_start().child(cover.clone()).child(fields).into_any_element())
    }

    fn render_details(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.form.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .children(self.field_pair(Field::Year, Field::Genre))
                .children(self.field_pair(Field::Composer, Field::Publisher))
                .children(self.field_pair(Field::Copyright, Field::Bpm))
                .child(form.field(Field::Grouping).clone())
                .child(form.comment.clone())
                .child(
                    Checkbox::new("metadata-compilation")
                        .label(t!("metadata.field.compilation"))
                        .checked(if self.compilation { CheckState::Checked } else { CheckState::Unchecked })
                        .on_change(cx.listener(|this, state: &CheckState, _, cx| {
                            this.set_compilation(*state == CheckState::Checked, cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn render_raw(&self) -> Option<AnyElement> {
        let view = self.view.as_ref()?;
        let is_mp3 = view.filename.to_ascii_lowercase().ends_with(".mp3");
        Some(RawFrames::new(view.raw_frames.clone(), is_mp3).into_any_element())
    }

    fn render_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors().clone();
        let mode_ix = MODES.iter().position(|mode| *mode == self.mode).unwrap_or(0);
        let hint = match self.mode {
            WriteMode::CleanKeepKnown => t!("metadata.mode.clean_keep_hint"),
            WriteMode::Clean => t!("metadata.mode.clean_hint"),
            WriteMode::Merge => t!("metadata.mode.merge_hint"),
        };
        let select = Select::new("metadata-mode")
            .label(t!("metadata.mode.label"))
            .items([
                SelectItem::new(t!("metadata.mode.clean_keep")),
                SelectItem::new(t!("metadata.mode.clean")),
                SelectItem::new(t!("metadata.mode.merge")),
            ])
            .selected(Some(mode_ix))
            .w(px(320.))
            .on_change(cx.listener(|this, ix: &usize, _, cx| {
                this.set_mode(MODES.get(*ix).copied().unwrap_or_default(), cx);
            }));
        let group = self.scope.filter(|scope| scope.songs > 1).map(|scope| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Checkbox::new("metadata-apply-group")
                        .label(t!("metadata.group.apply", count = scope.songs))
                        .checked(if self.apply_group { CheckState::Checked } else { CheckState::Unchecked })
                        .on_change(cx.listener(|this, state: &CheckState, _, cx| {
                            this.set_apply_group(*state == CheckState::Checked, cx);
                        })),
                )
                .child(div().text_sm().text_color(c.muted).child(t!("metadata.group.hint")))
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().flex_col().gap_1().child(select).child(div().text_sm().text_color(c.muted).child(hint)))
            .children(group)
            .into_any_element()
    }

    fn render_messages(&self, cx: &App) -> Option<AnyElement> {
        if self.error.is_none() && self.bulk.is_none() {
            return None;
        }
        let c = cx.theme().colors();
        let mut col = div().flex().flex_col().gap_1().p_3().rounded_md().bg(c.layer(Neutral::N100));
        if let Some(error) = &self.error {
            col = col.child(inline_note(error.clone(), c.danger, IconName::Warning));
        }
        if let Some(bulk) = &self.bulk {
            col = col.child(div().text_color(c.text).child(bulk.headline.clone()));
            for line in &bulk.problems {
                col = col.child(inline_note(line.clone(), c.warning, IconName::Warning));
            }
        }
        Some(col.into_any_element())
    }

    fn render_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors().clone();
        match &self.phase {
            Phase::Loading => return div().py_8().text_color(c.muted).child(t!("metadata.loading")).into_any_element(),
            Phase::Failed(text) => {
                return inline_note(format!("{}: {text}", t!("metadata.load_failed")), c.danger, IconName::Warning)
                    .into_any_element();
            }
            Phase::Ready => {}
        }
        let tabs = Tabs::new("metadata-tabs")
            .tab(TabItem::new(t!("metadata.tab.common")))
            .tab(TabItem::new(t!("metadata.tab.details")))
            .tab(TabItem::new(t!("metadata.tab.raw")))
            .selected(self.tab)
            .on_change(cx.listener(|this, ix: &usize, _, cx| {
                this.tab = *ix;
                cx.notify();
            }));
        let content = match self.tab {
            TAB_DETAILS => self.render_details(cx),
            TAB_RAW => self.render_raw(),
            _ => self.render_common(cx),
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(tabs)
            .children(content)
            .child(self.render_options(cx))
            .children(self.render_messages(cx))
            .into_any_element()
    }
}

fn refresh_player(cx: &mut App) {
    crate::player::player(cx).update(cx, |player, cx| player.refresh_current(cx));
}

fn inline_note(text: String, color: materiant::gpui_kit::Hsla, icon: IconName) -> impl IntoElement {
    div()
        .flex()
        .items_start()
        .gap_2()
        .text_sm()
        .text_color(color)
        .child(div().flex_none().mt(px(2.)).child(Icon::new(icon).size(px(14.)).color(color)))
        .child(div().flex_1().min_w_0().child(text))
}

impl Render for MetadataDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.note_closed(cx);
        let body = self.render_body(cx);
        let ready = self.is_ready();
        let cancel_label = if self.saved { t!("common.close") } else { t!("metadata.cancel") };
        let save_label = if self.saving { t!("metadata.saving") } else { t!("metadata.save") };
        Dialog::new("metadata-dialog", &self.handle)
            .max_width(px(900.))
            .title(self.title())
            .close_button(t!("metadata.close_label"))
            .close_on_escape(!self.saving)
            .close_on_backdrop(false)
            .actions([
                Button::new("metadata-cancel", cancel_label)
                    .text()
                    .base()
                    .disabled(self.saving)
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
                    .into_any_element(),
                Button::new("metadata-save", save_label)
                    .filled()
                    .primary()
                    .disabled(!ready || self.name_error.is_some())
                    .loading(self.saving)
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))
                    .into_any_element(),
            ])
            .child(
                div()
                    .id("metadata-body")
                    .flex()
                    .flex_col()
                    .flex_shrink(1.)
                    .min_h_0()
                    .overflow_y_scroll()
                    // Ctrl+V trên ô bìa dán ảnh; ngoài ô, hoặc clipboard không có ảnh, thì ô nhập tự dán chữ.
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        let keystroke = &event.keystroke;
                        let paste =
                            keystroke.key == "v" && keystroke.modifiers.secondary(cx) && !keystroke.modifiers.shift;
                        let Some(cover) = this.cover.clone().filter(|cover| paste && cover.read(cx).is_hovered())
                        else {
                            return;
                        };
                        if cover.update(cx, |cover, cx| cover.paste(cx)) {
                            cx.stop_propagation();
                        }
                    }))
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::Duration;

    use ::gpui::{TestAppContext, VisualTestContext};
    use groove_audio::engine::EngineOptions;
    use groove_audio::output::FakeOutput;
    use groove_core::i18n::{Language, set_language};
    use groove_library::actor::LibraryError;
    use groove_library::covers::CoverStore;
    use groove_library::fixtures::{TINY_PNG, write_mp3_with_frames};
    use groove_library::indexer::{TrackRow, apply_scanned_track};
    use groove_library::metadata_ops::BulkFailure;
    use groove_library::store::{Library, LibraryConfig};
    use groove_library::tags::{self, CoverEdit};
    use materiant::gpui_kit::base::Root;
    use tempfile::TempDir;

    use super::*;
    use crate::player::{self, PlayerDeps};

    // ---- Phần thuần: câu chữ của lỗi và báo cáo lô ----

    fn every_error() -> Vec<MetadataError> {
        let path = || PathBuf::from("C:/music/a.mp3");
        let io = || std::io::Error::other("busy");
        vec![
            MetadataError::InvalidName(FileNameError::TrailingDot),
            MetadataError::TargetExists(path()),
            MetadataError::SongNotFound(7),
            MetadataError::Tag(TagError::NotFound(path())),
            MetadataError::Tag(TagError::ReadOnly(path())),
            MetadataError::Tag(TagError::UnsupportedFormat(path())),
            MetadataError::Tag(TagError::UnsupportedCover),
            MetadataError::Tag(TagError::CoverTooLarge),
            MetadataError::Tag(TagError::Read { path: path(), message: "bad".into() }),
            MetadataError::Tag(TagError::Write { path: path(), message: "bad".into() }),
            MetadataError::Tag(TagError::Verify { path: path(), field: "title" }),
            MetadataError::Tag(TagError::Replace { path: path(), source: io() }),
            MetadataError::Tag(TagError::Panicked(path())),
            MetadataError::Tag(TagError::Io { path: path(), source: io() }),
            MetadataError::RenameFailed { from: path(), to: path(), source: io() },
            MetadataError::DatabaseAfterWrite(LibraryError::Closed),
            MetadataError::Library(LibraryError::Closed),
        ]
    }

    /// C6 — mỗi biến thể lỗi có câu riêng, không sót khoá hay biến (hai catalog cùng bộ khoá do test của `i18n` giữ).
    #[test]
    fn every_error_variant_has_its_own_sentence() {
        let mut seen = std::collections::HashSet::new();
        {
            let language = Language::Vi;
            set_language(language);
            for error in every_error() {
                let text = error_text(&error);
                assert!(!text.contains("metadata."), "thiếu khoá cho {error:?}: {text}");
                assert!(!text.contains('{'), "chưa thay biến: {text}");
                assert!(!text.trim().is_empty());
                if language == Language::Vi {
                    assert!(seen.insert(text.clone()), "hai biến thể trùng câu: {text}");
                }
            }
        }
        set_language(Language::Vi);
    }

    #[test]
    fn rename_and_database_errors_say_the_tags_were_saved() {
        set_language(Language::Vi);
        let rename = error_text(&MetadataError::RenameFailed {
            from: "a".into(),
            to: "b".into(),
            source: std::io::Error::other("busy"),
        });
        assert!(rename.contains("lưu thẻ"), "{rename}");
        assert!(error_text(&MetadataError::DatabaseAfterWrite(LibraryError::Closed)).contains("lưu thẻ"));
        assert!(error_text(&MetadataError::TargetExists("C:/m/x.mp3".into())).contains("x.mp3"));
        set_language(Language::Vi);
    }

    /// C7 — báo cáo lô nêu tên từng bài lỗi, đánh dấu bài đã ghi thẻ, đếm bài chưa làm, nhắc bìa nhóm.
    #[test]
    fn bulk_report_names_failed_songs_and_counts_the_rest() {
        set_language(Language::Vi);
        let report = BulkReport {
            done: vec![1, 2],
            failed: vec![
                BulkFailure {
                    id: 3,
                    path: "C:/m/locked.mp3".into(),
                    error: MetadataError::Tag(TagError::ReadOnly("C:/m/locked.mp3".into())),
                },
                BulkFailure {
                    id: 4,
                    path: "C:/m/late.mp3".into(),
                    error: MetadataError::DatabaseAfterWrite(LibraryError::Closed),
                },
            ],
            not_started: vec![5, 6, 7],
            group_cover_failed: true,
        };
        let text = describe_bulk(&report);
        assert_eq!(text.headline, "Đã lưu 2 bài khác trong mục này");
        let joined = text.problems.join("\n");
        assert!(joined.contains("Không lưu được 2 bài"), "{joined}");
        assert!(joined.contains("locked.mp3"), "{joined}");
        assert!(joined.contains("late.mp3") && joined.contains("thẻ đã ghi"), "{joined}");
        assert!(joined.contains("Chưa làm 3 bài"), "{joined}");
        assert!(joined.contains("bìa của cả mục"), "{joined}");
        assert!(bulk_has_problems(&report));
        let clean = describe_bulk(&BulkReport { done: vec![1], ..BulkReport::default() });
        assert!(clean.problems.is_empty());
        assert!(!bulk_has_problems(&BulkReport::default()));
        set_language(Language::Vi);
    }

    #[test]
    fn name_problem_checks_empty_stem_and_forbidden_chars() {
        assert_eq!(name_problem("  ", ".mp3", "a.mp3"), Some(FileNameError::Empty));
        assert_eq!(name_problem("a?b", ".mp3", "a.mp3"), Some(FileNameError::ForbiddenChar('?')));
        assert_eq!(name_problem("a/b", ".mp3", "a.mp3"), Some(FileNameError::NotSingleSegment));
        assert_eq!(name_problem("b", ".mp3", "a.mp3"), None);
    }

    // ---- Test view: thư viện và tệp trong thư mục tạm, FakeOutput, không chạm nhạc thật ----

    struct Env {
        dir: TempDir,
        library: Library,
        ops: MetadataOps,
    }

    fn env() -> Env {
        let dir = TempDir::new().unwrap();
        let config = LibraryConfig { target: dir.path().join("data").join("library.db"), legacy: None };
        let library = Library::open(config, Box::new(CoverStore::new(dir.path().join("covers")))).unwrap();
        let ops = MetadataOps::new(library.clone(), CoverStore::new(dir.path().join("covers"))).unwrap();
        Env { dir, library, ops }
    }

    impl Env {
        fn music(&self) -> PathBuf {
            let music = self.dir.path().join("music");
            fs::create_dir_all(&music).unwrap();
            music
        }

        fn mp3(&self, stem: &str, frames: &[(&str, &str)]) -> PathBuf {
            write_mp3_with_frames(&self.music(), stem, frames).unwrap()
        }

        fn index(&self, path: &Path) -> i64 {
            let row = TrackRow::from_scanned(path.to_owned(), tags::read_track(path).unwrap(), None);
            self.library.writer().transaction(move |conn| Ok(apply_scanned_track(conn, &row)?.id)).wait().unwrap()
        }

        fn folder_of(&self, id: i64) -> i64 {
            self.library
                .read(move |conn| {
                    Ok(conn.query_row("SELECT folder_id FROM songs WHERE id = ?1", [id], |row| row.get::<_, i64>(0))?)
                })
                .wait()
                .unwrap()
        }
    }

    struct Host {
        dialog: Option<Entity<MetadataDialog>>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().children(self.dialog.clone())
        }
    }

    type Events = Rc<RefCell<Vec<MetadataDialogEvent>>>;

    struct Rig {
        dialog: Entity<MetadataDialog>,
        events: Events,
    }

    fn start(cx: &mut TestAppContext, env: &Env) {
        set_language(Language::Vi);
        cx.executor().allow_parking();
        cx.update(|cx| {
            materiant::init(cx);
            let deps = PlayerDeps {
                library: Some(env.library.clone()),
                output: Box::new(FakeOutput::new(48_000, 2)),
                options: EngineOptions { stall_timeout: Duration::from_secs(3600), ..EngineOptions::default() },
                session_file: None,
            };
            player::install_for_test(deps, cx);
        });
    }

    fn open_dialog<'a>(
        cx: &'a mut TestAppContext,
        env: &Env,
        track: TrackRef,
        scope: Option<GroupScope>,
    ) -> (Rig, &'a mut VisualTestContext) {
        start(cx, env);
        let host = cx.new(|_| Host { dialog: None });
        let (_, cx) = cx.add_window_view({
            let host = host.clone();
            move |window, cx| Root::new(host, window, cx)
        });
        cx.update(|window, _| window.activate_window());
        let ops = env.ops.clone();
        let dialog = cx.update(|window, cx| MetadataDialog::open(ops, track, scope, window, cx));
        host.update(cx, |host, cx| {
            host.dialog = Some(dialog.clone());
            cx.notify();
        });
        let events: Events = Rc::default();
        let log = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(&dialog, move |_, event: &MetadataDialogEvent, _| log.borrow_mut().push(event.clone()))
                .detach();
        });
        wait_until(cx, "đọc thẻ xong", |cx| dialog.read_with(cx, |d, _| d.is_ready() || d.load_error().is_some()));
        (Rig { dialog, events }, cx)
    }

    fn frame(cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }

    /// Luồng thư viện chạy ngoài, đánh thức future từ luồng khác: lặp tới khi điều kiện đúng (trần ~5 s).
    fn wait_until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        for _ in 0..500 {
            cx.run_until_parked();
            if done(cx) {
                frame(cx);
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("quá hạn chờ: {what}");
    }

    fn saved_or_failed(rig: &Rig, cx: &mut VisualTestContext) {
        let dialog = rig.dialog.clone();
        wait_until(cx, "lưu xong", |cx| !dialog.read_with(cx, |d, _| d.is_saving()));
    }

    impl Rig {
        fn value(&self, field: Field, cx: &mut VisualTestContext) -> String {
            self.dialog.read_with(cx, |d, cx| d.form.as_ref().unwrap().value(field, cx))
        }

        fn set(&self, field: Field, value: &str, cx: &mut VisualTestContext) {
            self.dialog.update_in(cx, |d, window, cx| d.set_field(field, value, window, cx));
        }

        fn save(&self, cx: &mut VisualTestContext) {
            self.dialog.update_in(cx, |d, window, cx| d.save(window, cx));
            saved_or_failed(self, cx);
        }

        fn is_open(&self, cx: &mut VisualTestContext) -> bool {
            self.dialog.read_with(cx, |d, _| d.is_open())
        }
    }

    /// C1 — mở bài ngoài thư viện: đọc từ tệp, ô hiện đúng giá trị, tên tệp tách đuôi, đếm khung rác ở tiêu đề.
    #[::gpui::test]
    fn opens_from_the_file_and_shows_values_and_junk(cx: &mut TestAppContext) {
        let env = env();
        let path = env.mp3("Song", &[("TIT2", "Hello"), ("TPE1", "Me"), ("TALB", "Alb"), ("TSSE", "lavf")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path), None);
        assert_eq!(rig.value(Field::Title, cx), "Hello");
        assert_eq!(rig.value(Field::Artist, cx), "Me");
        assert_eq!(rig.value(Field::Album, cx), "Alb");
        rig.dialog.read_with(cx, |d, cx| {
            let form = d.form.as_ref().unwrap();
            assert_eq!(form.stem(cx), "Song");
            assert_eq!(form.extension, ".mp3");
            assert_eq!(junk_count(&d.view_data().unwrap().raw_frames), 1);
            assert!(d.title().contains('1'), "tiêu đề báo số khung rác: {}", d.title());
            assert!(d.grouping_warning(cx), "có album, thiếu nghệ sĩ album");
            assert!(!d.can_apply_to_group());
            assert_eq!(d.mode(), WriteMode::CleanKeepKnown, "chế độ ghi mặc định");
        });
    }

    /// C2 — sửa thẻ và đổi tên: ghi đúng vào tệp, phát `Saved`, đóng dialog, không để tệp tạm.
    #[::gpui::test]
    fn saves_tags_and_renames_the_file(cx: &mut TestAppContext) {
        let env = env();
        let path = env.mp3("Song", &[("TIT2", "Old")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path.clone()), None);
        rig.set(Field::Title, "New title", cx);
        rig.set(Field::Artist, "New artist", cx);
        rig.dialog.update_in(cx, |d, window, cx| d.type_filename("Renamed.MP3", window, cx));
        rig.dialog.read_with(cx, |d, cx| {
            assert_eq!(d.form.as_ref().unwrap().stem(cx), "Renamed", "đuôi gõ vào bị cắt");
            assert_eq!(d.name_error(), None);
        });
        rig.save(cx);
        let renamed = path.with_file_name("Renamed.mp3");
        assert!(renamed.exists() && !path.exists());
        let fields = tags::read_track(&renamed).unwrap().fields;
        assert_eq!(fields.title.as_deref(), Some("New title"));
        assert_eq!(fields.artist.as_deref(), Some("New artist"));
        assert!(matches!(rig.events.borrow().first(), Some(MetadataDialogEvent::Saved)));
        assert!(!rig.is_open(cx), "lưu xong thì đóng");
        wait_until(cx, "phát Closed", |_| {
            rig.events.borrow().iter().any(|e| matches!(e, MetadataDialogEvent::Closed))
        });
        let leftovers: Vec<_> = fs::read_dir(env.music())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| tags::is_temp_path(&e.path()))
            .collect();
        assert!(leftovers.is_empty(), "không sót bản tạm");
    }

    /// C3 — tên sai thì báo ngay khi gõ và không cho lưu.
    #[::gpui::test]
    fn rejects_a_bad_file_name_while_typing(cx: &mut TestAppContext) {
        let env = env();
        let path = env.mp3("Song", &[("TIT2", "T")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path.clone()), None);
        for bad in ["a/b", "x?y", "CON", ""] {
            rig.dialog.update_in(cx, |d, window, cx| d.type_filename(bad, window, cx));
            rig.dialog.read_with(cx, |d, _| {
                assert!(d.name_error().is_some(), "{bad:?} phải báo lỗi");
                assert!(!d.can_save());
            });
        }
        rig.dialog.update_in(cx, |d, window, cx| d.save(window, cx));
        assert!(path.exists(), "chưa động vào tệp");
        assert!(rig.events.borrow().is_empty());
        rig.dialog.update_in(cx, |d, window, cx| d.type_filename("Fine", window, cx));
        rig.dialog.read_with(cx, |d, _| assert!(d.can_save() && d.name_error().is_none()));
    }

    /// C4 — đổi sang tên đã có: hiện lỗi `TargetExists`, dialog ở lại, hai tệp nguyên vẹn.
    #[::gpui::test]
    fn keeps_the_dialog_open_when_the_target_exists(cx: &mut TestAppContext) {
        let env = env();
        let a = env.mp3("a", &[("TIT2", "A")]);
        let b = env.mp3("b", &[("TIT2", "B")]);
        let before = (fs::read(&a).unwrap(), fs::read(&b).unwrap());
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(a.clone()), None);
        rig.set(Field::Title, "Changed", cx);
        rig.dialog.update_in(cx, |d, window, cx| d.type_filename("b", window, cx));
        rig.save(cx);
        rig.dialog.read_with(cx, |d, _| {
            let error = d.error().expect("phải có lỗi");
            assert!(error.contains("b.mp3"), "{error}");
            assert!(d.is_open());
        });
        assert_eq!((fs::read(&a).unwrap(), fs::read(&b).unwrap()), before);
        assert!(rig.events.borrow().is_empty());
    }

    /// C5 — thẻ hỏng / tệp mất: dialog báo lỗi đọc, không cho lưu.
    #[::gpui::test]
    fn reports_a_load_error_and_blocks_saving(cx: &mut TestAppContext) {
        let env = env();
        let missing = env.music().join("gone.mp3");
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(missing), None);
        rig.dialog.read_with(cx, |d, _| {
            assert!(!d.is_ready() && !d.can_save());
            assert!(d.load_error().is_some_and(|text| !text.contains("metadata.")));
        });
        rig.dialog.update_in(cx, |d, window, cx| d.save(window, cx));
        assert!(rig.events.borrow().is_empty());
    }

    /// C8 — Fix grouping: nghệ sĩ album = album, bật tuyển tập, bật áp cho nhóm khi mở từ mục nhiều bài.
    #[::gpui::test]
    fn fix_grouping_fills_album_artist_and_turns_on_the_group(cx: &mut TestAppContext) {
        let env = env();
        let a = env.mp3("a", &[("TALB", "Abbey")]);
        let b = env.mp3("b", &[("TALB", "Abbey")]);
        let (ida, _idb) = (env.index(&a), env.index(&b));
        let folder = env.folder_of(ida);
        let scope = GroupScope { category: CategoryRef::Folder(folder), songs: 2 };
        let (rig, cx) = open_dialog(cx, &env, TrackRef::Library(ida), Some(scope));
        rig.dialog.read_with(cx, |d, cx| {
            assert!(d.grouping_warning(cx));
            assert!(d.can_apply_to_group() && !d.applies_to_group());
        });
        rig.dialog.update_in(cx, |d, window, cx| d.fix_grouping(window, cx));
        assert_eq!(rig.value(Field::AlbumArtist, cx), "Abbey");
        rig.dialog.read_with(cx, |d, cx| {
            assert!(!d.grouping_warning(cx));
            assert!(d.applies_to_group() && d.compilation);
        });
        rig.save(cx);
        for path in [&a, &b] {
            let fields = tags::read_track(path).unwrap().fields;
            assert_eq!(fields.album_artist.as_deref(), Some("Abbey"), "{}", path.display());
            assert_eq!(fields.compilation, Some(true));
        }
    }

    /// C9 — áp cho nhóm: chỉ trường đã đổi sang bài khác, tiêu đề riêng còn nguyên; ô áp chỉ hiện với mục > 1 bài.
    #[::gpui::test]
    fn applies_only_changed_fields_to_the_rest_of_the_group(cx: &mut TestAppContext) {
        let env = env();
        let a = env.mp3("a", &[("TIT2", "Title A"), ("TCON", "Rock")]);
        let b = env.mp3("b", &[("TIT2", "Title B"), ("TCON", "Jazz")]);
        let (ida, _) = (env.index(&a), env.index(&b));
        let folder = env.folder_of(ida);
        let single = GroupScope { category: CategoryRef::Folder(folder), songs: 1 };
        let (rig, cx) = open_dialog(cx, &env, TrackRef::Library(ida), Some(single));
        assert!(rig.dialog.read_with(cx, |d, _| !d.can_apply_to_group()), "mục một bài không có ô áp nhóm");
        rig.dialog.update_in(cx, |d, _, cx| d.set_apply_group(true, cx));
        assert!(rig.dialog.read_with(cx, |d, _| !d.applies_to_group()));
        let scope = GroupScope { category: CategoryRef::Folder(folder), songs: 2 };
        let (rig, cx) = open_dialog(cx, &env, TrackRef::Library(ida), Some(scope));
        assert_eq!(rig.value(Field::Genre, cx), "Rock", "ô ở tab chưa vẽ vẫn mang giá trị của tệp");
        rig.set(Field::Album, "Shared", cx);
        rig.dialog.update_in(cx, |d, _, cx| {
            d.set_apply_group(true, cx);
            d.set_mode(WriteMode::Clean, cx);
        });
        rig.save(cx);
        let (file_a, file_b) = (tags::read_track(&a).unwrap().fields, tags::read_track(&b).unwrap().fields);
        assert_eq!(file_a.album.as_deref(), Some("Shared"));
        assert_eq!(file_b.album.as_deref(), Some("Shared"), "album đã đổi nên áp sang bài kia");
        assert_eq!(file_b.genre.as_deref(), Some("Jazz"), "thể loại không đổi nên không ghi đè");
        assert_eq!(file_b.title.as_deref(), Some("Title B"), "tiêu đề riêng còn");
        assert!(!rig.is_open(cx));
    }

    fn set_readonly(path: &Path, readonly: bool) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(readonly);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// C10 — một bài trong nhóm lỗi: dialog ở lại, báo cáo nêu đúng tên bài đó, bài khác vẫn lưu.
    #[::gpui::test]
    fn bulk_failure_is_reported_by_name_and_keeps_the_dialog_open(cx: &mut TestAppContext) {
        let env = env();
        let a = env.mp3("a", &[("TIT2", "A")]);
        let b = env.mp3("locked", &[("TIT2", "B")]);
        let c = env.mp3("c", &[("TIT2", "C")]);
        let ida = env.index(&a);
        let _ = (env.index(&b), env.index(&c));
        let folder = env.folder_of(ida);
        set_readonly(&b, true);
        let scope = GroupScope { category: CategoryRef::Folder(folder), songs: 3 };
        let (rig, cx) = open_dialog(cx, &env, TrackRef::Library(ida), Some(scope));
        rig.set(Field::Genre, "Pop", cx);
        rig.dialog.update_in(cx, |d, _, cx| d.set_apply_group(true, cx));
        rig.save(cx);
        set_readonly(&b, false);
        rig.dialog.read_with(cx, |d, _| {
            let bulk = d.bulk_report().expect("có báo cáo lô");
            assert!(bulk.problems.iter().any(|line| line.contains("locked.mp3")), "{bulk:?}");
            assert!(bulk.headline.contains('2'), "hai bài đã lưu (a và c, mà a lưu riêng): {bulk:?}");
            assert!(d.is_open(), "còn lỗi thì không đóng");
        });
        assert_eq!(tags::read_track(&c).unwrap().fields.genre.as_deref(), Some("Pop"));
        assert_eq!(tags::read_track(&b).unwrap().fields.genre, None, "tệp lỗi nguyên vẹn");
        assert!(matches!(rig.events.borrow().first(), Some(MetadataDialogEvent::Saved)), "bài mở đã lưu");
    }

    /// C11 — bìa: ảnh hợp lệ thì lưu vào tệp; tệp không phải ảnh thì báo và giữ nguyên; bỏ bìa xoá bìa.
    #[::gpui::test]
    fn cover_is_set_rejected_and_removed(cx: &mut TestAppContext) {
        let env = env();
        let path = env.mp3("Song", &[("TIT2", "T")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path.clone()), None);
        let pane = rig.dialog.read_with(cx, |d, _| d.cover_pane().unwrap().clone());
        assert!(!pane.update(cx, |p, cx| p.set_image(b"not an image".to_vec(), cx)));
        pane.read_with(cx, |p, _| {
            assert_eq!(p.problem(), Some(super::super::cover_pane::CoverProblem::Unsupported));
            assert_eq!(p.edit(), CoverEdit::Keep);
        });
        assert!(pane.update(cx, |p, cx| p.set_image(TINY_PNG.to_vec(), cx)));
        pane.read_with(cx, |p, _| assert!(p.problem().is_none() && p.is_changed() && p.has_image()));
        rig.save(cx);
        assert_eq!(tags::read_track(&path).unwrap().cover.map(|c| c.data), Some(TINY_PNG.to_vec()));

        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path.clone()), None);
        let pane = rig.dialog.read_with(cx, |d, _| d.cover_pane().unwrap().clone());
        pane.read_with(cx, |p, _| assert!(p.has_image() && !p.is_changed(), "bìa gốc của tệp"));
        pane.update(cx, |p, cx| p.remove(cx));
        pane.update(cx, |p, cx| p.undo(cx));
        pane.read_with(cx, |p, _| assert_eq!(p.edit(), CoverEdit::Keep));
        pane.update(cx, |p, cx| p.remove(cx));
        rig.save(cx);
        assert!(tags::read_track(&path).unwrap().cover.is_none());
    }

    /// C12 — mở ở chế độ Merge giữ khung lạ, chế độ mặc định bỏ.
    #[::gpui::test]
    fn write_mode_decides_whether_junk_frames_survive(cx: &mut TestAppContext) {
        let env = env();
        let kept = env.mp3("kept", &[("TIT2", "T"), ("TSSE", "x")]);
        let cleaned = env.mp3("cleaned", &[("TIT2", "T"), ("TSSE", "x")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(kept.clone()), None);
        rig.dialog.update_in(cx, |d, _, cx| d.set_mode(WriteMode::Merge, cx));
        rig.save(cx);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(cleaned.clone()), None);
        rig.save(cx);
        let junk = |p: &Path| junk_count(&tags::read_raw_frames(p).unwrap());
        assert_eq!(junk(&kept), 1, "Merge giữ khung lạ");
        assert_eq!(junk(&cleaned), 0, "mặc định làm sạch");
    }

    /// C13 — bố cục: cả ba tab vẽ được, lưu hai lần liên tiếp bị chặn.
    #[::gpui::test]
    fn renders_every_tab_and_ignores_a_second_save_while_saving(cx: &mut TestAppContext) {
        let env = env();
        let path = env.mp3("Song", &[("TIT2", "T"), ("TSSE", "x")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path), None);
        for tab in [TAB_COMMON, TAB_DETAILS, TAB_RAW] {
            rig.dialog.update(cx, |d, cx| {
                d.tab = tab;
                cx.notify();
            });
            frame(cx);
        }
        rig.dialog.update_in(cx, |d, window, cx| {
            d.save(window, cx);
            assert!(d.is_saving());
            assert!(!d.can_save(), "đang lưu thì không lưu chồng");
            d.save(window, cx);
        });
        saved_or_failed(&rig, cx);
        assert_eq!(rig.events.borrow().iter().filter(|e| matches!(e, MetadataDialogEvent::Saved)).count(), 1);
    }

    /// C14 — dán: clipboard có ảnh thì ô bìa nhận, clipboard chỉ có chữ thì để ô nhập tự dán.
    #[::gpui::test]
    fn paste_takes_an_image_but_leaves_text_alone(cx: &mut TestAppContext) {
        let env = env();
        let path = env.mp3("Song", &[("TIT2", "T")]);
        let (rig, cx) = open_dialog(cx, &env, TrackRef::External(path), None);
        let pane = rig.dialog.read_with(cx, |d, _| d.cover_pane().unwrap().clone());
        cx.write_to_clipboard(::gpui::ClipboardItem::new_string("chỉ là chữ".into()));
        assert!(!pane.update(cx, |p, cx| p.paste(cx)), "chữ thì không nhận, Ctrl+V tiếp tục dán chữ");
        assert!(pane.read_with(cx, |p, _| !p.is_changed()));
        let image = ::gpui::Image::from_bytes(::gpui::ImageFormat::Png, TINY_PNG.to_vec());
        cx.write_to_clipboard(::gpui::ClipboardItem::from(image));
        assert!(pane.update(cx, |p, cx| p.paste(cx)));
        assert_eq!(pane.read_with(cx, |p, _| p.edit()), CoverEdit::Set(TINY_PNG.to_vec()));
    }
}
