//! ui-audit: dựng thư mục dữ liệu tạm cho chạy thử (`tools/measure-hidden.ps1 -Seed`): một tệp MP3 mẫu tên rất dài
//! (`fixtures`) làm bài đang mở trong `session.json`, nên thanh phát không bị khoá và tên chạy ngang (Marquee). Phiên khôi
//! phục chỉ nạp bài, không phát: không ra tiếng.
//!
//!   cargo run -p groove-library --example seed_data -- <thư mục dữ liệu>

use std::path::PathBuf;

use groove_library::fixtures::write_mp3_with_frames;

const LONG_TITLE: &str = "A very long song title that never fits in the player bar so the marquee keeps scrolling";

fn main() -> std::io::Result<()> {
    let data = PathBuf::from(std::env::args().nth(1).expect("usage: seed_data <data dir>"));
    let music = data.join("music");
    std::fs::create_dir_all(&music)?;
    let song = write_mp3_with_frames(&music, "long-title", &[("TIT2", LONG_TITLE), ("TPE1", "Fixture Artist")])?;
    let session = serde_json::json!({ "current": { "external": song }, "positionSec": 0.0 });
    std::fs::write(data.join("session.json"), serde_json::to_vec_pretty(&session)?)?;
    println!("seeded {}", song.display());
    Ok(())
}
