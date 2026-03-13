mod bench;
mod convertaudio;
mod session;
mod loadjson;
mod mkosmp;
mod pack;
mod packjson;

use clap::{Parser, Subcommand};
use osmpcore::OsmpFile;
use std::process;

#[derive(Parser)]
#[command(
    name    = "osmp",
    about   = "OSMP container SDK — build, pack, convert, and inspect .osmp files",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Convert a WAV file to raw PCM (.raw)
    Convert(convertaudio::ConvertArgs),

    /// Create a new .osmp container from one or more audio files
    Mkosmp(mkosmp::MkosmpArgs),

    /// Append one sample to an existing .osmp container (rebuilds the file)
    Pack(pack::PackArgs),

    /// Load and query a sfz2json .json instrument map
    Loadjson(loadjson::LoadjsonArgs),

    /// Convert all audio referenced in a zone map JSON and pack into .osmp
    Packjson(packjson::PackjsonArgs),

    /// Benchmark mmap access, zone queries, and f32 decode on an .osmp file
    Bench(bench::BenchArgs),

    /// Print metadata of an .osmp container
    Info {
        /// Path to the .osmp file
        file: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result: Result<(), Box<dyn std::error::Error>> = match cli.cmd {
        Cmd::Convert(a)    => convertaudio::run(a),
        Cmd::Mkosmp(a)     => mkosmp::run(a),
        Cmd::Pack(a)       => pack::run(a),
        Cmd::Loadjson(a)   => loadjson::run(a),
        Cmd::Packjson(a)   => packjson::run(a),
        Cmd::Bench(a)      => bench::run(a),
        Cmd::Info { file } => run_info(&file).map_err(|e| Box::new(e) as _),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn run_info(path: &str) -> osmpcore::Result<()> {
    let c = OsmpFile::open(path)?;

    println!("container : {}", if c.header.name.is_empty() { "<unnamed>" } else { &c.header.name });
    println!("version   : {}", c.header.version);
    println!("samples   : {}", c.header.num_samples);
    if c.header.json_len > 0 {
        println!("json blob : {} B (embedded zone map)", c.header.json_len);
    }
    println!();
    println!(
        "  {:<4}  {:<24}  {:>7}  {:>4}  {:>5}  {:>10}  {:>5}  {:>5}  vel",
        "#", "name", "rate", "ch", "fmt", "frames", "l.start", "l.end"
    );
    println!("  {}", "-".repeat(82));

    for (i, s) in c.samples.iter().enumerate() {
        let name  = if s.name.is_empty() { "<unnamed>" } else { &s.name };
        let fmt   = format!("{:?}", s.format);
        let loopt = if s.is_looping() { "L" } else { " " };
        println!(
            "  {:<4}  {:<24}  {:>7}  {:>4}  {:>5}  {:>10}  {:>7}  {:>5}  {}-{}  {}",
            i, name, s.sample_rate, s.channels, fmt, s.num_frames,
            s.loop_start, s.loop_end, s.velocity_lo, s.velocity_hi, loopt
        );
    }
    Ok(())
}
