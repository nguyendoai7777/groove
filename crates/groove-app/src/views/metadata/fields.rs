//! groove-gd1_19: các ô nhập của biểu mẫu metadata và phần logic thuần của nó (đặc tả 1.8).
//!
//! Quy ước trường của `tags`: `None` là không đụng, `Some("")` là xoá. Form gửi mọi trường (bản cũ `:546-548`), riêng
//! việc áp cho cả nhóm chỉ gửi trường người dùng đã đổi: gửi cả trường không đụng sẽ ghi đè giá trị riêng của từng
//! bài khác (mục 8 của task).

use groove_core::t;
use groove_library::metadata_ops::{FileNameError, GroupEdit, MetadataView};
use groove_library::tags::{CoverEdit, MetadataEdit, TagFields};
use materiant::field::{TextArea, TextField};
use materiant::gpui_kit::{App, AppContext, Entity, SharedString, Window};

/// Trường chữ một dòng của hai tab Common và Details (tên tệp, ghi chú, tuyển tập có ô riêng).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Artist,
    Album,
    AlbumArtist,
    TrackNo,
    TrackTotal,
    DiscNo,
    DiscTotal,
    Year,
    Genre,
    Composer,
    Publisher,
    Copyright,
    Bpm,
    Grouping,
}

impl Field {
    pub const ALL: [Field; 15] = [
        Field::Title,
        Field::Artist,
        Field::Album,
        Field::AlbumArtist,
        Field::TrackNo,
        Field::TrackTotal,
        Field::DiscNo,
        Field::DiscTotal,
        Field::Year,
        Field::Genre,
        Field::Composer,
        Field::Publisher,
        Field::Copyright,
        Field::Bpm,
        Field::Grouping,
    ];

    /// Tên ổn định, dùng làm id phần tử và khoá test.
    pub fn key(self) -> &'static str {
        match self {
            Field::Title => "title",
            Field::Artist => "artist",
            Field::Album => "album",
            Field::AlbumArtist => "album_artist",
            Field::TrackNo => "track_no",
            Field::TrackTotal => "track_total",
            Field::DiscNo => "disc_no",
            Field::DiscTotal => "disc_total",
            Field::Year => "year",
            Field::Genre => "genre",
            Field::Composer => "composer",
            Field::Publisher => "publisher",
            Field::Copyright => "copyright",
            Field::Bpm => "bpm",
            Field::Grouping => "grouping",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Field::Title => t!("metadata.field.title"),
            Field::Artist => t!("metadata.field.artist"),
            Field::Album => t!("metadata.field.album"),
            Field::AlbumArtist => t!("metadata.field.album_artist"),
            Field::TrackNo => t!("metadata.field.track_no"),
            Field::TrackTotal => t!("metadata.field.track_total"),
            Field::DiscNo => t!("metadata.field.disc_no"),
            Field::DiscTotal => t!("metadata.field.disc_total"),
            Field::Year => t!("metadata.field.year"),
            Field::Genre => t!("metadata.field.genre"),
            Field::Composer => t!("metadata.field.composer"),
            Field::Publisher => t!("metadata.field.publisher"),
            Field::Copyright => t!("metadata.field.copyright"),
            Field::Bpm => t!("metadata.field.bpm"),
            Field::Grouping => t!("metadata.field.grouping"),
        }
    }

    fn slot(self, fields: &mut TagFields) -> &mut Option<String> {
        match self {
            Field::Title => &mut fields.title,
            Field::Artist => &mut fields.artist,
            Field::Album => &mut fields.album,
            Field::AlbumArtist => &mut fields.album_artist,
            Field::TrackNo => &mut fields.track_no,
            Field::TrackTotal => &mut fields.track_total,
            Field::DiscNo => &mut fields.disc_no,
            Field::DiscTotal => &mut fields.disc_total,
            Field::Year => &mut fields.year,
            Field::Genre => &mut fields.genre,
            Field::Composer => &mut fields.composer,
            Field::Publisher => &mut fields.publisher,
            Field::Copyright => &mut fields.copyright,
            Field::Bpm => &mut fields.bpm,
            Field::Grouping => &mut fields.grouping,
        }
    }
}

/// Chữ trong trường, `None` coi như rỗng.
pub fn text_of(fields: &TagFields, field: Field) -> String {
    let mut copy = fields.clone();
    field.slot(&mut copy).take().unwrap_or_default()
}

/// Đuôi tệp quen thuộc: gõ vào ô tên thì tự cắt (bản cũ `:427-457`), đuôi thật nằm ở chip riêng.
const KNOWN_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "m4r", "aac", "opus", "wma", "aiff", "mp4"];

/// Tách `Song.mp3` thành (`Song`, `.mp3`). Không có đuôi, hoặc chỉ là tệp ẩn `.mp3`, thì đuôi rỗng.
pub fn split_file_name(file_name: &str) -> (String, String) {
    match file_name.rfind('.') {
        Some(at) if at > 0 => (file_name[..at].to_owned(), file_name[at..].to_owned()),
        _ => (file_name.to_owned(), String::new()),
    }
}

/// `Some(phần đã cắt)` khi `input` kết thúc bằng đuôi âm thanh quen thuộc, `None` khi không có gì để cắt.
pub fn strip_known_extension(input: &str) -> Option<String> {
    let (stem, ext) = split_file_name(input.trim_end());
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    (!stem.is_empty() && KNOWN_EXTENSIONS.contains(&ext.as_str())).then_some(stem)
}

/// Tên tệp mới ghép từ phần tên đang gõ và đuôi cũ; `None` khi không đổi so với tên hiện tại.
pub fn new_file_name(stem_input: &str, extension: &str, current: &str) -> Option<String> {
    let stem = stem_input.trim();
    let name = format!("{stem}{extension}");
    (name != current).then_some(name)
}

/// Câu báo lỗi tên tệp theo biến thể.
pub fn name_error_text(error: &FileNameError) -> String {
    match error {
        FileNameError::Empty => t!("metadata.name_error.empty").to_owned(),
        FileNameError::NotSingleSegment => t!("metadata.name_error.not_single_segment").to_owned(),
        FileNameError::ForbiddenChar(ch) => t!("metadata.name_error.forbidden_char", ch = ch),
        FileNameError::Reserved => t!("metadata.name_error.reserved").to_owned(),
        FileNameError::TrailingDot => t!("metadata.name_error.trailing_dot").to_owned(),
        FileNameError::TooLong => t!("metadata.name_error.too_long").to_owned(),
        FileNameError::ExtensionChanged => t!("metadata.name_error.extension_changed").to_owned(),
    }
}

fn blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|text| text.trim().is_empty())
}

/// Album có mà nghệ sĩ album trống: bài dễ bị tách khỏi album ở trình phát khác (đặc tả 1.8, `:365-381`).
pub fn needs_grouping_fix(fields: &TagFields) -> bool {
    !blank(&fields.album) && blank(&fields.album_artist)
}

/// Fix grouping: nghệ sĩ album = album, bật tuyển tập.
pub fn apply_grouping_fix(fields: &mut TagFields) {
    fields.album_artist = fields.album.clone();
    fields.compilation = Some(true);
}

/// Phần sửa của một bài: mọi trường của form (kể cả rỗng, để xoá). Lyrics, timeline để `None`: dialog này không sửa.
pub fn edit_for_track(form: &TagFields, cover: CoverEdit) -> MetadataEdit {
    MetadataEdit {
        fields: TagFields {
            lyrics: None,
            timeline: None,
            compilation: Some(form.compilation.unwrap_or(false)),
            ..form.clone()
        },
        cover,
    }
}

fn changed(original: &Option<String>, current: &Option<String>) -> Option<String> {
    let before = original.as_deref().unwrap_or("").trim();
    let after = current.as_deref().unwrap_or("").trim();
    (before != after).then(|| after.to_owned())
}

/// Phần áp cho cả nhóm: chỉ trường đã đổi so với lúc mở, nên không ghi đè giá trị riêng của bài khác khi người dùng
/// không đụng tới.
pub fn group_edit(original: &TagFields, form: &TagFields, cover: CoverEdit) -> GroupEdit {
    let compilation = {
        let (before, after) = (original.compilation.unwrap_or(false), form.compilation.unwrap_or(false));
        (before != after).then_some(after)
    };
    GroupEdit {
        album: changed(&original.album, &form.album),
        album_artist: changed(&original.album_artist, &form.album_artist),
        genre: changed(&original.genre, &form.genre),
        year: changed(&original.year, &form.year),
        compilation,
        cover,
    }
}

/// Các ô nhập đang sống của một lần mở dialog.
pub struct Form {
    pub filename: Entity<TextField>,
    pub extension: String,
    pub current_name: String,
    pub fields: Vec<(Field, Entity<TextField>)>,
    pub comment: Entity<TextArea>,
}

impl Form {
    pub fn new(view: &MetadataView, window: &mut Window, cx: &mut App) -> Form {
        let (stem, extension) = split_file_name(&view.filename);
        let filename = cx.new(|cx| {
            let mut field = TextField::new("metadata-filename", window, cx).label(t!("metadata.field.filename"));
            field.set_value(stem, window, cx);
            field
        });
        let fields = Field::ALL
            .iter()
            .map(|&field| {
                let value = text_of(&view.fields, field);
                let id = SharedString::from(format!("metadata-{}", field.key()));
                let entity = cx.new(|cx| {
                    let mut entity = TextField::new(id, window, cx).label(field.label());
                    entity.set_value(value, window, cx);
                    entity
                });
                (field, entity)
            })
            .collect();
        let comment_text = view.fields.comment.clone().unwrap_or_default();
        let comment = cx.new(|cx| {
            let mut area = TextArea::new("metadata-comment", window, cx).label(t!("metadata.field.comment")).rows(3);
            area.set_value(comment_text, window, cx);
            area
        });
        Form { filename, extension, current_name: view.filename.clone(), fields, comment }
    }

    pub fn field(&self, field: Field) -> &Entity<TextField> {
        &self.fields.iter().find(|(f, _)| *f == field).expect("every field has an entity").1
    }

    #[cfg(test)]
    pub fn value(&self, field: Field, cx: &App) -> String {
        self.field(field).read(cx).value(cx).to_string()
    }

    pub fn set_value(&self, field: Field, value: &str, window: &mut Window, cx: &mut App) {
        self.field(field).update(cx, |entity, cx| entity.set_value(value.to_owned(), window, cx));
    }

    /// Mọi trường theo chữ đang gõ (rỗng là `Some("")`), kèm tuyển tập do dialog truyền.
    pub fn collect(&self, compilation: bool, cx: &App) -> TagFields {
        let mut out = TagFields { compilation: Some(compilation), ..TagFields::default() };
        for (field, entity) in &self.fields {
            *field.slot(&mut out) = Some(entity.read(cx).value(cx).to_string());
        }
        out.comment = Some(self.comment.read(cx).value(cx).to_string());
        out
    }

    /// Phần tên đang gõ (chưa kèm đuôi).
    pub fn stem(&self, cx: &App) -> String {
        self.filename.read(cx).value(cx).to_string()
    }

    pub fn new_file_name(&self, cx: &App) -> Option<String> {
        new_file_name(&self.stem(cx), &self.extension, &self.current_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_stem_and_extension() {
        assert_eq!(split_file_name("Song.mp3"), ("Song".into(), ".mp3".into()));
        assert_eq!(split_file_name("a.b.flac"), ("a.b".into(), ".flac".into()));
        assert_eq!(split_file_name("noext"), ("noext".into(), String::new()));
        assert_eq!(split_file_name(".mp3"), (".mp3".into(), String::new()), "tệp ẩn không có đuôi");
    }

    #[test]
    fn typed_audio_extension_is_cut() {
        assert_eq!(strip_known_extension("Song.MP3").as_deref(), Some("Song"));
        assert_eq!(strip_known_extension("Song.flac").as_deref(), Some("Song"));
        assert_eq!(strip_known_extension("Song. mix"), None, "đuôi lạ giữ nguyên");
        assert_eq!(strip_known_extension("v1.2"), None);
        assert_eq!(strip_known_extension(".mp3"), None, "không cắt thành tên rỗng");
    }

    #[test]
    fn new_name_is_none_when_unchanged() {
        assert_eq!(new_file_name("Song", ".mp3", "Song.mp3"), None);
        assert_eq!(new_file_name(" Song ", ".mp3", "Song.mp3"), None, "khoảng trắng hai đầu bị cắt");
        assert_eq!(new_file_name("Other", ".mp3", "Song.mp3").as_deref(), Some("Other.mp3"));
    }

    #[test]
    fn grouping_warning_and_fix() {
        let mut fields = TagFields { album: Some("Abbey".into()), ..TagFields::default() };
        assert!(needs_grouping_fix(&fields));
        apply_grouping_fix(&mut fields);
        assert_eq!(fields.album_artist.as_deref(), Some("Abbey"));
        assert_eq!(fields.compilation, Some(true));
        assert!(!needs_grouping_fix(&fields));
        assert!(!needs_grouping_fix(&TagFields::default()), "không album thì không cảnh báo");
        let spaces = TagFields { album: Some("A".into()), album_artist: Some("  ".into()), ..TagFields::default() };
        assert!(needs_grouping_fix(&spaces));
    }

    #[test]
    fn track_edit_sends_every_field_and_never_touches_lyrics() {
        let form = TagFields {
            title: Some(String::new()),
            artist: Some("A".into()),
            lyrics: Some("không được gửi".into()),
            ..TagFields::default()
        };
        let edit = edit_for_track(&form, CoverEdit::Keep);
        assert_eq!(edit.fields.title.as_deref(), Some(""), "rỗng là xoá");
        assert_eq!(edit.fields.lyrics, None);
        assert_eq!(edit.fields.compilation, Some(false));
    }

    #[test]
    fn group_edit_only_carries_changed_fields() {
        let original = TagFields {
            album: Some("Abbey".into()),
            genre: Some("Rock".into()),
            year: Some("1969".into()),
            ..TagFields::default()
        };
        let mut form = original.clone();
        assert_eq!(group_edit(&original, &form, CoverEdit::Keep), GroupEdit::default());
        form.genre = Some("Pop".into());
        form.album_artist = Some("The Beatles".into());
        form.compilation = Some(true);
        let edit = group_edit(&original, &form, CoverEdit::Remove);
        assert_eq!(edit.genre.as_deref(), Some("Pop"));
        assert_eq!(edit.album_artist.as_deref(), Some("The Beatles"));
        assert_eq!(edit.compilation, Some(true));
        assert_eq!(edit.album, None, "album không đổi thì không gửi");
        assert_eq!(edit.year, None);
        assert_eq!(edit.cover, CoverEdit::Remove);
    }

    #[test]
    fn clearing_a_field_is_a_change_in_the_group_edit() {
        let original = TagFields { genre: Some("Rock".into()), ..TagFields::default() };
        let form = TagFields { genre: Some(String::new()), ..TagFields::default() };
        assert_eq!(group_edit(&original, &form, CoverEdit::Keep).genre.as_deref(), Some(""));
    }

    #[test]
    fn every_field_has_label_and_distinct_key() {
        let mut keys: Vec<_> = Field::ALL.iter().map(|f| f.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), Field::ALL.len());
        for field in Field::ALL {
            assert_ne!(field.label(), "", "{field:?}");
            assert!(!field.label().starts_with("metadata."), "thiếu khoá i18n của {field:?}");
        }
    }

    #[test]
    fn name_errors_have_text() {
        use groove_core::i18n::{Language, set_language};
        for language in [Language::Vi] {
            set_language(language);
            for error in [
                FileNameError::Empty,
                FileNameError::NotSingleSegment,
                FileNameError::ForbiddenChar('?'),
                FileNameError::Reserved,
                FileNameError::TrailingDot,
                FileNameError::TooLong,
                FileNameError::ExtensionChanged,
            ] {
                let text = name_error_text(&error);
                assert!(!text.contains("metadata."), "thiếu khoá cho {error:?}: {text}");
                assert!(!text.contains('{'), "chưa thay biến: {text}");
            }
        }
    }
}
