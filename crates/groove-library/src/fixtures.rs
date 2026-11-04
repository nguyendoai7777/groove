//! groove-gd1_06: sinh tệp nhạc mẫu cho test (M6).
//!
//! Tệp ghép tay từ byte, chỉ dùng std: chạy được ở mọi máy, không cần tệp nhạc thật. Mỗi tệp dài khoảng 1 giây và lofty,
//! ffprobe đều đọc được khung âm thanh. Riêng OGG có phần setup giả nên chỉ dùng để thử thẻ, không giải mã được.
//!
//! [`audio_bytes`] trả đoạn âm thanh phải còn nguyên sau khi ghi thẻ.

use std::io;
use std::path::{Path, PathBuf};

/// PNG 1 x 1, dùng làm bìa mẫu.
pub const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xB5, 0x1C, 0x0C, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x49,
    0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x64, 0x60, 0x00, 0x00, 0x00, 0x06, 0x00, 0x02, 0x30, 0x81, 0xD0, 0x2F, 0x00,
    0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// Dấu nhận ra phần âm thanh sau khi ghi thẻ.
const MARKER: &[u8; 16] = b"GROOVE-AUDIO-END";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioKind {
    Mp3,
    Flac,
    Wav,
    M4a,
    M4r,
    Ogg,
}

impl AudioKind {
    pub const ALL: [AudioKind; 6] =
        [AudioKind::Mp3, AudioKind::Flac, AudioKind::Wav, AudioKind::M4a, AudioKind::M4r, AudioKind::Ogg];

    pub fn extension(self) -> &'static str {
        match self {
            AudioKind::Mp3 => "mp3",
            AudioKind::Flac => "flac",
            AudioKind::Wav => "wav",
            AudioKind::M4a => "m4a",
            AudioKind::M4r => "m4r",
            AudioKind::Ogg => "ogg",
        }
    }
}

/// Toàn bộ byte của tệp mẫu, chưa có thẻ.
pub fn sample_bytes(kind: AudioKind) -> Vec<u8> {
    match kind {
        AudioKind::Mp3 => mp3_audio(),
        AudioKind::Flac => flac(),
        AudioKind::Wav => wav(),
        AudioKind::M4a | AudioKind::M4r => mp4(),
        AudioKind::Ogg => ogg(),
    }
}

/// Đoạn âm thanh của tệp mẫu; ghi thẻ xong phải tìm lại được nguyên vẹn trong tệp.
pub fn audio_bytes(kind: AudioKind) -> Vec<u8> {
    match kind {
        AudioKind::Mp3 => mp3_audio(),
        AudioKind::Flac => flac_frames(),
        AudioKind::Wav => wav_samples(),
        AudioKind::M4a | AudioKind::M4r => mp4_mdat_payload(),
        AudioKind::Ogg => ogg_audio_packet(),
    }
}

/// Ghi tệp mẫu `<stem>.<đuôi>` vào `dir`.
pub fn write_sample(dir: &Path, stem: &str, kind: AudioKind) -> io::Result<PathBuf> {
    let path = dir.join(format!("{stem}.{}", kind.extension()));
    std::fs::write(&path, sample_bytes(kind))?;
    Ok(path)
}

/// MP3 kèm sẵn các khung văn bản ID3v2.3.
pub fn write_mp3_with_frames(dir: &Path, stem: &str, frames: &[(&str, &str)]) -> io::Result<PathBuf> {
    use id3::TagLike;

    let path = write_sample(dir, stem, AudioKind::Mp3)?;
    let mut tag = id3::Tag::new();
    for (id, value) in frames {
        tag.set_text(*id, *value);
    }
    tag.write_to_path(&path, id3::Version::Id3v23).map_err(io::Error::other)?;
    Ok(path)
}

fn id3_bytes(frames: &[(&str, &str)], version: id3::Version) -> io::Result<Vec<u8>> {
    use id3::TagLike;

    let mut tag = id3::Tag::new();
    for (id, value) in frames {
        tag.set_text(*id, *value);
    }
    let mut bytes = Vec::new();
    tag.write_to(&mut bytes, version).map_err(io::Error::other)?;
    Ok(bytes)
}

/// MP3 có hai thẻ ID3v2 nối tiếp (v2.3 rồi v2.4), như tệp đã qua hai trình sửa thẻ khác nhau: `first` là bản sửa sau,
/// `second` là thẻ gốc.
pub fn write_mp3_double_id3(
    dir: &Path,
    stem: &str,
    first: &[(&str, &str)],
    second: &[(&str, &str)],
) -> io::Result<PathBuf> {
    let path = dir.join(format!("{stem}.mp3"));
    let mut bytes = id3_bytes(first, id3::Version::Id3v23)?;
    bytes.extend(id3_bytes(second, id3::Version::Id3v24)?);
    bytes.extend(mp3_audio());
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// MP3 có header ID3v2.3 khai kích thước nhỏ hơn thật `short_by` byte: đuôi thẻ thành rác nằm trước khung MPEG đầu.
pub fn write_mp3_short_tag_size(
    dir: &Path,
    stem: &str,
    frames: &[(&str, &str)],
    short_by: usize,
) -> io::Result<PathBuf> {
    let path = dir.join(format!("{stem}.mp3"));
    let mut bytes = id3_bytes(frames, id3::Version::Id3v23)?;
    let real = bytes.len() - 10;
    assert!(short_by < real, "short_by must leave a tag body");
    let declared = (real - short_by) as u32;
    for (i, shift) in [21, 14, 7, 0].into_iter().enumerate() {
        bytes[6 + i] = ((declared >> shift) & 0x7f) as u8;
    }
    bytes.extend(mp3_audio());
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Thêm đuôi ID3v1 trần, thứ làm trường đã xoá hiện lại trong Explorer.
pub fn append_id3v1(path: &Path, title: &str, album: &str) -> io::Result<()> {
    let mut block = vec![0u8; 128];
    block[0..3].copy_from_slice(b"TAG");
    let title = title.as_bytes();
    block[3..3 + title.len().min(30)].copy_from_slice(&title[..title.len().min(30)]);
    let album = album.as_bytes();
    block[63..63 + album.len().min(30)].copy_from_slice(&album[..album.len().min(30)]);
    block[127] = 255;
    let mut data = std::fs::read(path)?;
    data.extend_from_slice(&block);
    std::fs::write(path, data)
}

/// Có đuôi ID3v1 ở cuối tệp không.
pub fn has_id3v1(path: &Path) -> io::Result<bool> {
    let bytes = std::fs::read(path)?;
    Ok(bytes.len() >= 128 && &bytes[bytes.len() - 128..bytes.len() - 125] == b"TAG")
}

// --- MP3: MPEG1 Layer III, 128 kbps, 44,1 kHz, stereo. Khung toàn 0 giải mã thành im lặng. ---

fn mp3_audio() -> Vec<u8> {
    const FRAME_LEN: usize = 417;
    const FRAMES: usize = 39;
    let mut out = Vec::with_capacity(FRAME_LEN * FRAMES);
    for i in 0..FRAMES {
        let mut frame = vec![0u8; FRAME_LEN];
        frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        if i == FRAMES - 1 {
            // Vùng ancillary của khung cuối, decoder bỏ qua.
            frame[FRAME_LEN - MARKER.len()..].copy_from_slice(MARKER);
        }
        out.extend_from_slice(&frame);
    }
    out
}

// --- WAV: PCM 16 bit, mono, 8 kHz. ---

fn wav_samples() -> Vec<u8> {
    let mut samples: Vec<u8> = (0..16_000u32).map(|i| (i % 251) as u8).collect();
    let start = samples.len() - MARKER.len();
    samples[start..].copy_from_slice(MARKER);
    samples
}

fn wav() -> Vec<u8> {
    let samples = wav_samples();
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes()); // PCM
    fmt.extend_from_slice(&1u16.to_le_bytes()); // mono
    fmt.extend_from_slice(&8_000u32.to_le_bytes());
    fmt.extend_from_slice(&16_000u32.to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());

    let mut body = b"WAVE".to_vec();
    push_riff_chunk(&mut body, b"fmt ", &fmt);
    push_riff_chunk(&mut body, b"data", &samples);

    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

fn push_riff_chunk(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
}

// --- FLAC: mono, 16 bit, 44,1 kHz, subframe hằng, CRC đúng nên giải mã được. ---

const FLAC_BLOCK: u32 = 4096;
const FLAC_FRAMES: u8 = 11;

fn flac_frames() -> Vec<u8> {
    let mut out = Vec::new();
    for number in 0..FLAC_FRAMES {
        // Chặn cố định; khối 4096, 44,1 kHz; mono, 16 bit; số khung UTF-8 một byte.
        let mut frame = vec![0xFF, 0xF8, 0xC9, 0x08, number];
        frame.push(crc8(&frame));
        // Subframe hằng, mẫu 0.
        frame.extend_from_slice(&[0x00, 0x00, 0x00]);
        let crc = crc16(&frame);
        frame.extend_from_slice(&crc.to_be_bytes());
        out.extend_from_slice(&frame);
    }
    out
}

fn flac() -> Vec<u8> {
    let total_samples = u64::from(FLAC_BLOCK) * u64::from(FLAC_FRAMES);
    let mut out = b"fLaC".to_vec();
    out.extend_from_slice(&[0x80, 0x00, 0x00, 34]); // STREAMINFO, khối siêu dữ liệu cuối
    out.extend_from_slice(&(FLAC_BLOCK as u16).to_be_bytes());
    out.extend_from_slice(&(FLAC_BLOCK as u16).to_be_bytes());
    out.extend_from_slice(&[0u8; 6]); // cỡ khung tối thiểu, tối đa: chưa biết
    // 20 bit tần số, 3 bit kênh - 1, 5 bit số bit - 1, 36 bit tổng mẫu.
    let packed: u64 = (44_100u64 << 44) | (15u64 << 36) | total_samples;
    out.extend_from_slice(&packed.to_be_bytes());
    out.extend_from_slice(&[0u8; 16]); // MD5: chưa biết
    out.extend_from_slice(&flac_frames());
    out
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
    }
    crc
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in data {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x8005 } else { crc << 1 };
        }
    }
    crc
}

// --- OGG Vorbis: đủ ba gói đầu cho thẻ, phần setup giả. ---

const OGG_SERIAL: u32 = 0x4752_4F56;

fn ogg_audio_packet() -> Vec<u8> {
    let mut packet = vec![0u8; 64];
    packet.extend_from_slice(MARKER);
    packet
}

fn ogg() -> Vec<u8> {
    let mut identification = vec![0x01];
    identification.extend_from_slice(b"vorbis");
    identification.extend_from_slice(&0u32.to_le_bytes());
    identification.push(1); // mono
    identification.extend_from_slice(&44_100u32.to_le_bytes());
    identification.extend_from_slice(&0u32.to_le_bytes());
    identification.extend_from_slice(&96_000u32.to_le_bytes());
    identification.extend_from_slice(&0u32.to_le_bytes());
    identification.extend_from_slice(&[0xB8, 0x01]); // khối 256 / 2048, bit khung

    let vendor = b"groove-fixture";
    let mut comment = vec![0x03];
    comment.extend_from_slice(b"vorbis");
    comment.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    comment.extend_from_slice(vendor);
    comment.extend_from_slice(&0u32.to_le_bytes());
    comment.push(1);

    let mut setup = vec![0x05];
    setup.extend_from_slice(b"vorbis");
    setup.extend_from_slice(&[0u8; 32]);

    let mut out = Vec::new();
    push_ogg_page(&mut out, 0x02, 0, 0, &[&identification]);
    push_ogg_page(&mut out, 0x00, 0, 1, &[&comment, &setup]);
    push_ogg_page(&mut out, 0x04, 44_100, 2, &[&ogg_audio_packet()]);
    out
}

fn push_ogg_page(out: &mut Vec<u8>, flags: u8, granule: u64, sequence: u32, packets: &[&[u8]]) {
    let mut lacing = Vec::new();
    let mut payload = Vec::new();
    for packet in packets {
        lacing.extend(std::iter::repeat_n(255u8, packet.len() / 255));
        lacing.push((packet.len() % 255) as u8);
        payload.extend_from_slice(packet);
    }

    let mut page = b"OggS".to_vec();
    page.push(0);
    page.push(flags);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&OGG_SERIAL.to_le_bytes());
    page.extend_from_slice(&sequence.to_le_bytes());
    let crc_at = page.len();
    page.extend_from_slice(&0u32.to_le_bytes());
    page.push(lacing.len() as u8);
    page.extend_from_slice(&lacing);
    page.extend_from_slice(&payload);

    let crc = ogg_crc(&page);
    page[crc_at..crc_at + 4].copy_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&page);
}

fn ogg_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for byte in data {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04C1_1DB7 } else { crc << 1 };
        }
    }
    crc
}

// --- M4A / M4R: AAC-LC stereo, moov đứng trước mdat để ghi thẻ phải dời chunk offset. ---

/// Khung AAC-LC stereo im lặng.
const AAC_SILENCE: [u8; 9] = [0x21, 0x00, 0x49, 0x90, 0x02, 0x19, 0x00, 0x23, 0x80];
const AAC_FRAMES: u32 = 44;

fn mp4_mdat_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    for _ in 0..AAC_FRAMES {
        payload.extend_from_slice(&AAC_SILENCE);
    }
    payload.extend_from_slice(MARKER);
    payload
}

fn atom(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(name);
    out.extend_from_slice(body);
    out
}

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn mp4_moov(chunk_offset: u32) -> Vec<u8> {
    let duration = AAC_FRAMES * 1024;
    let be = |v: u32| v.to_be_bytes();
    const MATRIX: [u8; 36] =
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x40, 0, 0, 0];

    let mvhd = atom(
        b"mvhd",
        &concat(&[
            &[0u8; 12],
            &be(44_100),
            &be(duration),
            &be(0x0001_0000),
            &[0x01, 0x00],
            &[0u8; 10],
            &MATRIX,
            &[0u8; 24],
            &be(2),
        ]),
    );
    let tkhd = atom(
        b"tkhd",
        &concat(&[
            &be(0x0000_0007),
            &[0u8; 8],
            &be(1),
            &[0u8; 4],
            &be(duration),
            &[0u8; 8],
            &[0u8; 4],
            &[0x01, 0x00, 0, 0],
            &MATRIX,
            &[0u8; 8],
        ]),
    );
    let mdhd = atom(b"mdhd", &concat(&[&[0u8; 12], &be(44_100), &be(duration), &[0x55, 0xC4, 0, 0]]));
    let hdlr = atom(b"hdlr", &concat(&[&[0u8; 8], b"soun", &[0u8; 12], b"\0"]));

    let esds = atom(
        b"esds",
        &[
            0, 0, 0, 0, // version, flags
            0x03, 25, 0, 1, 0, // ES_Descriptor
            0x04, 17, 0x40, 0x15, 0, 0, 0, 0, 1, 0xF4, 0, 0, 1, 0xF4, 0, // DecoderConfig
            0x05, 2, 0x12, 0x10, // AudioSpecificConfig: AAC-LC, 44,1 kHz, stereo
            0x06, 1, 2, // SLConfig
        ],
    );
    let mp4a = atom(
        b"mp4a",
        &concat(&[&[0u8; 6], &[0, 1], &[0u8; 8], &[0, 2], &[0, 16], &[0u8; 4], &be(44_100 << 16), &esds]),
    );
    let stsd = atom(b"stsd", &concat(&[&[0u8; 4], &be(1), &mp4a]));
    let stts = atom(b"stts", &concat(&[&[0u8; 4], &be(1), &be(AAC_FRAMES), &be(1024)]));
    let stsc = atom(b"stsc", &concat(&[&[0u8; 4], &be(1), &be(1), &be(AAC_FRAMES), &be(1)]));
    let mut sizes = Vec::new();
    for _ in 0..AAC_FRAMES {
        sizes.extend_from_slice(&be(AAC_SILENCE.len() as u32));
    }
    let stsz = atom(b"stsz", &concat(&[&[0u8; 8], &be(AAC_FRAMES), &sizes]));
    let stco = atom(b"stco", &concat(&[&[0u8; 4], &be(1), &be(chunk_offset)]));
    let stbl = atom(b"stbl", &concat(&[&stsd, &stts, &stsc, &stsz, &stco]));

    let url = atom(b"url ", &be(1));
    let dref = atom(b"dref", &concat(&[&[0u8; 4], &be(1), &url]));
    let dinf = atom(b"dinf", &dref);
    let smhd = atom(b"smhd", &[0u8; 8]);
    let minf = atom(b"minf", &concat(&[&smhd, &dinf, &stbl]));
    let mdia = atom(b"mdia", &concat(&[&mdhd, &hdlr, &minf]));
    let trak = atom(b"trak", &concat(&[&tkhd, &mdia]));
    atom(b"moov", &concat(&[&mvhd, &trak]))
}

fn mp4() -> Vec<u8> {
    let ftyp = atom(b"ftyp", &concat(&[b"M4A ", &[0u8; 4], b"M4A ", b"isom"]));
    let moov_len = mp4_moov(0).len();
    let moov = mp4_moov((ftyp.len() + moov_len + 8) as u32);
    let mdat = atom(b"mdat", &mp4_mdat_payload());
    concat(&[&ftyp, &moov, &mdat])
}

/// Chunk offset đầu tiên trong `stco` của tệp MP4, để test soát ghi thẻ có dời đúng không.
pub fn mp4_first_chunk_offset(bytes: &[u8]) -> Option<usize> {
    let at = bytes.windows(4).position(|w| w == b"stco")?;
    let raw: [u8; 4] = bytes.get(at + 12..at + 16)?.try_into().ok()?;
    Some(u32::from_be_bytes(raw) as usize)
}

/// Khung AAC đầu tiên của tệp mẫu MP4.
pub fn mp4_first_frame() -> &'static [u8] {
    &AAC_SILENCE
}
