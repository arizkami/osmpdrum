//! # osmpcore
//!
//! Core I/O library for the `.osmp` **multi-sample container** format.
//!
//! ## File layout
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │  CONTAINER HEADER   64 bytes (fixed)             │
//! ├──────────────────────────────────────────────────┤
//! │  SAMPLE TABLE       num_samples × 64 bytes       │
//! │  (capacity × 64 bytes reserved by writer)        │
//! ├──────────────────────────────────────────────────┤
//! │  AUDIO DATA  — raw s24le or s32le PCM,           │
//! │                interleaved, little-endian         │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! One container can hold up to ~3,200 individual sample entries.
//! Each entry carries its own `sample_rate`, `channels`, `format`, and name.
//! Supported sample rates: 44 100, 48 000, 88 200, 96 000 Hz.
//! Supported formats: `S24LE` (3 bytes/sample, packed) and `S32LE` (4 bytes/sample).

use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::Mmap;
use thiserror::Error;

// ── Constants ──────────────────────────────────────────────────────────────────

pub const MAGIC: [u8; 4]          = *b"OSMP";
pub const VERSION: u32             = 1;
/// Version with embedded JSON zone map.
pub const VERSION_2: u32           = 2;
pub const HEADER_SIZE: usize       = 64;
pub const SAMPLE_ENTRY_SIZE: usize = 64;
/// Practical upper bound: 3200 × 64 B = 200 KB table.
pub const MAX_SAMPLES: usize       = 6400;

/// Raw PCM sample format stored in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SampleFormat {
    /// Signed 24-bit little-endian, **packed** (3 bytes per value).
    S24LE = 24,
    /// Signed 32-bit little-endian (4 bytes per value).
    S32LE = 32,
}

impl SampleFormat {
    #[inline]
    pub fn bytes_per_value(self) -> usize {
        match self { SampleFormat::S24LE => 3, SampleFormat::S32LE => 4 }
    }
    pub fn from_u16(v: u16) -> Option<Self> {
        match v { 24 => Some(Self::S24LE), 32 => Some(Self::S32LE), _ => None }
    }
}

pub mod flags {
    /// Container / sample-level: sample should loop between loop_start and loop_end.
    pub const LOOPING: u32 = 0x01;
    /// Sample has cue / region markers.
    pub const HAS_CUE: u32 = 0x02;
}

// ── Error ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum OsmpError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid magic bytes – not an .osmp file")]
    BadMagic,
    #[error("unsupported file version: {0}")]
    BadVersion(u32),
    #[error("unsupported sample format: {0}")]
    BadFormat(u16),
    #[error("sample index {0} out of range (container has {1})")]
    IndexOutOfRange(usize, usize),
    #[error("capacity {0} exceeded (max {1})")]
    CapacityExceeded(usize, usize),
    #[error("file is truncated or corrupted")]
    Truncated,
    #[error("JSON parse error: {0}")]
    InvalidJson(String),
}

pub type Result<T> = std::result::Result<T, OsmpError>;

pub mod sfzjson;

// ── Container header (64 bytes) ────────────────────────────────────────────────

/// Fixed 64-byte container header.
///
/// ```text
/// [ 0.. 4]  magic        [u8; 4]  = b"OSMP"
/// [ 4.. 8]  version      u32      = 1
/// [ 8..12]  num_samples  u32
/// [12..16]  flags        u32
/// [16..48]  name         [u8; 32]  null-padded UTF-8
/// [48..64]  reserved     [u8; 16]
/// ```
#[derive(Debug, Clone)]
pub struct ContainerHeader {
    pub version:     u32,
    pub num_samples: u32,
    pub flags:       u32,
    /// Container / kit name (≤ 31 UTF-8 bytes).
    pub name:        String,
    /// Byte length of the embedded JSON zone-map blob (0 = none, v1 compat).
    /// The blob is stored immediately after the 64-byte header, zero-padded
    /// to the next 64-byte boundary before the sample table begins.
    pub json_len:    u32,
}

impl ContainerHeader {
    pub fn new(name: impl Into<String>) -> Self {
        Self { version: VERSION, num_samples: 0, flags: 0, name: name.into(), json_len: 0 }
    }

    pub fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if magic != MAGIC { return Err(OsmpError::BadMagic); }

        let version     = r.read_u32::<LittleEndian>()?;
        if version != VERSION && version != VERSION_2 {
            return Err(OsmpError::BadVersion(version));
        }

        let num_samples = r.read_u32::<LittleEndian>()?;
        let flags       = r.read_u32::<LittleEndian>()?;

        let mut name_buf = [0u8; 32];
        r.read_exact(&mut name_buf)?;
        let name = std::str::from_utf8(&name_buf)
            .unwrap_or("").trim_end_matches('\0').to_owned();

        // bytes [48..52] = json_len (was "reserved" in v1, safely zero)
        let json_len = r.read_u32::<LittleEndian>()?;
        let mut _reserved = [0u8; 12];
        r.read_exact(&mut _reserved)?;

        Ok(Self { version, num_samples, flags, name, json_len })
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<()> {
        w.write_all(&MAGIC)?;
        w.write_u32::<LittleEndian>(self.version)?;
        w.write_u32::<LittleEndian>(self.num_samples)?;
        w.write_u32::<LittleEndian>(self.flags)?;

        let mut nb = [0u8; 32];
        let b = self.name.as_bytes();
        nb[..b.len().min(31)].copy_from_slice(&b[..b.len().min(31)]);
        w.write_all(&nb)?;
        // [48..52] json_len, [52..64] reserved
        w.write_u32::<LittleEndian>(self.json_len)?;
        w.write_all(&[0u8; 12])?;
        Ok(())
    }

    /// Bytes the JSON blob occupies on disk (padded to 64-byte boundary).
    #[inline] pub fn json_blob_padded(json_len: u32) -> u64 {
        let n = json_len as u64;
        if n == 0 { 0 } else { (n + 63) & !63 }
    }

    /// Byte offset of the sample table.
    #[inline] pub fn table_offset(json_len: u32) -> u64 {
        HEADER_SIZE as u64 + Self::json_blob_padded(json_len)
    }

    /// Byte offset where audio data begins given `capacity` reserved entries.
    #[inline] pub fn audio_offset(capacity: u32, json_len: u32) -> u64 {
        Self::table_offset(json_len) + capacity as u64 * SAMPLE_ENTRY_SIZE as u64
    }
}

// ── Sample entry (64 bytes) ────────────────────────────────────────────────────

/// One row of the sample table — 64 bytes.
///
/// ```text
/// [ 0.. 8]  data_offset   u64
/// [ 8..16]  data_len      u64   bytes of raw PCM
/// [16..20]  sample_rate   u32   44100 | 48000 | 88200 | 96000
/// [20..22]  channels      u16   1 | 2
/// [22..24]  format        u16   24=S24LE  32=S32LE
/// [24..28]  num_frames    u32   total frames per channel
/// [28..32]  loop_start    u32   frame index (0 = beginning)
/// [32..36]  loop_end      u32   frame index (0 = end of sample)
/// [36..37]  velocity_lo   u8    MIDI velocity range low  (default 0)
/// [37..38]  velocity_hi   u8    MIDI velocity range high (default 127)
/// [38..40]  flags         u16   LOOPING=0x01
/// [40..64]  name          [u8; 24]  null-padded UTF-8
/// ```
#[derive(Debug, Clone)]
pub struct SampleEntry {
    /// Absolute byte offset of this sample's PCM data in the container file.
    pub data_offset:  u64,
    /// Byte length of the raw PCM data block.
    pub data_len:     u64,
    pub sample_rate:  u32,
    pub channels:     u16,
    pub format:       SampleFormat,
    /// Total audio frames (per channel).
    pub num_frames:   u32,
    /// Loop start frame (0 = from beginning).
    pub loop_start:   u32,
    /// Loop end frame (0 = until end of sample).
    pub loop_end:     u32,
    pub velocity_lo:  u8,
    pub velocity_hi:  u8,
    pub flags:        u16,
    /// Sample name (≤ 23 UTF-8 bytes, null-padded).
    pub name:         String,
}

impl SampleEntry {
    pub fn new(
        name: impl Into<String>,
        sample_rate: u32,
        channels: u16,
        format: SampleFormat,
    ) -> Self {
        Self {
            data_offset: 0, data_len: 0,
            sample_rate, channels, format,
            num_frames: 0, loop_start: 0, loop_end: 0,
            velocity_lo: 0, velocity_hi: 127,
            flags: 0, name: name.into(),
        }
    }

    pub fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let data_offset  = r.read_u64::<LittleEndian>()?;
        let data_len     = r.read_u64::<LittleEndian>()?;
        let sample_rate  = r.read_u32::<LittleEndian>()?;
        let channels     = r.read_u16::<LittleEndian>()?;
        let fmt_raw      = r.read_u16::<LittleEndian>()?;
        let format       = SampleFormat::from_u16(fmt_raw).ok_or(OsmpError::BadFormat(fmt_raw))?;
        let num_frames   = r.read_u32::<LittleEndian>()?;
        let loop_start   = r.read_u32::<LittleEndian>()?;
        let loop_end     = r.read_u32::<LittleEndian>()?;
        let velocity_lo  = r.read_u8()?;
        let velocity_hi  = r.read_u8()?;
        let flags        = r.read_u16::<LittleEndian>()?;

        let mut nb = [0u8; 24];
        r.read_exact(&mut nb)?;
        let name = std::str::from_utf8(&nb).unwrap_or("").trim_end_matches('\0').to_owned();

        Ok(Self { data_offset, data_len, sample_rate, channels, format,
                  num_frames, loop_start, loop_end, velocity_lo, velocity_hi, flags, name })
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> Result<()> {
        w.write_u64::<LittleEndian>(self.data_offset)?;
        w.write_u64::<LittleEndian>(self.data_len)?;
        w.write_u32::<LittleEndian>(self.sample_rate)?;
        w.write_u16::<LittleEndian>(self.channels)?;
        w.write_u16::<LittleEndian>(self.format as u16)?;
        w.write_u32::<LittleEndian>(self.num_frames)?;
        w.write_u32::<LittleEndian>(self.loop_start)?;
        w.write_u32::<LittleEndian>(self.loop_end)?;
        w.write_u8(self.velocity_lo)?;
        w.write_u8(self.velocity_hi)?;
        w.write_u16::<LittleEndian>(self.flags)?;

        let mut nb = [0u8; 24];
        let b = self.name.as_bytes();
        nb[..b.len().min(23)].copy_from_slice(&b[..b.len().min(23)]);
        w.write_all(&nb)?;
        Ok(())
    }

    /// Bytes per interleaved frame (channels × bytes_per_value).
    #[inline] pub fn bytes_per_frame(&self) -> usize {
        self.channels as usize * self.format.bytes_per_value()
    }

    pub fn is_looping(&self) -> bool { self.flags & flags::LOOPING as u16 != 0 }
}

// ── In-memory container ────────────────────────────────────────────────────────

/// Opened `.osmp` container: header + sample table loaded; audio read on demand.
pub struct OsmpFile {
    pub header:  ContainerHeader,
    pub samples: Vec<SampleEntry>,
    file:        File,
}

impl OsmpFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut br   = BufReader::new(&mut file);
        let header   = ContainerHeader::read_from(&mut br)?;
        // Skip over JSON blob (if any) to reach the sample table
        let table_off = ContainerHeader::table_offset(header.json_len);
        if header.json_len > 0 {
            br.seek(SeekFrom::Start(table_off))?;
        }
        let mut samples = Vec::with_capacity(header.num_samples as usize);
        for _ in 0..header.num_samples {
            samples.push(SampleEntry::read_from(&mut br)?);
        }
        Ok(Self { header, samples, file })
    }

    /// Read the embedded JSON zone-map string, or `None` if not present.
    pub fn read_json(&mut self) -> Result<Option<String>> {
        if self.header.json_len == 0 { return Ok(None); }
        self.file.seek(SeekFrom::Start(HEADER_SIZE as u64))?;
        let mut buf = vec![0u8; self.header.json_len as usize];
        self.file.read_exact(&mut buf)?;
        Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
    }

    /// Decode sample `index` fully into `Vec<f32>` (normalised −1.0 … +1.0).
    pub fn read_sample(&mut self, index: usize) -> Result<Vec<f32>> {
        let n  = self.samples.len();
        let se = self.samples.get(index).cloned().ok_or(OsmpError::IndexOutOfRange(index, n))?;
        self.file.seek(SeekFrom::Start(se.data_offset))?;
        let num_values = se.data_len as usize / se.format.bytes_per_value();
        decode_read(&mut self.file, se.format, num_values)
    }

    /// Build a streaming reader for sample `index`.
    pub fn stream_reader(&mut self, index: usize) -> Result<OsmpStreamReader<&mut File>> {
        let n  = self.samples.len();
        let se = self.samples.get(index).cloned().ok_or(OsmpError::IndexOutOfRange(index, n))?;
        self.file.seek(SeekFrom::Start(se.data_offset))?;
        Ok(OsmpStreamReader::new(&mut self.file, se))
    }
}

// ── Streaming reader ───────────────────────────────────────────────────────────

/// Pull audio frame-by-frame from disk without loading the full sample into RAM.
///
/// ```rust,ignore
/// let mut file   = OsmpFile::open("kit.osmp")?;
/// let mut stream = file.stream_reader(0)?;
/// while let Some(frames) = stream.next_chunk(512)? {
///     // process frames (Vec<f32>, interleaved)
/// }
/// ```
pub struct OsmpStreamReader<R: Read + Seek> {
    inner:           R,
    format:          SampleFormat,
    bytes_per_frame: usize,
    data_start:      u64,
    data_len:        u64,
    /// Bytes consumed so far from this sample's data block.
    pos:             u64,
}

impl<R: Read + Seek> OsmpStreamReader<R> {
    pub fn new(inner: R, entry: SampleEntry) -> Self {
        Self {
            inner,
            format:          entry.format,
            bytes_per_frame: entry.bytes_per_frame(),
            data_start:      entry.data_offset,
            data_len:        entry.data_len,
            pos:             0,
        }
    }

    /// Decode up to `frames` frames.  Returns `None` when exhausted.
    pub fn next_chunk(&mut self, frames: usize) -> Result<Option<Vec<f32>>> {
        let remaining = self.data_len.saturating_sub(self.pos);
        if remaining == 0 { return Ok(None); }

        let want  = frames as u64 * self.bytes_per_frame as u64;
        let take  = want.min(remaining) as usize;
        let nvals = take / self.format.bytes_per_value();

        let mut buf = vec![0u8; take];
        self.inner.read_exact(&mut buf)?;
        self.pos += take as u64;

        Ok(Some(decode_bytes(&buf, self.format, nvals)))
    }

    /// Seek to `frame` relative to the start of this sample.
    pub fn seek_to_frame(&mut self, frame: u64) -> Result<()> {
        let off = frame * self.bytes_per_frame as u64;
        if off > self.data_len { return Err(OsmpError::Truncated); }
        self.inner.seek(SeekFrom::Start(self.data_start + off))?;
        self.pos = off;
        Ok(())
    }

    pub fn remaining_frames(&self) -> u64 {
        self.data_len.saturating_sub(self.pos) / self.bytes_per_frame as u64
    }

    pub fn is_exhausted(&self) -> bool { self.pos >= self.data_len }
}

// ── Memory-mapped reader ───────────────────────────────────────────────────────

/// Zero-copy reader backed by `mmap(2)`.  Best for random access.
pub struct OsmpMmapReader {
    pub header:  ContainerHeader,
    pub samples: Vec<SampleEntry>,
    mmap:        Mmap,
}

impl OsmpMmapReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let mut cur = std::io::Cursor::new(&mmap[..]);
        let header  = ContainerHeader::read_from(&mut cur)?;
        // Skip over JSON blob to reach sample table
        let table_off = ContainerHeader::table_offset(header.json_len);
        if header.json_len > 0 {
            cur.set_position(table_off);
        }
        let mut samples = Vec::with_capacity(header.num_samples as usize);
        for _ in 0..header.num_samples {
            samples.push(SampleEntry::read_from(&mut cur)?);
        }
        Ok(Self { header, samples, mmap })
    }

    /// Raw bytes of the embedded JSON blob, or `None` if not present.
    pub fn json_bytes(&self) -> Option<&[u8]> {
        if self.header.json_len == 0 { return None; }
        let start = HEADER_SIZE;
        let end   = start + self.header.json_len as usize;
        if end > self.mmap.len() { return None; }
        Some(&self.mmap[start..end])
    }

    /// Decode the embedded JSON blob as a UTF-8 string.
    pub fn read_json(&self) -> Option<String> {
        self.json_bytes().map(|b| String::from_utf8_lossy(b).into_owned())
    }

    /// Raw bytes of sample `index` — zero allocation, zero copy.
    pub fn sample_bytes(&self, index: usize) -> Result<&[u8]> {
        let n  = self.samples.len();
        let se = self.samples.get(index).ok_or(OsmpError::IndexOutOfRange(index, n))?;
        let s  = se.data_offset as usize;
        let e  = s + se.data_len as usize;
        if e > self.mmap.len() { return Err(OsmpError::Truncated); }
        Ok(&self.mmap[s..e])
    }

    /// Decode sample `index` to `Vec<f32>`.
    pub fn sample_f32(&self, index: usize) -> Result<Vec<f32>> {
        let se    = self.samples.get(index).ok_or(OsmpError::IndexOutOfRange(index, self.samples.len()))?;
        let bytes = self.sample_bytes(index)?;
        let nvals = bytes.len() / se.format.bytes_per_value();
        Ok(decode_bytes(bytes, se.format, nvals))
    }

    /// For `S32LE` files: zero-copy `&[i32]` slice into the mmap.
    pub fn sample_i32_ref(&self, index: usize) -> Result<Option<&[i32]>> {
        let se = self.samples.get(index).ok_or(OsmpError::IndexOutOfRange(index, self.samples.len()))?;
        if se.format != SampleFormat::S32LE { return Ok(None); }
        let bytes = self.sample_bytes(index)?;
        let ptr   = bytes.as_ptr() as *const i32;
        if bytes.as_ptr() as usize % 4 != 0 { return Ok(None); }
        Ok(Some(unsafe { std::slice::from_raw_parts(ptr, bytes.len() / 4) }))
    }

    pub fn file_size(&self) -> usize { self.mmap.len() }
}

// ── Writer ─────────────────────────────────────────────────────────────────────

/// Builds an `.osmp` container file sample-by-sample.
///
/// `capacity` reserves space for that many sample-table entries so audio data
/// can be written contiguously without re-reading.  Must be ≥ the number of
/// `add_sample` calls.
pub struct OsmpWriter {
    writer:      BufWriter<File>,
    header:      ContainerHeader,
    samples:     Vec<SampleEntry>,
    pub capacity:    u32,
    pub audio_start: u64,
    pub table_start: u64,
    pos:         u64,
}

impl OsmpWriter {
    pub fn new<P: AsRef<Path>>(path: P, header: ContainerHeader, capacity: u32) -> Result<Self> {
        Self::new_with_json(path, header, capacity, &[])
    }

    /// Create a new container with an embedded JSON zone-map blob.
    /// Bumps the container version to `VERSION_2`.
    pub fn new_with_json<P: AsRef<Path>>(
        path:     P,
        header:   ContainerHeader,
        capacity: u32,
        json:     &[u8],
    ) -> Result<Self> {
        if capacity as usize > MAX_SAMPLES {
            return Err(OsmpError::CapacityExceeded(capacity as usize, MAX_SAMPLES));
        }
        let file = OpenOptions::new().write(true).create(true).truncate(true).open(path)?;
        let mut bw = BufWriter::new(file);

        let json_len   = json.len() as u32;
        let padded_len = ContainerHeader::json_blob_padded(json_len) as usize;

        let mut hdr    = header.clone();
        hdr.num_samples = 0;
        hdr.json_len    = json_len;
        hdr.version     = if json_len > 0 { VERSION_2 } else { VERSION };
        hdr.write_to(&mut bw)?;

        // Write JSON blob + zero-pad to 64-byte boundary
        if json_len > 0 {
            bw.write_all(json)?;
            let padding = padded_len - json_len as usize;
            if padding > 0 {
                bw.write_all(&vec![0u8; padding])?;
            }
        }

        // Reserve sample table space (filled with zeros, patched in finish())
        bw.write_all(&vec![0u8; capacity as usize * SAMPLE_ENTRY_SIZE])?;

        let table_start = ContainerHeader::table_offset(json_len);
        let audio_start = ContainerHeader::audio_offset(capacity, json_len);
        Ok(Self { writer: bw, header: hdr, samples: Vec::new(), capacity, audio_start, table_start, pos: audio_start })
    }

    /// Write raw PCM data for one sample and register its metadata.
    ///
    /// `pcm_bytes` must be packed little-endian matching `entry.format`
    /// (S24LE = 3 bytes/value, S32LE = 4 bytes/value).
    pub fn add_sample(&mut self, mut entry: SampleEntry, pcm_bytes: &[u8]) -> Result<()> {
        if self.samples.len() >= self.capacity as usize {
            return Err(OsmpError::CapacityExceeded(self.samples.len() + 1, self.capacity as usize));
        }
        entry.data_offset = self.pos;
        entry.data_len    = pcm_bytes.len() as u64;
        entry.num_frames  = (pcm_bytes.len() / entry.bytes_per_frame()) as u32;

        self.writer.write_all(pcm_bytes)?;
        self.pos += pcm_bytes.len() as u64;
        self.samples.push(entry);
        Ok(())
    }

    /// Convenience wrapper: encode `f32` samples to `S32LE` or `S24LE` on the fly.
    pub fn add_sample_f32(&mut self, mut entry: SampleEntry, samples: &[f32]) -> Result<()> {
        if self.samples.len() >= self.capacity as usize {
            return Err(OsmpError::CapacityExceeded(self.samples.len() + 1, self.capacity as usize));
        }
        let pcm = encode_f32(samples, entry.format);
        entry.data_offset = self.pos;
        entry.data_len    = pcm.len() as u64;
        entry.num_frames  = (samples.len() / entry.channels as usize) as u32;
        self.writer.write_all(&pcm)?;
        self.pos += pcm.len() as u64;
        self.samples.push(entry);
        Ok(())
    }

    /// Flush audio, patch header and sample table, close the file.
    pub fn finish(mut self) -> Result<ContainerHeader> {
        self.writer.flush()?;
        self.header.num_samples = self.samples.len() as u32;

        let mut file = self.writer.into_inner().map_err(|e| e.into_error())?;

        // Patch container header at byte 0
        file.seek(SeekFrom::Start(0))?;
        self.header.write_to(&mut file)?;

        // Seek to sample table (after header + optional JSON blob)
        if self.table_start > HEADER_SIZE as u64 {
            file.seek(SeekFrom::Start(self.table_start))?;
        }

        // Patch sample entries into the reserved table area
        for se in &self.samples {
            se.write_to(&mut file)?;
        }

        file.flush()?;
        Ok(self.header)
    }
}

// ── Decode / encode helpers ────────────────────────────────────────────────────

fn decode_read<R: Read>(r: &mut R, fmt: SampleFormat, nvals: usize) -> Result<Vec<f32>> {
    let mut out = Vec::with_capacity(nvals);
    match fmt {
        SampleFormat::S24LE => {
            for _ in 0..nvals {
                let mut b = [0u8; 3];
                r.read_exact(&mut b)?;
                out.push(s24le_to_f32(b));
            }
        }
        SampleFormat::S32LE => {
            for _ in 0..nvals {
                let v = r.read_i32::<LittleEndian>()?;
                out.push(v as f32 / 2_147_483_648.0);
            }
        }
    }
    Ok(out)
}

fn decode_bytes(bytes: &[u8], fmt: SampleFormat, nvals: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(nvals);
    match fmt {
        SampleFormat::S24LE => {
            for chunk in bytes.chunks_exact(3) {
                out.push(s24le_to_f32([chunk[0], chunk[1], chunk[2]]));
            }
        }
        SampleFormat::S32LE => {
            for chunk in bytes.chunks_exact(4) {
                let v = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                out.push(v as f32 / 2_147_483_648.0);
            }
        }
    }
    out
}

#[inline]
fn s24le_to_f32(b: [u8; 3]) -> f32 {
    let raw = (b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16);
    // Sign-extend from 24 bits
    let signed = if raw & 0x80_0000 != 0 { raw | !0xFF_FFFF } else { raw };
    signed as f32 / 8_388_608.0
}

fn encode_f32(samples: &[f32], fmt: SampleFormat) -> Vec<u8> {
    match fmt {
        SampleFormat::S24LE => {
            let mut out = Vec::with_capacity(samples.len() * 3);
            for &s in samples {
                let v = (s.clamp(-1.0, 1.0) * 8_388_607.0) as i32;
                out.push((v & 0xFF) as u8);
                out.push(((v >> 8) & 0xFF) as u8);
                out.push(((v >> 16) & 0xFF) as u8);
            }
            out
        }
        SampleFormat::S32LE => {
            let mut out = Vec::with_capacity(samples.len() * 4);
            for &s in samples {
                let v = (s.clamp(-1.0, 1.0) * 2_147_483_647.0) as i32;
                out.extend_from_slice(&v.to_le_bytes());
            }
            out
        }
    }
}

// ── Convenience ───────────────────────────────────────────────────────────────

/// Byte offset of the first audio byte given `capacity` reserved entries (no JSON blob).
pub fn audio_data_offset(capacity: u32) -> u64 {
    ContainerHeader::audio_offset(capacity, 0)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn make_entry(name: &str, sr: u32, ch: u16, fmt: SampleFormat) -> SampleEntry {
        SampleEntry::new(name, sr, ch, fmt)
    }

    // ── Header round-trip ──────────────────────────────────────────────────────

    #[test]
    fn container_header_round_trip() {
        let mut h = ContainerHeader::new("DrumKit-01");
        h.num_samples = 24;
        let mut buf = Vec::new();
        h.write_to(&mut buf).unwrap();
        assert_eq!(buf.len(), HEADER_SIZE);
        let h2 = ContainerHeader::read_from(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(h2.name, "DrumKit-01");
        assert_eq!(h2.num_samples, 24);
    }

    // ── SampleEntry round-trip ─────────────────────────────────────────────────

    #[test]
    fn sample_entry_round_trip() {
        let mut se = make_entry("kick-hard", 48000, 1, SampleFormat::S32LE);
        se.data_offset = 0x0010_0000;
        se.data_len    = 0x0004_0000;
        se.num_frames  = 0x4000;
        se.velocity_lo = 64;
        se.velocity_hi = 127;
        let mut buf = Vec::new();
        se.write_to(&mut buf).unwrap();
        assert_eq!(buf.len(), SAMPLE_ENTRY_SIZE);
        let se2 = SampleEntry::read_from(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(se2.name,        "kick-hard");
        assert_eq!(se2.sample_rate, 48000);
        assert_eq!(se2.format,      SampleFormat::S32LE);
        assert_eq!(se2.data_offset, 0x0010_0000);
        assert_eq!(se2.velocity_lo, 64);
    }

    // ── S24LE encode/decode ────────────────────────────────────────────────────

    #[test]
    fn s24le_round_trip() {
        let orig: Vec<f32> = vec![-1.0, -0.5, 0.0, 0.5, 1.0 - f32::EPSILON];
        let enc  = encode_f32(&orig, SampleFormat::S24LE);
        assert_eq!(enc.len(), orig.len() * 3);
        let dec  = decode_bytes(&enc, SampleFormat::S24LE, orig.len());
        for (a, b) in orig.iter().zip(dec.iter()) {
            assert!((a - b).abs() < 2.0 / 8_388_608.0, "s24le mismatch: {} vs {}", a, b);
        }
    }

    // ── S32LE encode/decode ────────────────────────────────────────────────────

    #[test]
    fn s32le_round_trip() {
        let orig: Vec<f32> = vec![-1.0, -0.5, 0.0, 0.5, 0.999_999];
        let enc  = encode_f32(&orig, SampleFormat::S32LE);
        assert_eq!(enc.len(), orig.len() * 4);
        let dec  = decode_bytes(&enc, SampleFormat::S32LE, orig.len());
        for (a, b) in orig.iter().zip(dec.iter()) {
            assert!((a - b).abs() < 2.0 / 2_147_483_648.0, "s32le mismatch: {} vs {}", a, b);
        }
    }

    // ── Write + full read ──────────────────────────────────────────────────────

    #[test]
    fn write_and_read_multi_sample() {
        let tmp = std::env::temp_dir().join("test_container.osmp");

        // Build two samples
        let sine: Vec<f32> = (0..4800)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI / 480.0).sin())
            .collect();
        let ramp: Vec<f32> = (0..2400).map(|i| i as f32 / 2400.0 - 1.0).collect();

        let hdr = ContainerHeader::new("TestKit");
        let mut w = OsmpWriter::new(&tmp, hdr, 2).unwrap();

        w.add_sample_f32(make_entry("sine", 48000, 1, SampleFormat::S32LE), &sine).unwrap();
        w.add_sample_f32(make_entry("ramp", 44100, 1, SampleFormat::S24LE), &ramp).unwrap();
        w.finish().unwrap();

        let mut c = OsmpFile::open(&tmp).unwrap();
        assert_eq!(c.header.num_samples, 2);
        assert_eq!(c.samples[0].name, "sine");
        assert_eq!(c.samples[1].name, "ramp");
        assert_eq!(c.samples[0].sample_rate, 48000);
        assert_eq!(c.samples[1].sample_rate, 44100);

        let got_sine = c.read_sample(0).unwrap();
        assert_eq!(got_sine.len(), sine.len());
        for (a, b) in sine.iter().zip(got_sine.iter()) {
            assert!((a - b).abs() < 2.0 / 2_147_483_648.0);
        }

        let got_ramp = c.read_sample(1).unwrap();
        assert_eq!(got_ramp.len(), ramp.len());
        for (a, b) in ramp.iter().zip(got_ramp.iter()) {
            assert!((a - b).abs() < 2.0 / 8_388_608.0);
        }

        std::fs::remove_file(&tmp).ok();
    }

    // ── Stream reader ──────────────────────────────────────────────────────────

    #[test]
    fn stream_reader_chunks() {
        let tmp   = std::env::temp_dir().join("test_stream2.osmp");
        let data: Vec<f32> = (0..9600).map(|i| (i as f32) / 9600.0 - 0.5).collect();

        let hdr = ContainerHeader::new("StreamKit");
        let mut w = OsmpWriter::new(&tmp, hdr, 1).unwrap();
        w.add_sample_f32(make_entry("ramp", 48000, 1, SampleFormat::S32LE), &data).unwrap();
        w.finish().unwrap();

        let mut c      = OsmpFile::open(&tmp).unwrap();
        let mut stream = c.stream_reader(0).unwrap();
        let mut got    = Vec::new();
        while let Some(chunk) = stream.next_chunk(1024).unwrap() {
            got.extend_from_slice(&chunk);
        }
        assert_eq!(got.len(), data.len());
        for (a, b) in data.iter().zip(got.iter()) {
            assert!((a - b).abs() < 2.0 / 2_147_483_648.0);
        }

        std::fs::remove_file(&tmp).ok();
    }

    // ── Mmap reader ───────────────────────────────────────────────────────────

    #[test]
    fn mmap_reader() {
        let tmp  = std::env::temp_dir().join("test_mmap2.osmp");
        let data: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0 - 0.5).collect();

        let hdr = ContainerHeader::new("MmapKit");
        let mut w = OsmpWriter::new(&tmp, hdr, 1).unwrap();
        w.add_sample_f32(make_entry("pad", 48000, 1, SampleFormat::S32LE), &data).unwrap();
        w.finish().unwrap();

        let mm  = OsmpMmapReader::open(&tmp).unwrap();
        let got = mm.sample_f32(0).unwrap();
        assert_eq!(got.len(), data.len());
        for (a, b) in data.iter().zip(got.iter()) {
            assert!((a - b).abs() < 2.0 / 2_147_483_648.0);
        }

        std::fs::remove_file(&tmp).ok();
    }

    // ── v2 JSON blob round-trip ───────────────────────────────────────────────

    #[test]
    fn v2_json_blob_round_trip() {
        let tmp  = std::env::temp_dir().join("test_v2_json.osmp");
        let json = r#"{"name":"TestKit","zones":[]}"#;
        let data: Vec<f32> = (0..480).map(|i| i as f32 / 480.0 - 0.5).collect();

        let hdr = ContainerHeader::new("V2Kit");
        let mut w = OsmpWriter::new_with_json(&tmp, hdr, 1, json.as_bytes()).unwrap();
        w.add_sample_f32(make_entry("kick", 48000, 1, SampleFormat::S24LE), &data).unwrap();
        let fhdr = w.finish().unwrap();

        // Header should reflect v2
        assert_eq!(fhdr.version,  VERSION_2);
        assert_eq!(fhdr.json_len, json.len() as u32);

        // OsmpFile: read JSON and audio
        let mut f = OsmpFile::open(&tmp).unwrap();
        assert_eq!(f.header.version,      VERSION_2);
        assert_eq!(f.header.json_len,     json.len() as u32);
        assert_eq!(f.header.num_samples,  1);
        assert_eq!(f.samples[0].name,     "kick");

        let got_json = f.read_json().unwrap().unwrap();
        assert_eq!(got_json, json);

        let got_audio = f.read_sample(0).unwrap();
        assert_eq!(got_audio.len(), data.len());
        for (a, b) in data.iter().zip(got_audio.iter()) {
            assert!((a - b).abs() < 2.0 / 8_388_608.0);
        }

        // OsmpMmapReader: read JSON and audio
        let mm = OsmpMmapReader::open(&tmp).unwrap();
        assert_eq!(mm.header.json_len, json.len() as u32);
        assert_eq!(mm.read_json().unwrap(), json);
        assert_eq!(mm.samples[0].name, "kick");

        let got_mm = mm.sample_f32(0).unwrap();
        assert_eq!(got_mm.len(), data.len());

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn v2_table_and_audio_offsets() {
        // json_len=0  → same as v1
        assert_eq!(ContainerHeader::table_offset(0),  HEADER_SIZE as u64);
        assert_eq!(ContainerHeader::audio_offset(4, 0),
            HEADER_SIZE as u64 + 4 * SAMPLE_ENTRY_SIZE as u64);

        // json_len=100 → padded to 128
        assert_eq!(ContainerHeader::json_blob_padded(100), 128);
        assert_eq!(ContainerHeader::table_offset(100),  HEADER_SIZE as u64 + 128);

        // json_len=64 → exactly 64
        assert_eq!(ContainerHeader::json_blob_padded(64), 64);
    }

    // ── Capacity guard ────────────────────────────────────────────────────────

    #[test]
    fn capacity_exceeded() {
        let tmp = std::env::temp_dir().join("test_cap.osmp");
        let hdr = ContainerHeader::new("Cap");
        let mut w = OsmpWriter::new(&tmp, hdr, 1).unwrap();
        let e = make_entry("a", 48000, 1, SampleFormat::S32LE);
        w.add_sample_f32(e.clone(), &[0.0; 16]).unwrap();
        let err = w.add_sample_f32(e, &[0.0; 16]);
        assert!(matches!(err, Err(OsmpError::CapacityExceeded(_, _))));
        std::fs::remove_file(&tmp).ok();
    }

    // ── Real drum instrument (mmap) ────────────────────────────────────────────
    // Requires packtest/test.osmp — build it first with:
    //   osmp packjson test\Programs\raw\full\01-full.json --pack-only \
    //       --pack packtest/test.osmp --skip-missing
    // Run: cargo test mmap_drum_instrument -- --ignored --nocapture

    #[test]
    #[ignore = "requires packtest/test.osmp"]
    fn mmap_drum_instrument() {
        let osmp_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()  // crates/osmpcore → crates
            .parent().unwrap()  // crates          → project root
            .join("packtest/test.osmp");

        if !osmp_path.exists() {
            eprintln!("SKIP: {:?} not found", osmp_path);
            return;
        }

        // ── Open via mmap ─────────────────────────────────────────────────────
        let t0 = std::time::Instant::now();
        let mm = OsmpMmapReader::open(&osmp_path).expect("open mmap");
        let open_us = t0.elapsed().as_micros();

        println!("container : {} v{}  ({} samples)  open={open_us}µs",
            mm.header.name, mm.header.version, mm.samples.len());
        println!("file size : {} MB", mm.file_size() / 1_048_576);
        if mm.header.json_len > 0 {
            println!("json blob : {} B", mm.header.json_len);
        }

        assert!(mm.header.version >= 1);
        assert!(!mm.samples.is_empty(), "no samples in container");

        // ── Embedded JSON → ZoneMap ───────────────────────────────────────────
        if mm.header.json_len > 0 {
            let json_str = mm.read_json().expect("read_json failed");
            let doc: crate::sfzjson::SfzJson =
                serde_json::from_str(&json_str).expect("invalid JSON in blob");
            let map = crate::sfzjson::ZoneMap::new(&doc);
            let cc  = doc.initial_cc();

            println!("zones     : {}", doc.zones.len());

            // Sample a spread of MIDI notes
            let test_notes: &[u8] = &[36, 38, 40, 42, 44, 46, 49, 51, 56, 60];
            println!("\nzone query results:");
            for &note in test_notes {
                let srcs = map.audio_sources(note, 100, &cc);
                if !srcs.is_empty() {
                    println!("  note {:>3}  {:>2} zone(s)  pitch={:.4}  amp={:.4}  {}",
                        note, srcs.len(),
                        srcs[0].pitch_ratio, srcs[0].amplitude,
                        srcs[0].sample);
                }
            }
        }

        // ── Random mmap sample access ─────────────────────────────────────────
        let n        = mm.samples.len();
        let n_reads  = 200.min(n);
        // LCG pseudo-random for reproducibility
        let mut rng  = 0xDEAD_BEEF_u64;
        let indices: Vec<usize> = (0..n_reads).map(|_| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng >> 33) as usize % n
        }).collect();

        let t1 = std::time::Instant::now();
        let mut total_bytes  = 0usize;
        let mut total_frames = 0u64;
        for &idx in &indices {
            let bytes = mm.sample_bytes(idx).expect("sample_bytes");
            assert!(!bytes.is_empty(), "sample {idx} has no PCM data");
            total_bytes  += bytes.len();
            total_frames += mm.samples[idx].num_frames as u64;
        }
        let elapsed = t1.elapsed();

        println!("\nrandom mmap reads ({n_reads} samples):");
        println!("  data        : {} MB  ({total_frames} frames)", total_bytes / 1_048_576);
        println!("  wall time   : {:?}", elapsed);
        println!("  throughput  : {:.1} MB/s",
            total_bytes as f64 / elapsed.as_secs_f64() / 1_048_576.0);
        println!("  avg/sample  : {:?}", elapsed / n_reads as u32);

        // ── Decode a random sample to f32 and validate range ──────────────────
        let pick = indices[0];
        let f32_data = mm.sample_f32(pick).expect("sample_f32");
        assert!(!f32_data.is_empty());
        for &s in &f32_data {
            assert!(s >= -1.0 && s <= 1.0,
                "sample[{pick}] out of range: {s}");
        }
        println!("\nsample[{pick}] ({}) decoded: {} frames  peak={:.4}",
            mm.samples[pick].name,
            f32_data.len() / mm.samples[pick].channels as usize,
            f32_data.iter().cloned().fold(0.0_f32, f32::max));
    }
}

