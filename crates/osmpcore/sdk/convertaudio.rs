use clap::Args;
use hound::{SampleFormat as WavFmt, WavReader};
use osmpcore::SampleFormat as OsmpFmt;
use std::{fs, io::Write, path::{Path, PathBuf}};

/// Decode any supported audio file (WAV or FLAC) to raw PCM at `dst_rate` and `out_fmt`.
/// Returns `(pcm_bytes, src_sample_rate, channels)`.
pub fn decode_audio_file(
    path:     &Path,
    out_fmt:  OsmpFmt,
    dst_rate: u32,
) -> Result<(Vec<u8>, u32, u16), Box<dyn std::error::Error>> {
    let ext = path.extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .into_owned();

    match ext.as_str() {
        "flac" => decode_flac_file(path, out_fmt, dst_rate),
        _      => {
            let reader   = WavReader::open(path)?;
            let src_rate = reader.spec().sample_rate;
            let channels = reader.spec().channels;
            let pcm      = convert_wav_with_rate(reader, out_fmt, dst_rate)?;
            Ok((pcm, src_rate, channels))
        }
    }
}

fn decode_flac_file(
    path:     &Path,
    out_fmt:  OsmpFmt,
    dst_rate: u32,
) -> Result<(Vec<u8>, u32, u16), Box<dyn std::error::Error>> {
    let mut reader = claxon::FlacReader::open(path)?;
    let info       = reader.streaminfo();
    let src_rate   = info.sample_rate;
    let channels   = info.channels as usize;
    let src_bits   = info.bits_per_sample as u16;

    // Decode interleaved samples → full-scale i32
    let raw_s32: Vec<i32> = reader
        .samples()
        .map(|s| s.map(|v| scale_to_s32(v, src_bits)))
        .collect::<Result<_, _>>()?;

    let src_frames = raw_s32.len() / channels;
    let dst_frames = if src_rate == dst_rate {
        src_frames
    } else {
        ((src_frames as u64 * dst_rate as u64) / src_rate as u64) as usize
    };

    let mut out = Vec::with_capacity(dst_frames * channels * out_fmt.bytes_per_value());

    for dst_i in 0..dst_frames {
        let src_pos = if src_rate == dst_rate {
            dst_i as f64
        } else {
            dst_i as f64 * src_rate as f64 / dst_rate as f64
        };
        let lo = src_pos as usize;
        let hi = (lo + 1).min(src_frames.saturating_sub(1));
        let t  = src_pos - lo as f64;

        for ch in 0..channels {
            let a = raw_s32[lo * channels + ch] as f64;
            let b = raw_s32[hi * channels + ch] as f64;
            let v = (a + (b - a) * t) as i32;
            push_sample(&mut out, v, 32, out_fmt);
        }
    }

    Ok((out, src_rate, channels as u16))
}

#[derive(Args)]
pub struct ConvertArgs {
    /// Input WAV file
    pub input: String,

    /// Output .raw file [default: replaces extension with .raw]
    #[arg(short, long)]
    pub out: Option<String>,

    /// Output sample format: s24le | s32le [default: s24le]
    #[arg(short, long, default_value = "s24le")]
    pub format: String,

    /// Output sample rate in Hz [default: 48000]; resamples if input differs
    #[arg(short, long, default_value = "48000")]
    pub rate: u32,
}

pub fn run(args: ConvertArgs) -> Result<(), Box<dyn std::error::Error>> {
    let fmt = parse_format(&args.format)?;

    let out_path = args.out.unwrap_or_else(|| {
        PathBuf::from(&args.input)
            .with_extension("raw")
            .to_string_lossy()
            .into_owned()
    });

    let reader    = WavReader::open(&args.input)?;
    let spec      = reader.spec();
    let src_rate  = spec.sample_rate;
    let dst_rate  = args.rate;

    eprintln!(
        "input  : {}  ({} Hz, {} ch, {}-bit {:?})",
        args.input, src_rate, spec.channels, spec.bits_per_sample, spec.sample_format
    );
    if src_rate != dst_rate {
        eprintln!("resample : {} Hz → {} Hz", src_rate, dst_rate);
    }
    eprintln!("output : {}  ({:?}, {} Hz)", out_path, fmt, dst_rate);

    let pcm = convert_wav_with_rate(reader, fmt, dst_rate)?;
    let mut f = fs::File::create(&out_path)?;
    f.write_all(&pcm)?;

    eprintln!("written  {} bytes", pcm.len());
    Ok(())
}

pub fn parse_format(s: &str) -> Result<OsmpFmt, String> {
    match s.to_ascii_lowercase().as_str() {
        "s24le" | "24" => Ok(OsmpFmt::S24LE),
        "s32le" | "32" => Ok(OsmpFmt::S32LE),
        other => Err(format!("unknown format '{}'; expected s24le or s32le", other)),
    }
}

/// Decode a WAV file and optionally resample to `dst_rate`, then encode to `out_fmt`.
///
/// Resampling uses per-channel linear interpolation.  When `dst_rate` equals
/// the source rate no interpolation is performed.
pub fn convert_wav_with_rate<R: std::io::Read>(
    reader:   WavReader<R>,
    out_fmt:  OsmpFmt,
    dst_rate: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let spec      = reader.spec();
    let src_bits  = spec.bits_per_sample;
    let src_rate  = spec.sample_rate;
    let channels  = spec.channels as usize;

    // 1. Decode all interleaved samples → i32 (full-scale S32)
    let raw_s32: Vec<i32> = match spec.sample_format {
        WavFmt::Int => reader
            .into_samples::<i32>()
            .map(|s| s.map(|v| scale_to_s32(v, src_bits)))
            .collect::<Result<_, _>>()?,
        WavFmt::Float => reader
            .into_samples::<f32>()
            .map(|s| s.map(|v| (v.clamp(-1.0, 1.0) * 2_147_483_647.0) as i32))
            .collect::<Result<_, _>>()?,
    };

    let src_frames = raw_s32.len() / channels;

    // 2. Resample per channel (linear interpolation)
    let dst_frames = if src_rate == dst_rate {
        src_frames
    } else {
        ((src_frames as u64 * dst_rate as u64) / src_rate as u64) as usize
    };

    let mut out = Vec::with_capacity(dst_frames * channels * out_fmt.bytes_per_value());

    for dst_i in 0..dst_frames {
        // fractional source position
        let src_pos = if src_rate == dst_rate {
            dst_i as f64
        } else {
            dst_i as f64 * src_rate as f64 / dst_rate as f64
        };
        let lo = src_pos as usize;
        let hi = (lo + 1).min(src_frames.saturating_sub(1));
        let t  = src_pos - lo as f64;

        for ch in 0..channels {
            let a = raw_s32[lo * channels + ch] as f64;
            let b = raw_s32[hi * channels + ch] as f64;
            let v = (a + (b - a) * t) as i32;
            push_sample(&mut out, v, 32, out_fmt);
        }
    }

    Ok(out)
}

/// Convenience wrapper — no resampling (uses source rate).
pub fn convert_wav<R: std::io::Read>(
    reader:  WavReader<R>,
    out_fmt: OsmpFmt,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let src_rate = reader.spec().sample_rate;
    convert_wav_with_rate(reader, out_fmt, src_rate)
}

#[inline]
fn push_sample(out: &mut Vec<u8>, v: i32, src_bits: u16, fmt: OsmpFmt) {
    match fmt {
        OsmpFmt::S32LE => {
            let s32 = scale_to_s32(v, src_bits);
            out.extend_from_slice(&s32.to_le_bytes());
        }
        OsmpFmt::S24LE => {
            let s24 = scale_to_s24(v, src_bits);
            out.push((s24 & 0xFF) as u8);
            out.push(((s24 >> 8) & 0xFF) as u8);
            out.push(((s24 >> 16) & 0xFF) as u8);
        }
    }
}

/// Scale an integer PCM value from `src_bits` depth to full signed-32 range.
fn scale_to_s32(v: i32, src_bits: u16) -> i32 {
    match src_bits {
        8  => (v.wrapping_sub(128)) << 24,
        16 => v << 16,
        24 => v << 8,
        32 => v,
        n  => {
            let shift = 32i32 - n as i32;
            if shift >= 0 { v << shift } else { v >> (-shift) }
        }
    }
}

/// Scale an integer PCM value from `src_bits` depth to signed-24 range.
fn scale_to_s24(v: i32, src_bits: u16) -> i32 {
    match src_bits {
        8  => (v.wrapping_sub(128)) << 16,
        16 => v << 8,
        24 => v,
        32 => v >> 8,
        n  => {
            let shift = 24i32 - n as i32;
            if shift >= 0 { v << shift } else { v >> (-shift) }
        }
    }
}
