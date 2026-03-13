use clap::Args;
use hound::WavReader;
use osmpcore::{ContainerHeader, OsmpMmapReader, OsmpWriter, SampleEntry};
use std::path::Path;

#[derive(Args)]
pub struct PackArgs {
    /// Existing .osmp container to append to
    pub container: String,

    /// Input audio file (WAV or .raw)
    pub input: String,

    /// Sample name [default: input filename stem]
    #[arg(short, long)]
    pub name: Option<String>,

    /// Sample rate — required for .raw inputs, ignored for WAV [default: 48000]
    #[arg(short, long)]
    pub rate: Option<u32>,

    /// Channel count — required for .raw inputs, ignored for WAV [default: 1]
    #[arg(short = 'c', long)]
    pub channels: Option<u16>,

    /// PCM format: s24le | s32le [default: s32le]
    #[arg(short, long, default_value = "s32le")]
    pub format: String,

    /// MIDI velocity range low [default: 0]
    #[arg(long, default_value = "0")]
    pub velocity_lo: u8,

    /// MIDI velocity range high [default: 127]
    #[arg(long, default_value = "127")]
    pub velocity_hi: u8,

    /// Mark sample as looping
    #[arg(long)]
    pub looping: bool,

    /// Loop start frame index [default: 0]
    #[arg(long, default_value = "0")]
    pub loop_start: u32,

    /// Loop end frame index [default: 0 = end of sample]
    #[arg(long, default_value = "0")]
    pub loop_end: u32,

    /// Output .osmp path [default: overwrite <container>]
    #[arg(short, long)]
    pub out: Option<String>,
}

pub fn run(args: PackArgs) -> Result<(), Box<dyn std::error::Error>> {
    let fmt      = crate::convertaudio::parse_format(&args.format)?;
    let out_path = args.out.as_deref().unwrap_or(&args.container).to_owned();

    // ── Load existing container (mmap for zero-copy raw byte access) ──────────
    let existing        = OsmpMmapReader::open(&args.container)?;
    let n_exist         = existing.samples.len();
    let container_name  = existing.header.name.clone();
    let old_samples: Vec<SampleEntry> = existing.samples.clone();

    // Collect raw PCM bytes for all existing samples into owned buffers.
    // This lets us drop the mmap before writing the output file (important
    // when out_path == container, i.e. overwrite mode).
    let old_pcm: Vec<Vec<u8>> = (0..n_exist)
        .map(|i| existing.sample_bytes(i).map(|b| b.to_vec()))
        .collect::<Result<_, _>>()?;

    drop(existing); // release mmap before potentially overwriting the file

    // ── Resolve new sample metadata ───────────────────────────────────────────
    let input_path  = Path::new(&args.input);
    let stem        = input_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let sample_name = args.name.as_deref().unwrap_or(&stem).to_owned();
    let ext = input_path
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .into_owned();

    // Decode the new sample
    let (new_entry, new_pcm): (SampleEntry, Vec<u8>) = match ext.as_str() {
        "wav" => {
            let reader = WavReader::open(input_path)?;
            let spec   = reader.spec();
            let rate   = args.rate.unwrap_or(spec.sample_rate);
            let ch     = args.channels.unwrap_or(spec.channels);
            let entry  = build_entry(&sample_name, rate, ch, fmt, &args);
            let pcm    = crate::convertaudio::convert_wav(reader, fmt)?;
            (entry, pcm)
        }
        _ => {
            let rate  = args.rate.unwrap_or(48_000);
            let ch    = args.channels.unwrap_or(1);
            let entry = build_entry(&sample_name, rate, ch, fmt, &args);
            let pcm   = std::fs::read(input_path)?;
            (entry, pcm)
        }
    };

    // ── Rebuild container with capacity = old + 1 ────────────────────────────
    let capacity = (n_exist + 1) as u32;
    let new_hdr  = ContainerHeader::new(&container_name);
    let mut w    = OsmpWriter::new(&out_path, new_hdr, capacity)?;

    for (se, raw) in old_samples.iter().zip(old_pcm.iter()) {
        w.add_sample(se.clone(), raw)?;
    }
    w.add_sample(new_entry, &new_pcm)?;

    let hdr = w.finish()?;
    eprintln!(
        "container: {}  ({} → {} samples)",
        out_path, n_exist, hdr.num_samples
    );
    Ok(())
}

fn build_entry(
    name:     &str,
    rate:     u32,
    channels: u16,
    fmt:      osmpcore::SampleFormat,
    args:     &PackArgs,
) -> SampleEntry {
    let mut e    = SampleEntry::new(name, rate, channels, fmt);
    e.velocity_lo = args.velocity_lo;
    e.velocity_hi = args.velocity_hi;
    e.loop_start  = args.loop_start;
    e.loop_end    = args.loop_end;
    if args.looping {
        e.flags |= osmpcore::flags::LOOPING as u16;
    }
    e
}
