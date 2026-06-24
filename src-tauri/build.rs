use std::fs;
use std::path::Path;

fn create_ico<F>(draw_fn: F) -> Vec<u8>
where
    F: Fn(u32, u32) -> (u8, u8, u8, u8),
{
    let xor_size = 32 * 32 * 4;
    let and_size = 32 * 32 / 8; // 128 bytes
    let dib_header_size = 40;
    let img_data_size = dib_header_size + xor_size + and_size;
    let ico_file_size = 6 + 16 + img_data_size;
    
    let mut buffer = vec![0u8; ico_file_size];
    
    // 1. Icon Header
    buffer[0..2].copy_from_slice(&0u16.to_le_bytes()); // Reserved
    buffer[2..4].copy_from_slice(&1u16.to_le_bytes()); // Type: Icon
    buffer[4..6].copy_from_slice(&1u16.to_le_bytes()); // Count: 1
    
    // 2. Directory Entry
    buffer[6] = 32; // Width
    buffer[7] = 32; // Height
    buffer[8] = 0;  // Colors
    buffer[9] = 0;  // Reserved
    buffer[10..12].copy_from_slice(&1u16.to_le_bytes()); // Planes
    buffer[12..14].copy_from_slice(&32u16.to_le_bytes()); // BPP
    buffer[14..18].copy_from_slice(&(img_data_size as u32).to_le_bytes()); // Size
    buffer[18..22].copy_from_slice(&22u32.to_le_bytes()); // Offset
    
    // 3. DIB Header
    buffer[22..26].copy_from_slice(&(dib_header_size as u32).to_le_bytes());
    buffer[26..30].copy_from_slice(&32i32.to_le_bytes()); // Width
    buffer[30..34].copy_from_slice(&64i32.to_le_bytes()); // Height (doubled!)
    buffer[34..36].copy_from_slice(&1u16.to_le_bytes()); // Planes
    buffer[36..38].copy_from_slice(&32u16.to_le_bytes()); // BPP
    buffer[38..42].copy_from_slice(&0u32.to_le_bytes()); // Compression (BI_RGB)
    buffer[42..46].copy_from_slice(&((xor_size + and_size) as u32).to_le_bytes()); // Image size
    buffer[46..50].copy_from_slice(&0i32.to_le_bytes());
    buffer[50..54].copy_from_slice(&0i32.to_le_bytes());
    buffer[54..58].copy_from_slice(&0u32.to_le_bytes());
    buffer[58..62].copy_from_slice(&0u32.to_le_bytes());
    
    // 4. XOR Mask (pixels, bottom-up BGRA)
    let xor_offset = 22 + dib_header_size;
    for y in 0..32 {
        for x in 0..32 {
            let (r, g, b, a) = draw_fn(x, y);
            let bmp_y = 31 - y; // flip Y for bottom-up BMP
            let idx = xor_offset + ((bmp_y * 32 + x) * 4) as usize;
            buffer[idx] = b;
            buffer[idx + 1] = g;
            buffer[idx + 2] = r;
            buffer[idx + 3] = a;
        }
    }
    
    buffer
}

fn write_if_changed(path: &Path, content: &[u8]) {
    if path.exists() {
        if let Ok(existing) = fs::read(path) {
            if existing == content {
                return; // Content is unchanged; do not write to avoid triggering a rebuild loop!
            }
        }
    }
    fs::write(path, content).unwrap();
}

fn main() {
    // Generate taskbar icons
    let output_dir = Path::new("icons/taskbar");
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).unwrap();
    }
    
    let prev_draw = |x: u32, y: u32| {
        if x >= 6 && x <= 10 && y >= 6 && y <= 26 {
            (255, 255, 255, 255)
        } else if x >= 10 && x <= 26 && (y as i32 - 16).abs() <= ((x as i32 - 10) * 10 / 16) {
            (255, 255, 255, 255)
        } else {
            (0, 0, 0, 0)
        }
    };
    
    let next_draw = |x: u32, y: u32| {
        if x >= 22 && x <= 26 && y >= 6 && y <= 26 {
            (255, 255, 255, 255)
        } else if x >= 6 && x <= 22 && (y as i32 - 16).abs() <= ((22 - x as i32) * 10 / 16) {
            (255, 255, 255, 255)
        } else {
            (0, 0, 0, 0)
        }
    };
    
    let play_draw = |x: u32, y: u32| {
        if x >= 8 && x <= 26 && (y as i32 - 16).abs() <= ((26 - x as i32) * 10 / 18) {
            (255, 255, 255, 255)
        } else {
            (0, 0, 0, 0)
        }
    };
    
    let pause_draw = |x: u32, y: u32| {
        if x >= 8 && x <= 13 && y >= 6 && y <= 26 {
            (255, 255, 255, 255)
        } else if x >= 19 && x <= 24 && y >= 6 && y <= 26 {
            (255, 255, 255, 255)
        } else {
            (0, 0, 0, 0)
        }
    };
    
    write_if_changed(&output_dir.join("prev.ico"), &create_ico(prev_draw));
    write_if_changed(&output_dir.join("next.ico"), &create_ico(next_draw));
    write_if_changed(&output_dir.join("play.ico"), &create_ico(play_draw));
    write_if_changed(&output_dir.join("pause.ico"), &create_ico(pause_draw));
    
    println!("cargo:warning=Generated taskbar icons in src-tauri/icons/taskbar");
    
    tauri_build::build();
}
