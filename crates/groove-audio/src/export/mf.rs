//! groove-gd2_02: bộ mã hoá MP3 / AAC của Windows (Media Foundation — có sẵn trong Windows 8+, không kèm thư viện nào, D-2).
//! PCM giải mã bằng symphonia ([`ClipReader`]) ⇒ Sink Writer (MP3 trong `.mp3`, AAC trong MPEG-4). AAC trong MP4 thì chép nguyên
//! mẫu (Source Reader ⇒ Sink Writer, kiểu vào = kiểu ra nên không mã lại); bộ ghi từ chối thì mã lại. Bản Windows N không có
//! Media Feature Pack ⇒ `ExportError::Encoder`. Mẫu theo `fastcast/crates/video/src/transcode_mf.rs`.

use std::mem::ManuallyDrop;
use std::path::Path;

use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Variant::VT_I8;
use windows::core::{GUID, HSTRING};

use super::pcm::ClipReader;
use super::{Control, ExportError};
use crate::decoder::Converter;
use crate::output::OutputConfig;

/// `MF_SOURCE_READER_FIRST_AUDIO_STREAM` / `_ALL_STREAMS` (hằng âm của SDK, kiểu `DWORD`).
const FIRST_AUDIO: u32 = 0xFFFF_FFFD;
const ALL_STREAMS: u32 = 0xFFFF_FFFE;
/// Đơn vị thời gian của Media Foundation: 100 ns.
const TICKS_PER_SEC: f64 = 10_000_000.0;
/// Một lượt mẫu gửi bộ ghi (~0,1 s ở 48 kHz).
const CHUNK_FRAMES: usize = 4800;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Codec {
    /// 320 kbps stereo (bộ mã hoá MP3 của Microsoft nhận 32 / 44,1 / 48 kHz).
    Mp3,
    /// 192 kbps stereo (bộ mã hoá AAC nhận 44,1 / 48 kHz; bitrate 96 / 128 / 160 / 192).
    Aac,
}

impl Codec {
    fn subtype(self) -> GUID {
        match self {
            Codec::Mp3 => MFAudioFormat_MP3,
            Codec::Aac => MFAudioFormat_AAC,
        }
    }

    fn container(self) -> GUID {
        match self {
            Codec::Mp3 => MFTranscodeContainerType_MP3,
            Codec::Aac => MFTranscodeContainerType_MPEG4,
        }
    }

    fn bytes_per_sec(self) -> u32 {
        match self {
            Codec::Mp3 => 320_000 / 8,
            Codec::Aac => 192_000 / 8,
        }
    }

    /// Tần số bộ mã hoá nhận: giữ của nguồn nếu được, không thì đổi sang mức gần nhất.
    fn sample_rate(self, source: u32) -> u32 {
        match (self, source) {
            (_, 44_100 | 48_000) => source,
            (Codec::Mp3, 32_000) => source,
            (_, rate) if rate > 44_100 => 48_000,
            _ => 44_100,
        }
    }
}

struct Session;

impl Session {
    fn start() -> Result<Session, ExportError> {
        // SAFETY: cặp với `CoUninitialize` / `MFShutdown` trong `Drop`, cùng luồng xuất.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().map_err(|e| failed("CoInitializeEx", &e))?;
            if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) {
                CoUninitialize();
                return Err(failed("MFStartup", &e));
            }
        }
        Ok(Session)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: mọi interface MF đã rơi trước (khai sau `Session` trong hàm gọi).
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}

fn failed(what: &str, e: &windows::core::Error) -> ExportError {
    log::error!("export: {what} failed ({:#010x})", e.code().0 as u32);
    // Không ghi được tệp (đường dẫn, quyền, đĩa đầy) ⇒ lỗi ghi; còn lại là bộ mã hoá.
    match e.code().0 as u32 {
        0x8007_0002..=0x8007_0005 | 0x8007_0070 | 0x8007_007B => ExportError::Write,
        _ => ExportError::Encoder,
    }
}

fn bad(what: &'static str) -> impl Fn(windows::core::Error) -> ExportError {
    move |e| failed(what, &e)
}

fn refuse(what: &'static str) -> impl Fn(windows::core::Error) {
    move |e| log::info!("export: aac passthrough refused at {what} ({:#010x}), re-encoding", e.code().0 as u32)
}

fn writer_attributes(container: &GUID) -> windows::core::Result<IMFAttributes> {
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
        // Đuôi `.part` ⇒ phải nói rõ container.
        attributes.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, container)?;
        Ok(attributes)
    }
}

fn audio_type(subtype: &GUID, rate: u32, channels: u32, bytes_per_sec: u32) -> windows::core::Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
        t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels)?;
        t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_sec)?;
        if *subtype == MFAudioFormat_PCM || *subtype == MFAudioFormat_AAC {
            t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        }
        if *subtype == MFAudioFormat_PCM {
            t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, channels * 2)?;
            t.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        }
        Ok(t)
    }
}

/// Giải mã đoạn rồi mã `codec`.
pub(super) fn encode(
    source: &Path,
    dest: &Path,
    start: f64,
    end: f64,
    codec: Codec,
    control: &Control,
) -> Result<(), ExportError> {
    let mut clip = ClipReader::open(source, start, end)?;
    let spec = clip.spec();
    // Luôn stereo: bộ mã hoá MP3 của Microsoft từ chối mono (MF_E_INVALIDMEDIATYPE ở SetInputMediaType).
    let target = OutputConfig { sample_rate: codec.sample_rate(spec.sample_rate), channels: 2 };
    let mut converter = (spec.sample_rate != target.sample_rate || spec.channels != usize::from(target.channels))
        .then(|| Converter::new(spec, target))
        .transpose()
        .map_err(ExportError::Source)?;
    let channels = u32::from(target.channels);

    let _session = Session::start()?;
    let writer = unsafe {
        let writer = MFCreateSinkWriterFromURL(
            &HSTRING::from(dest.as_os_str()),
            None,
            &writer_attributes(&codec.container()).map_err(bad("attributes"))?,
        )
        .map_err(bad("MFCreateSinkWriterFromURL"))?;
        let out = audio_type(&codec.subtype(), target.sample_rate, channels, codec.bytes_per_sec())
            .map_err(bad("output type"))?;
        let stream = writer.AddStream(&out).map_err(bad("AddStream"))?;
        let input = audio_type(&MFAudioFormat_PCM, target.sample_rate, channels, target.sample_rate * channels * 2)
            .map_err(bad("input type"))?;
        writer.SetInputMediaType(stream, &input, None).map_err(bad("SetInputMediaType"))?;
        writer.BeginWriting().map_err(bad("BeginWriting"))?;
        PcmWriter {
            writer,
            stream,
            rate: target.sample_rate,
            channels: channels as usize,
            frames: 0,
            pending: Vec::new(),
        }
    };
    let mut writer = writer;
    let (mut buf, mut converted) = (Vec::new(), Vec::new());
    while clip.next(&mut buf)? {
        control.check()?;
        match &mut converter {
            Some(converter) => {
                converter.push(&buf, &mut converted).map_err(ExportError::Source)?;
                writer.push(&converted)?;
                converted.clear();
            }
            None => writer.push(&buf)?,
        }
        control.set_progress(clip.progress() * 0.98);
    }
    if let Some(converter) = &mut converter {
        converter.finish(&mut converted).map_err(ExportError::Source)?;
        writer.push(&converted)?;
    }
    writer.finish()
}

struct PcmWriter {
    writer: IMFSinkWriter,
    stream: u32,
    rate: u32,
    channels: usize,
    frames: u64,
    /// Mẫu chưa gửi (gom đủ [`CHUNK_FRAMES`]).
    pending: Vec<i16>,
}

impl PcmWriter {
    fn push(&mut self, samples: &[f32]) -> Result<(), ExportError> {
        self.pending.extend(samples.iter().map(|&s| super::quantize(s, 16) as i16));
        while self.pending.len() >= CHUNK_FRAMES * self.channels {
            let rest = self.pending.split_off(CHUNK_FRAMES * self.channels);
            let chunk = std::mem::replace(&mut self.pending, rest);
            self.write(&chunk)?;
        }
        Ok(())
    }

    fn write(&mut self, samples: &[i16]) -> Result<(), ExportError> {
        if samples.is_empty() {
            return Ok(());
        }
        let frames = (samples.len() / self.channels) as u64;
        let bytes = samples.len() * 2;
        let time = |frames: u64| (frames as f64 * TICKS_PER_SEC / f64::from(self.rate)).round() as i64;
        unsafe {
            let buffer = MFCreateMemoryBuffer(bytes as u32).map_err(bad("MFCreateMemoryBuffer"))?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None).map_err(bad("Lock"))?;
            // SAFETY: bộ đệm vừa cấp `bytes` byte, khoá xong mới chép, chép xong mở khoá.
            std::ptr::copy_nonoverlapping(samples.as_ptr().cast::<u8>(), data, bytes);
            buffer.Unlock().map_err(bad("Unlock"))?;
            buffer.SetCurrentLength(bytes as u32).map_err(bad("SetCurrentLength"))?;
            let sample = MFCreateSample().map_err(bad("MFCreateSample"))?;
            sample.AddBuffer(&buffer).map_err(bad("AddBuffer"))?;
            sample.SetSampleTime(time(self.frames)).map_err(bad("SetSampleTime"))?;
            sample
                .SetSampleDuration(time(self.frames + frames) - time(self.frames))
                .map_err(bad("SetSampleDuration"))?;
            self.writer.WriteSample(self.stream, &sample).map_err(bad("WriteSample"))?;
        }
        self.frames += frames;
        Ok(())
    }

    fn finish(mut self) -> Result<(), ExportError> {
        let rest = std::mem::take(&mut self.pending);
        self.write(&rest)?;
        if self.frames == 0 {
            return Err(ExportError::EmptyRange);
        }
        unsafe { self.writer.Finalize() }.map_err(|e| failed("Finalize", &e))
    }
}

fn time_variant(ticks: i64) -> PROPVARIANT {
    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_I8,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 { hVal: ticks },
            }),
        },
    }
}

/// AAC trong MP4 ⇒ chép nguyên các mẫu giao với đoạn; nguồn không phải AAC / bộ ghi từ chối ⇒ mã lại AAC.
pub(super) fn copy_aac(source: &Path, dest: &Path, start: f64, end: f64, control: &Control) -> Result<(), ExportError> {
    let copied = {
        let _session = Session::start()?;
        // SAFETY: COM + MF đã khởi tạo trên luồng này; mọi interface rơi trong khối, trước `_session`.
        unsafe { try_copy_aac(source, dest, start, end, control) }
    };
    match copied {
        Ok(true) => Ok(()),
        Ok(false) => {
            let _ = std::fs::remove_file(dest);
            encode(source, dest, start, end, Codec::Aac, control)
        }
        Err(err) => Err(err),
    }
}

/// `Ok(false)` = không chép nguyên được (chưa ghi gì), bên gọi mã lại.
unsafe fn try_copy_aac(
    source: &Path,
    dest: &Path,
    start: f64,
    end: f64,
    control: &Control,
) -> Result<bool, ExportError> {
    unsafe {
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(source.as_os_str()), None)
            .map_err(bad("MFCreateSourceReaderFromURL"))?;
        reader.SetStreamSelection(ALL_STREAMS, false).map_err(bad("SetStreamSelection"))?;
        reader.SetStreamSelection(FIRST_AUDIO, true).map_err(bad("SetStreamSelection"))?;
        let native = reader.GetNativeMediaType(FIRST_AUDIO, 0).map_err(bad("GetNativeMediaType"))?;
        if native.GetGUID(&MF_MT_SUBTYPE).ok() != Some(MFAudioFormat_AAC) {
            return Ok(false);
        }
        let attributes = writer_attributes(&MFTranscodeContainerType_MPEG4).map_err(bad("attributes"))?;
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(dest.as_os_str()), None, &attributes)
            .map_err(bad("MFCreateSinkWriterFromURL"))?;
        let Ok(stream) = writer.AddStream(&native).map_err(refuse("AddStream")) else { return Ok(false) };
        if writer.SetInputMediaType(stream, &native, None).map_err(refuse("SetInputMediaType")).is_err()
            || writer.BeginWriting().map_err(refuse("BeginWriting")).is_err()
        {
            return Ok(false);
        }
        let (start_ticks, end_ticks) = ((start * TICKS_PER_SEC) as i64, (end * TICKS_PER_SEC) as i64);
        reader.SetCurrentPosition(&GUID::zeroed(), &time_variant(start_ticks)).map_err(bad("SetCurrentPosition"))?;
        let mut origin: Option<i64> = None;
        loop {
            control.check()?;
            let (mut flags, mut timestamp, mut sample) = (0u32, 0i64, None);
            reader
                .ReadSample(FIRST_AUDIO, 0, None, Some(&mut flags), Some(&mut timestamp), Some(&mut sample))
                .map_err(bad("ReadSample"))?;
            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return Err(ExportError::Source(crate::decoder::AudioErrorKind::Corrupt));
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 || timestamp >= end_ticks {
                break;
            }
            let Some(sample) = sample else { continue };
            let duration = sample.GetSampleDuration().unwrap_or(0);
            if timestamp + duration <= start_ticks {
                continue;
            }
            let origin = *origin.get_or_insert(timestamp);
            sample.SetSampleTime(timestamp - origin).map_err(bad("SetSampleTime"))?;
            writer.WriteSample(stream, &sample).map_err(bad("WriteSample"))?;
            control.set_progress((timestamp + duration - start_ticks) as f64 / (end_ticks - start_ticks).max(1) as f64);
        }
        if origin.is_none() {
            return Err(ExportError::EmptyRange);
        }
        writer.Finalize().map_err(bad("Finalize"))?;
        Ok(true)
    }
}
