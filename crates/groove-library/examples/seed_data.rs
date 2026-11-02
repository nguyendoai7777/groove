//! ui-audit: dựng thư mục dữ liệu tạm cho chạy thử (`tools/measure-hidden.ps1 -Seed`): một tệp MP3 mẫu tên rất dài
//! (`fixtures`) làm bài đang mở trong `session.json`, nên thanh phát không bị khoá và tên chạy ngang (Marquee). Phiên khôi
//! phục chỉ nạp bài, không phát: không ra tiếng.
//!
//! cpu (03/11/2026): thêm `<số bài>` thì sinh thêm chừng ấy tệp mẫu (nhiều album, nghệ sĩ) và import cả thư mục vào
//! `library.db` của thư mục tạm, để đo với thư viện có bài; bài tên dài khi đó là bài trong thư viện.
//!
//!   cargo run -p groove-library --example seed_data -- <thư mục dữ liệu> [số bài]

use std::path::PathBuf;

use groove_library::covers::CoverStore;
use groove_library::fixtures::write_mp3_with_frames;
use groove_library::indexer::Indexer;
use groove_library::store::{Library, LibraryConfig};

const LONG_TITLE: &str = "A very long song title that never fits in the player bar so the marquee keeps scrolling";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("usage: seed_data <data dir> [songs]"));
    let songs: usize = args.next().map(|n| n.parse()).transpose()?.unwrap_or(0);
    let music = data.join("music");
    std::fs::create_dir_all(&music)?;
    let song = write_mp3_with_frames(&music, "long-title", &[("TIT2", LONG_TITLE), ("TPE1", "Fixture Artist")])?;
    let mut current = serde_json::json!({ "external": song });

    if songs > 0 {
        for index in 0..songs {
            let album = index / 12;
            let dir = music.join(format!("album-{album:03}"));
            std::fs::create_dir_all(&dir)?;
            let title = format!("Seeded song number {index} with a reasonably long title");
            let (artist, album_name) = (format!("Seeded Artist {}", album % 25), format!("Seeded Album {album}"));
            let track = format!("{}", index % 12 + 1);
            write_mp3_with_frames(
                &dir,
                &format!("song-{index:05}"),
                &[("TIT2", &title), ("TPE1", &artist), ("TALB", &album_name), ("TRCK", &track)],
            )?;
        }
        let config = LibraryConfig { target: data.join("library.db"), legacy: None };
        let sink = CoverStore::new(data.join("cache").join("covers"));
        let library = Library::open(config, Box::new(sink.clone()))?;
        let report = Indexer::new(library.clone(), sink).import(&music).wait()?;
        println!("imported {report:?}");
        let id = library.track_for_path(song.clone()).wait()?;
        current = serde_json::to_value(id)?;
        futures::executor::block_on(library.shutdown());
    }

    let session = serde_json::json!({ "current": current, "positionSec": 0.0 });
    std::fs::write(data.join("session.json"), serde_json::to_vec_pretty(&session)?)?;
    println!("seeded {} (+{songs} songs)", song.display());
    Ok(())
}
