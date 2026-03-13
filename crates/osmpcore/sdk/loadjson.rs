use clap::Args;
use osmpcore::sfzjson::{AudioSource, SfzJson, ZoneMap};

#[derive(Args)]
pub struct LoadjsonArgs {
    /// Path to the .json file produced by sfz2json.py
    pub file: String,

    /// Query zones: note=<0-127> vel=<0-127> [ccN=<0-127> ...]
    ///
    /// Example: --query note=36 vel=100 cc70=80
    #[arg(long, num_args = 1.., value_name = "KEY=VAL")]
    pub query: Vec<String>,

    /// Print stats summary only (zone count, CC labels, note distribution)
    #[arg(long)]
    pub stats: bool,

    /// Show full zone details in query results (default: summary line per zone)
    #[arg(long)]
    pub verbose: bool,
}

pub fn run(args: LoadjsonArgs) -> Result<(), Box<dyn std::error::Error>> {
    let doc = SfzJson::from_file(&args.file)?;

    if args.query.is_empty() || args.stats {
        print_stats(&doc);
    }

    if !args.query.is_empty() {
        let (note, vel, cc) = parse_query(&args.query, &doc)?;
        let map     = ZoneMap::new(&doc);
        let sources = map.audio_sources(note, vel, &cc);

        println!();
        println!("query  note={note}  vel={vel}  →  {} source(s) matched", sources.len());
        println!();

        for (i, src) in sources.iter().enumerate() {
            if args.verbose {
                print_audio_source(i, src);
            } else {
                println!(
                    "  [{i}]  seq={}/{}  pitch={:.4}  amp={:.4}  {}",
                    src.seq_position, src.seq_length,
                    src.pitch_ratio, src.amplitude,
                    src.sample,
                );
            }
        }
    }

    Ok(())
}

fn print_audio_source(i: usize, src: &AudioSource) {
    println!("  [{i}] sample      : {}", src.sample);
    println!("       pitch_ratio : {:.6}  ({:+.2} semitones)",
        src.pitch_ratio,
        src.pitch_ratio.log2() * 12.0);
    println!("       amplitude   : {:.4}  ({:.1} dBFS)",
        src.amplitude,
        if src.amplitude > 0.0 { 20.0 * src.amplitude.log10() } else { f64::NEG_INFINITY });
    println!("       pan         : {:+.4}  vol_db={:.1}", src.pan, src.volume_db);
    println!("       loop        : {}  {}-{}", src.loop_mode, src.loop_start, src.loop_end);
    println!("       seq         : {}/{}", src.seq_position, src.seq_length);
    println!("       group/off_by: {}/{}  off_mode={}", src.group_id, src.off_by, src.off_mode);
    println!("       trigger     : {}", src.trigger);
    println!("       ampeg       : A{:.3} H{:.3} D{:.3} S{:.1} R{:.3}",
        src.ampeg.attack, src.ampeg.hold, src.ampeg.decay,
        src.ampeg.sustain, src.ampeg.release);
    println!();
}

fn print_stats(doc: &SfzJson) {
    println!("name       : {}", doc.name);
    println!("zones      : {}", doc.zones.len());
    println!("cc_labels  : {}", doc.cc_labels.len());
    println!("cc_init    : {}", doc.cc_init.len());
    println!("defines    : {}", doc.defines.len());

    if !doc.cc_labels.is_empty() {
        println!();
        println!("  CC labels:");
        let mut labels: Vec<_> = doc.cc_labels.iter().collect();
        labels.sort_by_key(|(k, _)| k.parse::<u32>().unwrap_or(0));
        for (n, label) in &labels {
            let init = doc.cc_init.get(*n).copied().unwrap_or(0);
            println!("    CC{:<4}  init={:3}  {}", n, init, label);
        }
    }

    // Note distribution (chromatic)
    let map    = ZoneMap::new(doc);
    let counts = map.zone_count_per_note();
    let active: Vec<(usize, usize)> = counts.iter().enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(n, c)| (n, *c))
        .collect();

    if !active.is_empty() {
        println!();
        println!("  Note distribution ({} active notes):", active.len());
        for (n, c) in &active {
            let name = midi_name(*n as u8);
            println!("    {:>3} {:>3}  {:>4} zone(s)  {}",
                n, name, c, bar(*c, 40));
        }
    }
}

fn parse_query(
    args: &[String],
    doc:  &SfzJson,
) -> Result<(u8, u8, [u8; 128]), Box<dyn std::error::Error>> {
    let mut note: u8 = 60;
    let mut vel:  u8 = 100;
    let mut cc          = doc.initial_cc();

    for arg in args {
        let (k, v) = arg.split_once('=')
            .ok_or_else(|| format!("expected KEY=VAL, got: {arg}"))?;
        let k = k.trim().to_lowercase();
        let v: u8 = v.trim().parse()
            .map_err(|_| format!("expected integer 0-127 for '{k}', got: {v}"))?;

        match k.as_str() {
            "note" => note = v,
            "vel"  => vel  = v,
            other => {
                if let Some(n_str) = other.strip_prefix("cc") {
                    let n: usize = n_str.parse()
                        .map_err(|_| format!("invalid CC number: {n_str}"))?;
                    if n < 128 {
                        cc[n] = v;
                    }
                } else {
                    return Err(format!("unknown query key '{k}' (use note, vel, ccN)").into());
                }
            }
        }
    }

    Ok((note, vel, cc))
}

fn midi_name(n: u8) -> String {
    const NAMES: &[&str] = &["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
    let oct  = (n / 12) as i32 - 1;
    let name = NAMES[(n % 12) as usize];
    format!("{name}{oct}")
}

fn bar(n: usize, max_width: usize) -> String {
    let width = (n.min(200) * max_width / 200).max(1);
    "█".repeat(width)
}
