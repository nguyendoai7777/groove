//! groove-gd1_10: công cụ kiểm tay, không nằm trong bản phát hành.
//!
//!   cargo run -p groove-audio --example audio_check -- play <tệp>      # PHÁT RA LOA. Gõ: p, s <giây>, v <0-1>, q
//!   cargo run -p groove-audio --example audio_check -- verify <tệp>…   # giải mã hết, kiểm tua; không ra loa
//!   cargo run -p groove-audio --example audio_check -- scan <danh-sách> # mỗi dòng một đường dẫn; chỉ đọc, chỉ in số đếm

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufRead;
use std::path::Path;

use groove_audio::decoder::{AudioErrorKind, TrackDecoder};
use groove_audio::engine::{AudioCmd, AudioEngine, EngineOptions};
use groove_audio::output::default_output;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("play") if args.len() == 2 => play(&args[1]),
        Some("verify") => args[1..].iter().for_each(|path| verify(path)),
        Some("scan") if args.len() == 2 => scan(&args[1]),
        _ => eprintln!("usage: play <file> | verify <file>... | scan <list-file>"),
    }
}

fn play(path: &str) {
    let (engine, mut events) = AudioEngine::spawn(default_output(), EngineOptions::default()).expect("spawn");
    let generation = engine.next_generation();
    engine.send(AudioCmd::Load { generation, path: path.into(), start_sec: 0.0, autoplay: true });
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let mut playing = true;
    loop {
        while let Some(event) = events.try_recv() {
            println!("{event:?}");
        }
        if let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(500)) {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next().and_then(|v| v.parse::<f64>().ok())) {
                (Some("q"), _) => return,
                (Some("p"), _) => {
                    playing = !playing;
                    engine.send(if playing { AudioCmd::Play } else { AudioCmd::Pause });
                }
                (Some("s"), Some(sec)) => engine.send(AudioCmd::Seek { generation, sec }),
                (Some("v"), Some(volume)) => engine.send(AudioCmd::SetVolume(volume as f32)),
                _ => {}
            }
        }
        println!("position {:.2}s playing={}", engine.position_sec(), engine.is_playing());
    }
}

fn decode_all(decoder: &mut TrackDecoder) -> Result<(Vec<f32>, u32, usize), AudioErrorKind> {
    let (mut all, mut chunk, mut rate, mut channels) = (Vec::new(), Vec::new(), 0, 1);
    while let Some(spec) = decoder.next_frames(&mut chunk)? {
        (rate, channels) = (spec.sample_rate, spec.channels);
        all.extend_from_slice(&chunk);
    }
    Ok((all, rate, channels))
}

/// Giải mã hết, rồi tua tới 40% và so 4096 frame với bản giải mã liền mạch; khớp thì RMS sai số gần 0.
fn verify(path: &str) {
    let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let result = (|| {
        let mut decoder = TrackDecoder::open(Path::new(path))?;
        let declared = decoder.duration_sec();
        let (all, rate, channels) = decode_all(&mut decoder)?;
        let frames = all.len() / channels.max(1);
        let target = 0.4 * frames as f64 / f64::from(rate.max(1));
        decoder.seek(target)?;
        let mut chunk = Vec::new();
        let mut after = Vec::new();
        while after.len() < 4096 * channels {
            match decoder.next_frames(&mut chunk)? {
                Some(_) => after.extend_from_slice(&chunk),
                None => break,
            }
        }
        let start = (target * f64::from(rate)).round() as usize * channels;
        let compare = after.len().min(4096 * channels);
        let error: f32 = after[..compare].iter().zip(&all[start..]).map(|(a, b)| (a - b) * (a - b)).sum();
        let rms = (error / compare.max(1) as f32).sqrt();
        // Lệch thật tính bằng frame: khớp tốt nhất trong phần sau frame 2048 (bỏ đoạn codec còn nạp trạng thái).
        let probe = &after[(2048 * channels).min(compare)..compare];
        let offset = (-1500i64..=1500)
            .map(|d| {
                let at = start as i64 + (2048 + d) * channels as i64;
                let error: f32 = all[at.max(0) as usize..].iter().zip(probe).map(|(a, b)| (a - b) * (a - b)).sum();
                ((error * 1e6) as u64, d)
            })
            .min()
            .map_or(0, |best| best.1);
        Ok::<_, AudioErrorKind>(format!(
            "ok rate={rate} ch={channels} declared={declared:.3}s decoded={:.3}s seek_rms={rms:.6} seek_offset_frames={offset}",
            frames as f64 / f64::from(rate.max(1))
        ))
    })();
    println!("{name}: {}", result.unwrap_or_else(|kind| format!("ERROR {kind:?}")));
}

/// Loại AAC theo AudioSpecificConfig. `implicit-sbr?`: khai LC nhưng tần số thấp, có thể là HE-AAC báo ngầm.
fn classify(path: &Path) -> String {
    let Ok(file) = File::open(path) else { return "unreadable".into() };
    let source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(&extension.to_ascii_lowercase());
    }
    let probe =
        symphonia::default::get_probe().probe(&hint, source, FormatOptions::default(), MetadataOptions::default());
    let Ok(format) = probe else { return "probe-failed".into() };
    let Some(params) =
        format.default_track(TrackType::Audio).and_then(|t| t.codec_params.as_ref()).and_then(|p| p.audio())
    else {
        return "no-audio-track".into();
    };
    let codec = format!("{}", params.codec);
    match params.extra_data.as_deref() {
        Some([first, ..])
            if format!("{:?}", params.codec)
                == format!("{:?}", symphonia::core::codecs::audio::well_known::CODEC_ID_AAC) =>
        {
            match first >> 3 {
                5 | 29 => "aac-he-explicit".into(),
                2 if params.sample_rate.is_some_and(|rate| rate <= 24_000) => "aac-lc-implicit-sbr?".into(),
                2 => "aac-lc".into(),
                other => format!("aac-aot{other}"),
            }
        }
        _ => codec,
    }
}

fn scan(list: &str) {
    let file = File::open(list).expect("list file");
    let mut counts: BTreeMap<(String, String, String), u32> = BTreeMap::new();
    let mut total = 0;
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let path = Path::new(line.trim());
        if line.trim().is_empty() {
            continue;
        }
        total += 1;
        let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let codec = classify(path);
        let opens = match TrackDecoder::open(path).and_then(|mut decoder| {
            let mut chunk = Vec::new();
            decoder.next_frames(&mut chunk).map(|_| ())
        }) {
            Ok(()) => "ok".to_string(),
            Err(kind) => format!("{kind:?}"),
        };
        *counts.entry((extension, codec, opens)).or_default() += 1;
    }
    println!("total {total}");
    for ((extension, codec, opens), count) in counts {
        println!("{count:>6}  .{extension:<5} {codec:<24} {opens}");
    }
}
