use clap::Args;
use osmpcore::{sfzjson::SfzJson, ContainerHeader, OsmpWriter, SampleEntry};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Args)]
pub struct PackjsonArgs {
    /// Input zone map JSON (from sfz2json.py) — sample paths reference FLAC/WAV
    pub json: String,

    /// Output directory for converted .raw PCM files
    /// (not required when using --pack-only)
    #[arg(long)]
    pub out_dir: Option<String>,

    /// Output JSON path with updated sample paths pointing to .raw files
    /// [default: <out-dir>/<input-stem>.json]
    #[arg(long)]
    pub out_json: Option<String>,

    /// Audio root for resolving relative FLAC/WAV paths
    /// [default: directory of the input JSON]
    #[arg(long)]
    pub root: Option<String>,

    /// Output PCM format: s24le | s32le [default: s24le]
    #[arg(short, long, default_value = "s24le")]
    pub format: String,

    /// Output sample rate in Hz [default: 48000]
    #[arg(long, default_value = "48000")]
    pub rate: u32,

    /// Skip missing audio files instead of aborting
    #[arg(long)]
    pub skip_missing: bool,

    /// Also pack everything into a single .osmp container (v2) with embedded JSON
    #[arg(long, value_name = "PATH")]
    pub pack: Option<String>,

    /// Skip conversion — read existing .raw files referenced in the JSON and pack
    /// directly into --pack output. Does not need --out-dir.
    #[arg(long)]
    pub pack_only: bool,
}

pub fn run(args: PackjsonArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.pack_only {
        return run_pack_only(&args);
    }

    let fmt      = crate::convertaudio::parse_format(&args.format)?;
    let dst_rate = args.rate;

    let json_path  = PathBuf::from(&args.json);
    let json_dir   = json_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let audio_root: PathBuf = args.root
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| json_dir.clone());

    let out_dir: PathBuf = match &args.out_dir {
        Some(d) => PathBuf::from(d),
        None    => return Err("--out-dir is required unless --pack-only is set".into()),
    };
    fs::create_dir_all(&out_dir)?;

    let stem = json_path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let out_json_path: PathBuf = args.out_json
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.join(format!("{stem}.json")));

    eprintln!("loading  {}", args.json);
    let mut doc = SfzJson::from_file(&json_path)?;
    eprintln!("         {} zones  |  {} unique samples",
        doc.zones.len(),
        doc.zones.iter().map(|z| &z.sample).collect::<HashSet<_>>().len()
    );
    eprintln!("out-dir  {}", out_dir.display());

    // Build a map: original FLAC path → new .raw relative path
    // Process unique files only; skip or error on missing
    let unique_paths: Vec<String> = {
        let mut seen  = HashSet::new();
        let mut order = Vec::new();
        for z in &doc.zones {
            if seen.insert(z.sample.clone()) {
                order.push(z.sample.clone());
            }
        }
        order
    };

    let total = unique_paths.len();
    eprintln!("converting {} unique audio files → {:?} @ {} Hz", total, fmt, dst_rate);

    // raw_map: original sample path → new relative .raw path (relative to out_dir)
    let mut raw_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::with_capacity(total);

    let mut converted = 0usize;
    let mut skipped   = 0usize;

    for (i, rel_path) in unique_paths.iter().enumerate() {
        let src_abs = audio_root.join(rel_path.replace('\\', "/"));

        if !src_abs.exists() {
            if args.skip_missing {
                eprintln!("  [skip] {}", rel_path);
                skipped += 1;
                continue;
            }
            return Err(format!(
                "not found: {}\n  (use --skip-missing to continue)",
                src_abs.display()
            ).into());
        }

        // Derive output .raw filename: flatten the path into a safe stem
        // e.g. "../Samples/kick_24/kick/k_vl1_rr1.flac" → "kick_24__kick__k_vl1_rr1.raw"
        let flat_stem = flatten_path(rel_path);
        let raw_name  = format!("{flat_stem}.raw");
        let raw_abs   = out_dir.join(&raw_name);

        let (pcm, src_rate, channels) =
            crate::convertaudio::decode_audio_file(&src_abs, fmt, dst_rate)?;

        let frames = pcm.len() / channels as usize / fmt.bytes_per_value();

        let mut f = fs::File::create(&raw_abs)?;
        f.write_all(&pcm)?;

        let resamp = if src_rate != dst_rate {
            format!("  [{src_rate}→{dst_rate}Hz]")
        } else {
            String::new()
        };

        eprintln!(
            "  [{:>4}/{total}]  {} ch  {frames:>7} frames  {} B{resamp}  {raw_name}",
            i + 1, channels, pcm.len()
        );

        raw_map.insert(rel_path.clone(), raw_name);
        converted += 1;
    }

    // Rewrite every zone's sample path to the new .raw filename
    for zone in &mut doc.zones {
        if let Some(new_path) = raw_map.get(&zone.sample) {
            zone.sample = new_path.clone();
        }
    }

    // Serialise updated JSON
    let json_out = serde_json::to_string_pretty(&doc)?;
    fs::write(&out_json_path, &json_out)?;

    eprintln!();
    eprintln!("converted  {converted} files  ({skipped} skipped)");
    eprintln!("raw files  {}", out_dir.display());
    eprintln!("new json   {}", out_json_path.display());

    // ── Optional: pack everything into a single .osmp v2 container ──────────
    if let Some(pack_path) = &args.pack {
        pack_osmp(
            pack_path,
            &json_out,
            &raw_map,
            &out_dir,
            fmt,
            dst_rate,
        )?;
    }

    Ok(())
}

/// Pack-only mode: JSON already has .raw paths → read them directly, no conversion.
fn run_pack_only(args: &PackjsonArgs) -> Result<(), Box<dyn std::error::Error>> {
    let pack_path = args.pack.as_deref()
        .ok_or("--pack <output.osmp> is required with --pack-only")?;

    let fmt      = crate::convertaudio::parse_format(&args.format)?;
    let dst_rate = args.rate;

    let json_path = PathBuf::from(&args.json);
    let json_dir  = json_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    // Raw files are resolved relative to --root, else JSON directory
    let raw_root: PathBuf = args.root
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| json_dir.clone());

    eprintln!("loading  {}", args.json);
    let doc = SfzJson::from_file(&json_path)?;

    let unique_paths: Vec<String> = {
        let mut seen  = HashSet::new();
        let mut order = Vec::new();
        for z in &doc.zones {
            if seen.insert(z.sample.clone()) {
                order.push(z.sample.clone());
            }
        }
        order
    };

    let total = unique_paths.len();
    eprintln!("         {} zones  |  {} unique .raw files", doc.zones.len(), total);
    eprintln!("root     {}", raw_root.display());

    let json_str  = serde_json::to_string_pretty(&doc)?;
    let kit_name  = json_path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let hdr       = ContainerHeader::new(&kit_name);
    let capacity  = total as u32;
    let mut w     = OsmpWriter::new_with_json(pack_path, hdr, capacity, json_str.as_bytes())?;

    eprintln!("packing  {} → {} samples", pack_path, total);

    let mut packed  = 0usize;
    let mut skipped = 0usize;

    for (i, rel_path) in unique_paths.iter().enumerate() {
        let abs_path = raw_root.join(rel_path.replace('\\', "/"));

        if !abs_path.exists() {
            if args.skip_missing {
                eprintln!("  [skip] {}", rel_path);
                skipped += 1;
                continue;
            }
            return Err(format!(
                "not found: {}\n  (use --skip-missing to continue)",
                abs_path.display()
            ).into());
        }

        let pcm    = fs::read(&abs_path)?;
        let frames = pcm.len() / fmt.bytes_per_value();
        let entry  = SampleEntry::new(rel_path.as_str(), dst_rate, 2, fmt);
        w.add_sample(entry, &pcm)?;

        eprintln!("  [{:>4}/{total}]  {frames:>7} frames  {rel_path}", i + 1);
        packed += 1;
    }

    let final_hdr = w.finish()?;
    eprintln!();
    eprintln!("packed   {}  ({} samples, {} skipped, json={} B)",
        pack_path, packed, skipped, final_hdr.json_len);

    Ok(())
}

fn pack_osmp(
    pack_path: &str,
    json_str:  &str,
    raw_map:   &std::collections::HashMap<String, String>,
    out_dir:   &Path,
    fmt:       osmpcore::SampleFormat,
    dst_rate:  u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let capacity = raw_map.len() as u32;
    // Preserve insertion order: collect (raw_name → raw_path) sorted by raw_name
    let mut entries: Vec<(&String, &String)> = raw_map.iter().collect();
    entries.sort_by_key(|(_, raw)| raw.as_str());

    let kit_name = Path::new(pack_path)
        .file_stem().unwrap_or_default()
        .to_string_lossy().into_owned();

    let hdr = ContainerHeader::new(&kit_name);
    let mut w = OsmpWriter::new_with_json(
        pack_path,
        hdr,
        capacity,
        json_str.as_bytes(),
    )?;

    let total = entries.len();
    eprintln!("packing  {} → {} samples", pack_path, total);

    for (i, (_orig, raw_name)) in entries.iter().enumerate() {
        let raw_abs = out_dir.join(raw_name);
        let pcm     = fs::read(&raw_abs)?;
        let frames  = pcm.len() / fmt.bytes_per_value();
        let entry   = SampleEntry::new(raw_name.as_str(), dst_rate, 2, fmt);
        w.add_sample(entry, &pcm)?;
        eprintln!("  [{:>4}/{total}]  {frames:>7} frames  {raw_name}", i + 1);
    }

    let final_hdr = w.finish()?;
    eprintln!();
    eprintln!("packed   {}  ({} samples, json={} B)",
        pack_path, final_hdr.num_samples, final_hdr.json_len);
    Ok(())
}

/// Flatten a relative path into a single safe filename stem.
/// "../Samples/kick_24/kick/k_vl1_rr1.flac" → "kick_24__kick__k_vl1_rr1"
fn flatten_path(rel: &str) -> String {
    let p = PathBuf::from(rel.replace('\\', "/"));
    // Drop leading ".." components and collect meaningful parts
    let parts: Vec<String> = p.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    // Remove extension from last component
    let mut parts = parts;
    if let Some(last) = parts.last_mut() {
        if let Some(stem) = Path::new(last.as_str()).file_stem() {
            *last = stem.to_string_lossy().into_owned();
        }
    }

    // Join with __ to keep names readable and unique
    parts.join("__")
}
