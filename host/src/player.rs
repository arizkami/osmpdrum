use osmpcore::{
    sfzjson::{SfzJson, ZoneMap},
    OsmpMmapReader,
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

/// Maximum decoded-PCM entries kept in the cache at one time.
/// Entries that are only referenced by the cache (refcount == 1) are
/// evicted first; if the map is still over this limit the whole cache
/// is flushed so we never accumulate unbounded heap usage.
const MAX_CACHE_ENTRIES: usize = 512;

/// Maximum number of samples decoded upfront during a WarmCache request.
/// Large kits can have thousands of samples – decoding all of them at once
/// would exhaust RAM.
const MAX_WARM_ENTRIES: usize = 128;

/// A loaded `.osmp` drum instrument.
///
/// Holds the memory-mapped file, embedded JSON zone map, and a lazy
/// decode cache so each PCM blob is only decoded once per session.
pub struct OsmpPlayer {
    pub mmap:        OsmpMmapReader,
    pub doc:         SfzJson,
    pub initial_cc:  [u8; 128],
    /// Live CC state (starts from initial_cc, can be overridden via set_cc)
    pub cc_state:    Mutex<[u8; 128]>,
    /// SampleEntry.name (≤23 bytes) → sample index in mmap
    sample_index:    HashMap<String, usize>,
    /// sample index → decoded f32 PCM (Arc so callers can hold without copy)
    decode_cache:    Mutex<HashMap<usize, Arc<Vec<f32>>>>,
}

impl OsmpPlayer {
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let mmap = OsmpMmapReader::open(&path)?;

        let json_str = mmap.read_json().ok_or_else(|| {
            anyhow::anyhow!(
                "'{}' has no embedded JSON zone map (not a v2 .osmp file)",
                path.as_ref().display()
            )
        })?;

        let doc: SfzJson = serde_json::from_str(&json_str)?;
        let initial_cc = doc.initial_cc();

        // truncated entry name → index
        let mut sample_index = HashMap::with_capacity(mmap.samples.len());
        for (i, entry) in mmap.samples.iter().enumerate() {
            sample_index.insert(entry.name.clone(), i);
        }

        println!(
            "[OsmpPlayer] '{}' v{}  {} samples  {} zones  {} MB",
            mmap.header.name, mmap.header.version,
            mmap.samples.len(), doc.zones.len(),
            mmap.file_size() / 1_048_576,
        );

        Ok(Self {
            mmap,
            doc,
            cc_state: Mutex::new(initial_cc),
            initial_cc,
            sample_index,
            decode_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Resolve note+vel to matching zones; return `(amplitude, pan, Arc<pcm_f32>)` per zone.
    ///
    /// `cc` is optional — falls back to the document's default CC state.
    /// Override one CC value in the live state.
    pub fn set_cc(&self, num: u8, value: u8) {
        if let Ok(mut cc) = self.cc_state.lock() {
            cc[num as usize] = value;
        }
    }

    /// Read current CC array (snapshot).
    pub fn get_cc(&self) -> [u8; 128] {
        self.cc_state.lock().map(|g| *g).unwrap_or(self.initial_cc)
    }

    pub fn trigger(
        &self,
        note: u8,
        vel:  u8,
        cc:   Option<&[u8; 128]>,
    ) -> Vec<(f32, f32, Arc<Vec<f32>>)> {
        let live   = self.cc_state.lock().map(|g| *g).unwrap_or(self.initial_cc);
        let cc_ref: &[u8; 128] = cc.unwrap_or(&live);
        let map     = ZoneMap::new(&self.doc);
        let sources = map.audio_sources(note, vel, cc_ref);

        let mut cache = self.decode_cache.lock().unwrap_or_else(|p| p.into_inner());

        // Evict stale entries (only referenced by this cache) when over limit
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.retain(|_, v| Arc::strong_count(v) > 1);
            if cache.len() >= MAX_CACHE_ENTRIES {
                cache.clear();
                println!("[OsmpPlayer] decode_cache flushed (over {} entries)", MAX_CACHE_ENTRIES);
            }
        }

        let mut out = Vec::with_capacity(sources.len());

        for src in &sources {
            let key = truncate_name(&src.sample);
            let Some(&idx) = self.sample_index.get(&key) else { continue };

            let pcm = cache.entry(idx).or_insert_with(|| {
                Arc::new(self.mmap.sample_f32(idx).unwrap_or_default())
            }).clone();

            out.push((src.amplitude as f32, src.pan as f32, pcm));
        }
        out
    }

    /// Decode and cache the most-used samples upfront (capped to avoid OOM).
    pub fn warm_cache(&self) {
        let total = self.mmap.samples.len();
        let limit = total.min(MAX_WARM_ENTRIES);
        let mut cache = self.decode_cache.lock().unwrap_or_else(|p| p.into_inner());
        for i in 0..limit {
            cache.entry(i).or_insert_with(|| {
                Arc::new(self.mmap.sample_f32(i).unwrap_or_default())
            });
        }
        println!("[OsmpPlayer] cache warm: {}/{} samples", limit, total);
    }

    /// Release all cached decoded PCM data that is not actively playing.
    /// Entries with Arc refcount > 1 are still held by audio buffers and are kept.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.decode_cache.lock() {
            let before = cache.len();
            cache.retain(|_, v| Arc::strong_count(v) > 1);
            println!("[OsmpPlayer] clear_cache: {} → {} entries", before, cache.len());
        }
    }

    pub fn name(&self)         -> &str  { &self.mmap.header.name }
    pub fn sample_count(&self) -> usize { self.mmap.samples.len() }
    pub fn zone_count(&self)   -> usize { self.doc.zones.len() }
    pub fn file_size_mb(&self) -> usize { self.mmap.file_size() / 1_048_576 }
    pub fn json_len(&self)     -> u32   { self.mmap.header.json_len }
}

/// Truncate to the first 23 bytes — matching `SampleEntry` on-disk name width.
fn truncate_name(s: &str) -> String {
    let b = s.as_bytes();
    String::from_utf8_lossy(&b[..b.len().min(23)]).into_owned()
}

/// Interleaved stereo f32 → mono f32 by averaging pairs.
pub fn stereo_to_mono(src: &[f32]) -> Vec<f32> {
    src.chunks(2)
        .map(|c| (c[0] + c.get(1).copied().unwrap_or(0.0)) * 0.5)
        .collect()
}
