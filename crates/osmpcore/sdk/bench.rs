use clap::Args;
use osmpcore::{sfzjson::ZoneMap, OsmpMmapReader};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

#[derive(Args)]
pub struct BenchArgs {
    /// Path to the .osmp file — omit to reload the last session
    pub file: Option<String>,

    /// Number of random sample reads for the throughput benchmark [default: 200]
    #[arg(short, long, default_value = "200")]
    pub samples: usize,

    /// MIDI notes to query via the embedded zone map (comma-separated)
    #[arg(long, default_value = "36,38,40,42,44,46,49,51,56,60")]
    pub notes: String,

    /// MIDI velocity for zone queries [default: 100]
    #[arg(long, default_value = "100")]
    pub vel: u8,

    /// Keep running random-read passes until Ctrl+C
    #[arg(short = 'l', long)]
    pub loop_mode: bool,
}

pub fn run(args: BenchArgs) -> Result<(), Box<dyn std::error::Error>> {
    // ── Resolve file path from arg or last session ────────────────────────────
    let mut session = crate::session::Session::load();

    let file_path = match args.file.as_deref() {
        Some(p) => p.to_owned(),
        None    => session.last_file.clone().ok_or(
            "no file specified and no session found — run: osmp bench <file.osmp>"
        )?,
    };

    // ── Open via mmap ─────────────────────────────────────────────────────────
    let t0 = Instant::now();
    let mm = OsmpMmapReader::open(&file_path)?;

    // Save session after successful open
    session.last_file = Some(file_path.clone());
    session.save();

    let file_path = file_path.as_str();
    let open_us = t0.elapsed().as_micros();

    let n = mm.samples.len();

    println!("container : {}", mm.header.name);
    println!("version   : {}", mm.header.version);
    println!("samples   : {n}");
    println!("file size : {} MB", mm.file_size() / 1_048_576);
    if mm.header.json_len > 0 {
        println!("json blob : {} B", mm.header.json_len);
    }
    println!("open time : {open_us} µs");

    if n == 0 {
        return Ok(());
    }

    // ── Embedded JSON zone queries ────────────────────────────────────────────
    if mm.header.json_len > 0 {
        let json_str = mm.read_json().ok_or("container has no JSON blob")?;
        let doc: osmpcore::sfzjson::SfzJson = serde_json::from_str(&json_str)?;
        let map = ZoneMap::new(&doc);
        let cc  = doc.initial_cc();

        println!("\nzone map  : {} zones  |  {} CC labels",
            doc.zones.len(), doc.cc_labels.len());

        let notes: Vec<u8> = args.notes.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        println!("\n  {:<5}  {:<5}  {:<8}  {:<8}  sample", "note", "srcs", "pitch", "amp");
        println!("  {}", "-".repeat(60));

        for note in &notes {
            let srcs = map.audio_sources(*note, args.vel, &cc);
            if srcs.is_empty() {
                println!("  {:<5}  (no zones)", note);
            } else {
                for src in &srcs {
                    println!("  {:<5}  {:<5}  {:<8.4}  {:<8.4}  {}",
                        note, srcs.len(), src.pitch_ratio, src.amplitude, src.sample);
                }
            }
        }
    }

    // ── Ctrl+C handler (shared across single + loop mode) ────────────────────
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    let n_reads = args.samples.min(n);

    if args.loop_mode {
        println!("\nlooping  ({n_reads} reads/pass) — press Ctrl+C to stop\n");
        println!("  {:<6}  {:<10}  {:<12}  {:<12}  {:<10}",
            "pass", "MB read", "wall time", "MB/s", "avg/sample");
        println!("  {}", "-".repeat(58));

        let mut pass = 0u64;
        let mut rng  = 0xDEAD_BEEF_u64;

        while running.load(Ordering::SeqCst) {
            pass += 1;
            let t = Instant::now();
            let mut bytes_this_pass = 0usize;

            for _ in 0..n_reads {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let idx   = (rng >> 33) as usize % n;
                let bytes = mm.sample_bytes(idx)?;
                bytes_this_pass += bytes.len();
            }

            let elapsed = t.elapsed();
            let mb = bytes_this_pass as f64 / 1_048_576.0;
            let throughput = if elapsed.as_nanos() > 0 {
                mb / elapsed.as_secs_f64()
            } else { 0.0 };

            println!("  {:<6}  {:<10.2}  {:<12?}  {:<12.1}  {:?}",
                pass, mb, elapsed, throughput,
                elapsed / n_reads as u32);
        }

        println!("\nstopped after {pass} pass(es)");

    } else {
        // ── Single-pass bench ─────────────────────────────────────────────────
        let mut rng = 0xDEAD_BEEF_u64;
        let indices: Vec<usize> = (0..n_reads).map(|_| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng >> 33) as usize % n
        }).collect();

        println!("\nrandom mmap reads ({n_reads} × sample_bytes):");
        let t1 = Instant::now();
        let mut total_bytes  = 0usize;
        let mut total_frames = 0u64;
        for &idx in &indices {
            let bytes = mm.sample_bytes(idx)?;
            total_bytes  += bytes.len();
            total_frames += mm.samples[idx].num_frames as u64;
        }
        let elapsed = t1.elapsed();

        println!("  data read   : {} MB  ({total_frames} frames)",
            total_bytes / 1_048_576);
        println!("  wall time   : {:?}", elapsed);
        if elapsed.as_nanos() > 0 {
            println!("  throughput  : {:.1} MB/s",
                total_bytes as f64 / elapsed.as_secs_f64() / 1_048_576.0);
        }
        println!("  avg/sample  : {:?}", elapsed / n_reads as u32);

        // ── f32 decode of first picked sample ─────────────────────────────────
        let pick  = indices[0];
        let entry = &mm.samples[pick];
        println!("\nsample[{pick}] \"{}\"  rate={}  ch={}  fmt={:?}  frames={}",
            entry.name, entry.sample_rate, entry.channels,
            entry.format, entry.num_frames);

        let t2     = Instant::now();
        let f32s   = mm.sample_f32(pick)?;
        let dec_us = t2.elapsed().as_micros();

        let peak = f32s.iter().cloned().fold(0.0_f32, |a, b| a.max(b.abs()));
        println!("  decoded     : {} values  peak={peak:.4}  ({dec_us} µs)", f32s.len());
    }

    Ok(())
}
