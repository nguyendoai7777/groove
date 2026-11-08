//! groove-gd1_17: menu chuột phải của bài (đặc tả 1.2): thêm vào danh sách phát, sửa timeline / lyrics / metadata; trong
//! playlist có thêm "Xoá khỏi danh sách phát", chữ đỏ. groove-gd2_03 thêm "Save as new" (cắt ra tệp mới).

use groove_core::model::{CategoryRef, Song, TrackRef};
use groove_core::t;
use materiant::gpui_kit::App;
use materiant::menu::{Menu, MenuItem};

use crate::library_ctl::{self, SongEditKind};

/// Mục "Save as new": hộp cắt / xuất của `LibraryCtl` (groove-gd2_03).
pub fn save_as(ctl: &materiant::gpui_kit::Entity<library_ctl::LibraryCtl>, track: TrackRef) -> MenuItem {
    let ctl = ctl.clone();
    MenuItem::new("song-menu-save-as", t!("save_as.menu")).on_select(move |_, cx| {
        ctl.update(cx, |ctl, cx| ctl.open_save_as(track.clone(), cx));
    })
}

/// Bài `song` trong danh sách `reference`. Chưa có `LibraryCtl` thì menu rỗng.
pub fn build(menu: Menu, reference: CategoryRef, song: &Song, cx: &App) -> Menu {
    let Some(ctl) = library_ctl::ctl(cx) else { return menu };
    let song_id = match song.track {
        TrackRef::Library(id) => id,
        TrackRef::External(_) => return menu,
    };

    let mut add = Menu::new("song-menu-playlists");
    for playlist in ctl.read(cx).playlists().iter() {
        let CategoryRef::Playlist(playlist_id) = playlist.reference else { continue };
        let ctl = ctl.clone();
        add = add.item(MenuItem::new(("song-menu-playlist", playlist_id as usize), playlist.name.clone()).on_select(
            move |_, cx| {
                ctl.update(cx, |ctl, cx| ctl.add_to_playlist(playlist_id, song_id, cx));
            },
        ));
    }
    let create = ctl.clone();
    add = add.separator().item(MenuItem::new("song-menu-new-playlist", t!("actions.song.new_playlist")).on_select(
        move |_, cx| {
            create.update(cx, |ctl, cx| ctl.open_create_playlist(Some(song_id), cx));
        },
    ));

    let edit = |id: &'static str, label: &'static str, kind: SongEditKind| {
        let (ctl, track) = (ctl.clone(), song.track.clone());
        MenuItem::new(id, label).on_select(move |_, cx| {
            ctl.update(cx, |ctl, cx| ctl.open_text_dialog(kind, track.clone(), cx));
        })
    };
    let metadata = {
        let (ctl, track) = (ctl.clone(), song.track.clone());
        MenuItem::new("song-menu-metadata", t!("actions.song.edit_metadata")).on_select(move |_, cx| {
            ctl.update(cx, |ctl, cx| ctl.open_metadata(track.clone(), reference, cx));
        })
    };
    let menu = menu
        .submenu(MenuItem::new("song-menu-add", t!("actions.song.add_to_playlist")), add)
        .separator()
        .item(edit("song-menu-timeline", t!("actions.song.edit_timeline"), SongEditKind::Timeline))
        .item(edit("song-menu-lyrics", t!("actions.song.edit_lyrics"), SongEditKind::Lyrics))
        .item(metadata)
        .item(save_as(&ctl, song.track.clone()));

    if let CategoryRef::Playlist(playlist_id) = reference {
        let ctl = ctl.clone();
        menu.separator().item(
            MenuItem::new("song-menu-remove", t!("actions.song.remove_from_playlist")).danger(true).on_select(
                move |_, cx| {
                    ctl.update(cx, |ctl, cx| ctl.remove_from_playlist(playlist_id, song_id, cx));
                },
            ),
        )
    } else {
        menu
    }
}
