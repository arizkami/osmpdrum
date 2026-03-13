use clap::Args;
use hound::WavReader;
use osmpcore::{ContainerHeader, OsmpWriter, SampleEntry, MAX_SAMPLES};
use std::path::Path;

#[derive(Args)]
pub struct MkosmpArgs {
    /// Input audio files (WAV, or .raw with explicit --rate/--channels/--format)
    #[arg(required = true)]
    pub files: Vec<String>,

    /// Container / kit name [default: empty]
    #[arg(short, long, default_value = "")]
    pub name: String,

    /// Output .osmp path
    #[arg(short, long)]
    pub out: String,

    /// Output PCM format: s24le | s32le [default: s24le]
    #[arg(short, long, default_value = "s24le")]
    pub format: String,

    /// Output sample rate in Hz — WAV inputs are resampled if needed [default: 48000]
    #[arg(long, default_value = "48000")]
    pub out_rate: u32,

    /// Sample rate for raw (.raw) inputs that carry no header [default: 48000]
    #[arg(short, long)]
    pub rate: Option<u32>,

    /// Channel count for .raw inputs [default: 1]
    #[arg(short = 'c', long)]
    pub channels: Option<u16>,

    /// Reserve extra capacity beyond the number of files provided
    #[arg(long, default_value = "0")]
    pub extra_capacity: u32,
}

pub fn run(args: MkosmpArgs) -> Result<(), Box<dyn std::error::Error>> {
    let default_fmt  = crate::convertaudio::parse_format(&args.format)?;
    let out_rate     = args.out_rate;
    let default_rate = args.rate.unwrap_or(out_rate);
    let default_ch   = args.channels.unwrap_or(1);

    let n = args.files.len();
    if n == 0 { return Err("no input files".into()); }

    let capacity = n as u32 + args.extra_capacity;
    if capacity as usize > MAX_SAMPLES {
        return Err(format!("capacity {} exceeds MAX_SAMPLES {}", capacity, MAX_SAMPLES).into());
    }

    let hdr = ContainerHeader::new(&args.name);
    let mut w = OsmpWriter::new(&args.out, hdr, capacity)?;

    for path_str in &args.files {
        let path = Path::new(path_str);
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .to_string_lossy()
            .into_owned();

        match ext.as_str() {
            "wav" => {
                let reader   = WavReader::open(path)?;
                let spec     = reader.spec();
                let src_rate = spec.sample_rate;
                let channels = spec.channels;
                let pcm      = crate::convertaudio::convert_wav_with_rate(reader, default_fmt, out_rate)?;
                let entry    = SampleEntry::new(&stem, out_rate, channels, default_fmt);
                w.add_sample(entry, &pcm)?;
                let resamp = if src_rate != out_rate {
                    format!(" (resampled {} → {} Hz)", src_rate, out_rate)
                } else { String::new() };
                eprintln!(
                    "  + {:24}  {} Hz  {} ch  {:?}  ({} B){}",
                    stem, out_rate, channels, default_fmt, pcm.len(), resamp
                );
            }
            _ => {
                let pcm   = std::fs::read(path)?;
                let entry = SampleEntry::new(&stem, default_rate, default_ch, default_fmt);
                w.add_sample(entry, &pcm)?;
                eprintln!(
                    "  + {:24}  {} Hz  {} ch  {:?}  ({} B)  [raw]",
                    stem, default_rate, default_ch, default_fmt, pcm.len()
                );
            }
        }
    }

    let hdr = w.finish()?;
    eprintln!("created  {}  ({} samples, capacity {})", args.out, hdr.num_samples, capacity);
    Ok(())
}
