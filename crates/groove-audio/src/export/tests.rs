//! groove-gd2_02: test xuất "Save as new". Tệp mẫu tự dựng (`groove_library::fixtures`, sóng sin tự ghi) trong `TestDir`,
//! không đụng nhạc thật, không phát ra loa.

use std::path::{Path, PathBuf};

use groove_core::test_dir::TestDir;
use groove_library::fixtures::{self, AudioKind};

use super::mp3_copy::parse_header;
use super::pcm::wav_header;
use super::*;
use crate::decoder::{SourceSpec, TrackDecoder};

/// WAV 16 bit có sóng sin 440 Hz, `secs` giây.
fn sine_wav(dir: &Path, name: &str, rate: u32, channels: usize, secs: f64) -> PathBuf {
    let frames = (secs * f64::from(rate)) as usize;
    let mut data = Vec::with_capacity(frames * channels * 2);
    for i in 0..frames {
        let v = (i as f64 / f64::from(rate) * 440.0 * std::f64::consts::TAU).sin() as f32 * 0.5;
        for _ in 0..channels {
            data.extend_from_slice(&(quantize(v, 16) as i16).to_le_bytes());
        }
    }
    let spec = SourceSpec { sample_rate: rate, channels };
    let mut bytes = wav_header(spec, 16, data.len() as u32).to_vec();
    bytes.extend(data);
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// (số frame giải mã được, tần số).
fn decoded_frames(path: &Path) -> (u64, u32) {
    let mut decoder = TrackDecoder::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let (mut buf, mut frames, mut rate) = (Vec::new(), 0u64, 0);
    while let Some(spec) = decoder.next_frames(&mut buf).unwrap() {
        frames += (buf.len() / spec.channels) as u64;
        rate = spec.sample_rate;
    }
    (frames, rate)
}

fn request(source: &Path, dest: &Path, start: f64, end: f64, format: ExportFormat) -> ExportRequest {
    ExportRequest {
        source: source.to_path_buf(),
        dest: dest.to_path_buf(),
        start_sec: start,
        end_sec: end,
        format,
        overwrite: false,
    }
}

#[test]
fn plan_prefers_copy_and_locks_what_has_no_mp3_encoder() {
    use ExportFormat::*;
    use Method::*;
    use SourceKind as K;
    let table = [
        (K::Mp3, Current, Ok(Mp3FrameCopy), Ok(Mp3FrameCopy)),
        (K::Mp3, Mp3, Ok(Mp3FrameCopy), Ok(Mp3FrameCopy)),
        (K::Mp3, M4r, Ok(EncodeAac), Ok(EncodeAac)),
        (K::Wav, Current, Ok(Wav), Ok(Wav)),
        (K::Flac, Current, Ok(Flac), Ok(Flac)),
        (K::Flac, Mp3, Ok(EncodeMp3), Err(Unavailable::NoEncoderOnThisOs)),
        (K::Mp4, Current, Ok(AacCopy), Ok(AacCopy)),
        (K::Mp4, M4r, Ok(AacCopy), Ok(AacCopy)),
        (K::Wav, M4r, Ok(EncodeAac), Ok(EncodeAac)),
        (K::Other, Current, Err(Unavailable::NoEncoderForSource), Err(Unavailable::NoEncoderForSource)),
        (K::Other, Mp3, Ok(EncodeMp3), Err(Unavailable::NoEncoderOnThisOs)),
    ];
    for (kind, format, with_mp3, without_mp3) in table {
        assert_eq!(plan_for(kind, format, true), with_mp3, "{kind:?} {format:?} with MP3 encoder");
        assert_eq!(plan_for(kind, format, false), without_mp3, "{kind:?} {format:?} without MP3 encoder");
    }
    // M4R không còn khoá theo OS.
    assert_eq!(plan(Path::new("a/b.flac"), M4r), Ok(EncodeAac));
    assert_eq!(source_kind(Path::new("a/B.M4A")), SourceKind::Mp4);
    assert_eq!(source_kind(Path::new("a/b.ogg")), SourceKind::Other);
}

#[test]
fn extension_follows_format() {
    let source = Path::new("x/Bài.FLAC");
    assert_eq!(extension(source, ExportFormat::Current), "flac");
    assert_eq!(extension(source, ExportFormat::Mp3), "mp3");
    assert_eq!(extension(source, ExportFormat::M4r), "m4r");
}

#[test]
fn mp3_header_parses_lengths_and_rejects_other_layers() {
    let h = parse_header(&[0xFF, 0xFB, 0x90, 0x00]).unwrap();
    assert_eq!((h.len, h.sample_rate, h.samples), (417, 44_100, 1152));
    let padded = parse_header(&[0xFF, 0xFB, 0x92, 0x00]).unwrap();
    assert_eq!(padded.len, 418);
    // MPEG-2 Layer III 64 kbps 22,05 kHz: 72 * 64000 / 22050 = 208.
    let v2 = parse_header(&[0xFF, 0xF3, 0x80, 0x00]).unwrap();
    assert_eq!((v2.len, v2.sample_rate, v2.samples), (208, 22_050, 576));
    assert!(parse_header(&[0xFF, 0xFD, 0x90, 0x00]).is_none(), "Layer II");
    assert!(parse_header(&[0xFF, 0xFB, 0xF0, 0x00]).is_none(), "bitrate 15");
    assert!(parse_header(&[0xFF, 0xFB, 0x0C, 0x00]).is_none(), "free format / rate 3");
    assert!(parse_header(b"TAG\0").is_none());
}

#[test]
fn mp3_cut_copies_whole_frames_and_keeps_the_tag() {
    let dir = TestDir::new();
    let source =
        fixtures::write_mp3_with_frames(dir.path(), "song", &[("TIT2", "Bài hát"), ("TPE1", "Ca sĩ")]).unwrap();
    let original = std::fs::read(&source).unwrap();
    let audio = fixtures::audio_bytes(AudioKind::Mp3);
    let tag_len = original.len() - audio.len();

    let dest = dir.path().join("song (cut).mp3");
    let control = Control::default();
    run(&request(&source, &dest, 0.2, 0.6, ExportFormat::Current), &control).unwrap();
    let out = std::fs::read(&dest).unwrap();
    // Frame 26,12 ms: frame 7 (182,9–209,0 ms) tới frame 22 (574,7–600,8 ms).
    assert_eq!(&out[..tag_len], &original[..tag_len], "ID3v2 giữ nguyên");
    assert_eq!(&out[tag_len..], &audio[7 * 417..23 * 417], "frame chép nguyên, không mã lại");
    assert_eq!(control.progress(), 1.0);
    assert!(!part_path(&dest).exists());
    assert_eq!(std::fs::read(&source).unwrap(), original, "tệp gốc không đổi");
}

#[test]
fn mp3_cut_drops_the_xing_frame() {
    let dir = TestDir::new();
    let mut bytes = vec![0u8; 417];
    bytes[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
    bytes[36..40].copy_from_slice(b"Info");
    bytes.extend(fixtures::audio_bytes(AudioKind::Mp3));
    let source = dir.path().join("lame.mp3");
    std::fs::write(&source, &bytes).unwrap();
    let dest = dir.path().join("out.mp3");
    run(&request(&source, &dest, 0.0, 10.0, ExportFormat::Mp3), &Control::default()).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), fixtures::audio_bytes(AudioKind::Mp3), "hết bài: mọi frame trừ Info");
}

#[test]
fn wav_cut_keeps_rate_and_exact_length() {
    let dir = TestDir::new();
    let source = sine_wav(dir.path(), "sine.wav", 44_100, 2, 3.0);
    let dest = dir.path().join("sine (cut).wav");
    run(&request(&source, &dest, 1.0, 2.0, ExportFormat::Current), &Control::default()).unwrap();
    let (frames, rate) = decoded_frames(&dest);
    assert_eq!(rate, 44_100);
    assert_eq!(frames, 44_100);
    let header = std::fs::read(&dest).unwrap();
    assert_eq!(u16::from_le_bytes([header[34], header[35]]), 16, "16 bit như gốc");
}

#[test]
fn flac_cut_reencodes_losslessly() {
    let dir = TestDir::new();
    let source = fixtures::write_sample(dir.path(), "song", AudioKind::Flac).unwrap();
    let (all, rate) = decoded_frames(&source);
    let dest = dir.path().join("out.flac");
    run(&request(&source, &dest, 0.1, 0.3, ExportFormat::Current), &Control::default()).unwrap();
    let (frames, out_rate) = decoded_frames(&dest);
    assert_eq!(out_rate, rate);
    let wanted = (0.2 * f64::from(rate)).round() as u64;
    assert_eq!(frames, wanted.min(all), "{all} frame trong gốc");
    assert!(std::fs::read(&dest).unwrap().starts_with(b"fLaC"));
}

#[test]
fn never_writes_over_the_source_or_an_existing_file() {
    let dir = TestDir::new();
    let source = sine_wav(dir.path(), "a.wav", 8_000, 1, 1.0);
    let before = std::fs::read(&source).unwrap();
    let mut same = request(&source, &source, 0.0, 0.5, ExportFormat::Current);
    same.overwrite = true;
    assert_eq!(run(&same, &Control::default()), Err(ExportError::SameAsSource));
    let other_spelling = dir.path().join(".").join("a.wav");
    same.dest = other_spelling;
    assert_eq!(run(&same, &Control::default()), Err(ExportError::SameAsSource));
    assert_eq!(std::fs::read(&source).unwrap(), before);

    let existing = dir.path().join("b.wav");
    std::fs::write(&existing, b"keep").unwrap();
    let mut req = request(&source, &existing, 0.0, 0.5, ExportFormat::Current);
    assert_eq!(run(&req, &Control::default()), Err(ExportError::Exists));
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
    req.overwrite = true;
    run(&req, &Control::default()).unwrap();
    assert_ne!(std::fs::read(&existing).unwrap(), b"keep");
}

#[test]
fn empty_range_and_cancel_leave_nothing_behind() {
    let dir = TestDir::new();
    let source = sine_wav(dir.path(), "a.wav", 8_000, 1, 1.0);
    let dest = dir.path().join("out.wav");
    assert_eq!(
        run(&request(&source, &dest, 0.5, 0.55, ExportFormat::Current), &Control::default()),
        Err(ExportError::EmptyRange)
    );
    let control = Control::default();
    control.cancel();
    assert_eq!(run(&request(&source, &dest, 0.0, 1.0, ExportFormat::Current), &control), Err(ExportError::Cancelled));
    assert!(!dest.exists());
    assert!(!part_path(&dest).exists());
}

#[test]
fn job_reports_progress_and_finishes_once() {
    let dir = TestDir::new();
    let source = sine_wav(dir.path(), "a.wav", 44_100, 2, 2.0);
    let dest = dir.path().join("out.wav");
    let job = ExportJob::spawn(request(&source, &dest, 0.0, 2.0, ExportFormat::Current)).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let result = loop {
        if let Some(result) = job.try_finish() {
            break result;
        }
        assert!(std::time::Instant::now() < deadline, "xuất quá lâu");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(result, Ok(dest.clone()));
    assert_eq!(job.progress(), 1.0);
    assert!(job.try_finish().is_none(), "kết quả trả một lần");
}

#[cfg(windows)]
#[test]
fn windows_encodes_mp3_and_m4r_from_pcm() {
    let dir = TestDir::new();
    // 22,05 kHz mono: bộ mã hoá không nhận ⇒ đổi tần số trước.
    for (rate, channels) in [(44_100, 2), (22_050, 1)] {
        let source = sine_wav(dir.path(), &format!("sine-{rate}.wav"), rate, channels, 3.0);
        for format in [ExportFormat::Mp3, ExportFormat::M4r] {
            let dest = dir.path().join(format!("out-{rate}.{}", extension(&source, format)));
            run(&request(&source, &dest, 0.5, 1.5, format), &Control::default())
                .unwrap_or_else(|e| panic!("{rate} {format:?}: {e}"));
            let (frames, out_rate) = decoded_frames(&dest);
            let secs = frames as f64 / f64::from(out_rate);
            assert!((secs - 1.0).abs() < 0.12, "{rate} {format:?}: {secs} s");
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_cuts_aac_into_m4r() {
    let dir = TestDir::new();
    // M4A thật: mã từ WAV trước, rồi cắt sang M4R (chép nguyên mẫu AAC).
    let wav = sine_wav(dir.path(), "sine.wav", 48_000, 2, 4.0);
    let m4a = dir.path().join("sine.m4a");
    run(&request(&wav, &m4a, 0.0, 4.0, ExportFormat::M4r), &Control::default()).unwrap();
    let dest = dir.path().join("ring.m4r");
    run(&request(&m4a, &dest, 1.0, 3.0, ExportFormat::M4r), &Control::default()).unwrap();
    let (frames, rate) = decoded_frames(&dest);
    let secs = frames as f64 / f64::from(rate);
    assert!((secs - 2.0).abs() < 0.12, "{secs} s");
}

/// Đọc lại `.m4r`: brand, codec, tần số, độ dài trình bày (theo edit list) tính bằng giây.
#[cfg(not(windows))]
fn m4r_info(path: &Path) -> (String, filmcraft_isobmff::AacConfig, u32, f64) {
    let file = std::fs::File::open(path).unwrap();
    let mp4 = filmcraft_isobmff::open(&file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let track = &mp4.tracks[mp4.track_of_kind(filmcraft_isobmff::TrackKind::Audio).expect("audio track")];
    let Some(filmcraft_isobmff::CodecConfig::Aac(aac)) = track.codec() else { panic!("codec {:?}", track.codec()) };
    assert!(!track.samples.is_empty());
    let brand = String::from_utf8_lossy(&mp4.major_brand.0).into_owned();
    let rate = track.audio().map(|a| a.sample_rate as u32).unwrap_or_default();
    (brand, aac.clone(), rate, mp4.duration as f64 / f64::from(mp4.timescale))
}

#[cfg(not(windows))]
#[test]
fn encodes_m4r_from_pcm_without_os_encoder() {
    let dir = TestDir::new();
    // 22,05 kHz mono ⇒ đổi sang 44,1 kHz stereo như Windows; 48 kHz giữ nguyên.
    for (rate, channels, out_rate) in [(44_100, 2, 44_100), (22_050, 1, 44_100), (48_000, 2, 48_000)] {
        let source = sine_wav(dir.path(), &format!("sine-{rate}.wav"), rate, channels, 3.0);
        let dest = dir.path().join(format!("ring-{rate}.m4r"));
        run(&request(&source, &dest, 0.5, 1.75, ExportFormat::M4r), &Control::default())
            .unwrap_or_else(|e| panic!("{rate}: {e}"));
        let (brand, aac, got_rate, secs) = m4r_info(&dest);
        assert_eq!(brand, "M4A ", "{rate}");
        assert_eq!((aac.object_type, aac.channels()), (2, 2), "{rate}: AAC-LC stereo");
        assert_eq!(got_rate, out_rate, "{rate}");
        assert!((secs - 1.25).abs() < 0.001, "{rate}: {secs} s theo edit list");
        // Giải mã được bằng chính decoder của app (symphonia bỏ qua edit list ⇒ dư mẫu mồi + đệm cuối).
        let (frames, decoded_rate) = decoded_frames(&dest);
        let decoded = frames as f64 / f64::from(decoded_rate);
        assert!((decoded - 1.25).abs() < 0.1, "{rate}: decoded {decoded} s");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.contains(".part"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}

#[cfg(not(windows))]
#[test]
fn cuts_aac_into_m4r_by_copying_samples() {
    let dir = TestDir::new();
    // M4A thật: mã từ WAV trước, rồi cắt sang M4R (chép nguyên mẫu AAC, không mã lại).
    let wav = sine_wav(dir.path(), "sine.wav", 48_000, 2, 4.0);
    let m4a = dir.path().join("sine.m4a");
    run(&request(&wav, &m4a, 0.0, 4.0, ExportFormat::M4r), &Control::default()).unwrap();
    let (_, source_aac, _, source_secs) = m4r_info(&m4a);
    assert!((source_secs - 4.0).abs() < 0.001, "{source_secs}");
    let dest = dir.path().join("ring.m4r");
    run(&request(&m4a, &dest, 1.0, 3.0, ExportFormat::M4r), &Control::default()).unwrap();
    let (brand, aac, rate, secs) = m4r_info(&dest);
    assert_eq!(brand, "M4A ");
    assert_eq!(aac.asc, source_aac.asc, "chép nguyên cấu hình AAC của nguồn");
    assert_eq!(rate, 48_000);
    assert!((secs - 2.0).abs() < 0.001, "{secs} s");
    let (frames, decoded_rate) = decoded_frames(&dest);
    let decoded = frames as f64 / f64::from(decoded_rate);
    assert!((decoded - 2.0).abs() < 0.1, "decoded {decoded} s");
    // Chép nguyên mẫu: cỡ tệp xấp xỉ tỉ lệ đoạn cắt, không phải mã lại.
    let ratio = std::fs::metadata(&dest).unwrap().len() as f64 / std::fs::metadata(&m4a).unwrap().len() as f64;
    assert!((0.4..0.65).contains(&ratio), "{ratio}");
}

#[cfg(not(windows))]
#[test]
fn m4r_cancel_leaves_nothing_behind() {
    let dir = TestDir::new();
    let source = sine_wav(dir.path(), "sine.wav", 44_100, 2, 2.0);
    let dest = dir.path().join("ring.m4r");
    let control = Control::default();
    control.cancel();
    assert_eq!(run(&request(&source, &dest, 0.0, 2.0, ExportFormat::M4r), &control), Err(ExportError::Cancelled));
    let names: Vec<_> =
        std::fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()?.file_name().into_string().ok()).collect();
    assert_eq!(names, ["sine.wav"]);
}
