//! G-6: mọi chữ của groove độ đậm normal. Quét mã nguồn `src/**` không còn gọi độ đậm khác `NORMAL`
//! (`FontWeight::BOLD`, `.font_semibold()`…). Component materiant mặc định đậm thì groove truyền `FontWeight::NORMAL`.

use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "FontWeight::THIN",
    "FontWeight::EXTRA_LIGHT",
    "FontWeight::LIGHT",
    "FontWeight::MEDIUM",
    "FontWeight::SEMIBOLD",
    "FontWeight::BOLD",
    "FontWeight::EXTRA_BOLD",
    "FontWeight::BLACK",
    "FontWeight(",
    ".font_thin(",
    ".font_extralight(",
    ".font_light(",
    ".font_medium(",
    ".font_semibold(",
    ".font_bold(",
    ".font_extrabold(",
    ".font_black(",
];

fn scan(dir: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).unwrap();
            for (ix, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                if let Some(hit) = FORBIDDEN.iter().find(|f| code.contains(*f)) {
                    found.push(format!("{}:{}: {hit}", path.display(), ix + 1));
                }
            }
        }
    }
}

#[test]
fn no_font_weight_other_than_normal() {
    let mut found = Vec::new();
    scan(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
    assert!(found.is_empty(), "G-6: chỉ độ đậm normal, còn:\n{}", found.join("\n"));
}
