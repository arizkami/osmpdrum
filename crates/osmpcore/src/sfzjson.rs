//! SFZ-JSON instrument map types.
//!
//! This module provides Rust types for the JSON format emitted by `sfz2json.py`
//! and a [`ZoneMap`] query helper for real-time note + velocity + CC lookup.
//!
//! # Quick start
//! ```no_run
//! use osmpcore::sfzjson::{SfzJson, ZoneMap};
//!
//! let text   = std::fs::read_to_string("instrument.json").unwrap();
//! let doc: SfzJson = serde_json::from_str(&text).unwrap();
//! let map    = ZoneMap::new(&doc);
//! let mut cc = [0u8; 128];
//! cc[70] = 100; // kick mic on
//! let hits = map.query(36, 100, &cc); // note 36, vel 100
//! for z in hits { println!("{}", z.sample); }
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Helpers for serde defaults ─────────────────────────────────────────────────

fn d_127()        -> u8     { 127 }
fn d_60()         -> u8     { 60  }
fn d_one_u8()     -> u8     { 1   }
fn d_no_loop()    -> String { "no_loop".into() }
fn d_attack()     -> String { "attack".into()  }
fn d_fast()       -> String { "fast".into()    }
fn d_100f()       -> f64    { 100.0 }
fn d_001f()       -> f64    { 0.001 }
fn d_005f()       -> f64    { 0.05  }

// ── Amp envelope ───────────────────────────────────────────────────────────────

/// AHDSR envelope inherited from the SFZ `ampeg_*` opcodes.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Ampeg {
    #[serde(default = "d_001f")]
    pub attack:  f64,
    #[serde(default)]
    pub hold:    f64,
    #[serde(default)]
    pub decay:   f64,
    #[serde(default = "d_100f")]
    pub sustain: f64,
    #[serde(default = "d_005f")]
    pub release: f64,
}

impl Default for Ampeg {
    fn default() -> Self {
        Self { attack: 0.001, hold: 0.0, decay: 0.0, sustain: 100.0, release: 0.05 }
    }
}

// ── Zone ───────────────────────────────────────────────────────────────────────

/// A single sample zone with its full mapping context.
///
/// All inherited SFZ opcodes (`global → master → group → region`) have been
/// merged into this flat record by `sfz2json.py`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Zone {
    /// Sample path relative to the original SFZ file.
    pub sample: String,

    #[serde(default)]
    pub lo_key:  u8,
    #[serde(default = "d_127")]
    pub hi_key:  u8,
    #[serde(default = "d_60")]
    pub root_key: u8,

    #[serde(default)]
    pub lo_vel: u8,
    #[serde(default = "d_127")]
    pub hi_vel: u8,

    #[serde(default = "d_no_loop")]
    pub loop_mode:  String,
    #[serde(default)]
    pub loop_start: u32,
    #[serde(default)]
    pub loop_end:   u32,

    #[serde(default = "d_one_u8")]
    pub seq_length:   u8,
    #[serde(default = "d_one_u8")]
    pub seq_position: u8,

    #[serde(default = "d_attack")]
    pub trigger:  String,
    #[serde(default)]
    pub group_id: u32,
    #[serde(default)]
    pub off_by:   u32,
    #[serde(default = "d_fast")]
    pub off_mode: String,

    #[serde(default)]
    pub tune_cents: i32,
    #[serde(default)]
    pub transpose:  i32,
    #[serde(default)]
    pub volume_db:  f64,
    #[serde(default = "d_100f")]
    pub amplitude:  f64,
    #[serde(default)]
    pub pan:        f64,

    #[serde(default)]
    pub ampeg: Ampeg,

    /// CC range conditions: `{ "70": [1, 127] }` means CC70 must be in 1..=127.
    #[serde(default)]
    pub cc_conditions: HashMap<String, [u8; 2]>,

    /// CC modulation amounts: `{ "amplitude_cc70": 100, "tune_cc75": 1200 }`.
    #[serde(default)]
    pub cc_mods: HashMap<String, f64>,

    /// Velocity curve breakpoints: `{ "9": 1.0, "18": 1.0, ... }`.
    #[serde(default)]
    pub amp_velcurve: HashMap<String, f64>,
}

impl Zone {
    /// Returns true when `note` and `vel` are inside this zone's key/vel range.
    /// Does **not** check CC conditions (use [`ZoneMap::query`] for that).
    pub fn matches_note_vel(&self, note: u8, vel: u8) -> bool {
        note >= self.lo_key && note <= self.hi_key
            && vel >= self.lo_vel && vel <= self.hi_vel
    }

    /// Returns `true` when all CC conditions are satisfied by `cc`.
    pub fn cc_ok(&self, cc: &[u8; 128]) -> bool {
        self.cc_conditions.iter().all(|(k, &[lo, hi])| {
            k.parse::<usize>()
                .map(|n| n < 128 && cc[n] >= lo && cc[n] <= hi)
                .unwrap_or(true)
        })
    }

    /// Effective amplitude after applying initial CC values (0–1 scale).
    ///
    /// Uses the amplitude field as a base (0–100 → 0–1) and optionally
    /// scales by the active CC value for `amplitude_ccN` modulation.
    pub fn amplitude_with_cc(&self, cc: &[u8; 128]) -> f64 {
        let mut amp = self.amplitude / 100.0;
        for (k, &scale) in &self.cc_mods {
            if let Some(n_str) = k.strip_prefix("amplitude_cc") {
                if let Ok(n) = n_str.parse::<usize>() {
                    if n < 128 {
                        amp *= (cc[n] as f64 / 127.0) * (scale / 100.0);
                    }
                }
            }
        }
        amp.clamp(0.0, 1.0)
    }
}

// ── Top-level document ─────────────────────────────────────────────────────────

/// Top-level structure produced by `sfz2json.py`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SfzJson {
    /// Instrument name (SFZ file stem).
    pub name: String,

    /// Human-readable CC labels: `{ "70": "Kick mic", ... }`.
    #[serde(default)]
    pub cc_labels: HashMap<String, String>,

    /// Initial CC values from `set_ccN` opcodes in the SFZ control section.
    #[serde(default)]
    pub cc_init: HashMap<String, u8>,

    /// `$define` variable table from the SFZ (may be omitted with `--no-defines`).
    #[serde(default)]
    pub defines: HashMap<String, String>,

    /// All zones, fully merged (global < master < group < region).
    pub zones: Vec<Zone>,
}

impl SfzJson {
    /// Load from a JSON file path.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> crate::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| crate::OsmpError::Io(e))?;
        serde_json::from_str(&text)
            .map_err(|e| crate::OsmpError::InvalidJson(e.to_string()))
    }

    /// Build a CC state array pre-initialised from `cc_init` values.
    pub fn initial_cc(&self) -> [u8; 128] {
        let mut cc = [0u8; 128];
        for (k, &v) in &self.cc_init {
            if let Ok(n) = k.parse::<usize>() {
                if n < 128 {
                    cc[n] = v;
                }
            }
        }
        cc
    }
}

// ── ZoneMap ────────────────────────────────────────────────────────────────────

/// Zone lookup helper — borrows an [`SfzJson`] document and answers queries.
pub struct ZoneMap<'a> {
    pub doc: &'a SfzJson,
}

impl<'a> ZoneMap<'a> {
    pub fn new(doc: &'a SfzJson) -> Self {
        Self { doc }
    }

    /// Return all zones matching `note`, `vel`, and the given CC state.
    ///
    /// A zone matches when:
    /// - `lo_key <= note <= hi_key`
    /// - `lo_vel <= vel  <= hi_vel`
    /// - Every `cc_condition[n] = [lo, hi]` satisfied by `cc[n]`
    pub fn query(&self, note: u8, vel: u8, cc: &[u8; 128]) -> Vec<&'a Zone> {
        self.doc.zones.iter()
            .filter(|z| z.matches_note_vel(note, vel) && z.cc_ok(cc))
            .collect()
    }

    /// Return all zones matching `note` and `vel` only (ignore CC conditions).
    pub fn query_no_cc(&self, note: u8, vel: u8) -> Vec<&'a Zone> {
        self.doc.zones.iter()
            .filter(|z| z.matches_note_vel(note, vel))
            .collect()
    }

    /// Count zones per MIDI note (chromatic distribution).
    pub fn zone_count_per_note(&self) -> [usize; 128] {
        let mut counts = [0usize; 128];
        for z in &self.doc.zones {
            for n in z.lo_key..=z.hi_key {
                counts[n as usize] += 1;
            }
        }
        counts
    }

    /// Resolve all matching zones into ready-to-play [`AudioSource`] descriptors.
    pub fn audio_sources(&self, note: u8, vel: u8, cc: &[u8; 128]) -> Vec<AudioSource> {
        self.query(note, vel, cc)
            .into_iter()
            .map(|z| AudioSource::from_zone(z, note, vel, cc))
            .collect()
    }
}

// ── AudioSource ────────────────────────────────────────────────────────────────

/// A fully-resolved, ready-to-play audio descriptor derived from a [`Zone`].
///
/// All zone opcodes, CC modulations, and velocity scaling have been evaluated
/// into concrete playback parameters that an audio engine can consume directly.
///
/// # Example
/// ```no_run
/// use osmpcore::sfzjson::{SfzJson, ZoneMap};
///
/// let doc = SfzJson::from_file("instrument.json").unwrap();
/// let map = ZoneMap::new(&doc);
/// let cc  = doc.initial_cc();
/// for src in map.audio_sources(36, 100, &cc) {
///     println!("{} pitch={:.4} amp={:.4}", src.sample, src.pitch_ratio, src.amplitude);
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSource {
    /// Sample file path (relative to original SFZ root).
    pub sample: String,

    /// Playback speed multiplier.  1.0 = original pitch.
    /// Accounts for `note → root_key` offset, `transpose`, `tune_cents`,
    /// and any active `tune_ccN` modulations.
    pub pitch_ratio: f64,

    /// Linear amplitude 0.0–1.0 after `amplitude`, `amplitude_ccN`, and
    /// velocity scaling (`amp_velcurve` breakpoints or default linear curve).
    pub amplitude: f64,

    /// Stereo pan: −1.0 (full left) … 0.0 (centre) … +1.0 (full right).
    /// Accounts for `pan` and any active `pan_ccN` modulations.
    pub pan: f64,

    /// Volume offset in dB from the `volume_db` opcode (additive; 0.0 = no change).
    pub volume_db: f64,

    /// Loop mode string from the zone (`"one_shot"`, `"no_loop"`, `"loop_continuous"`, …).
    pub loop_mode: String,
    pub loop_start: u32,
    pub loop_end:   u32,

    /// Resolved AHDSR envelope — CC modulations on `ampeg_*_ccN` are NOT yet
    /// applied here (they require per-frame evaluation by the engine).
    pub ampeg: Ampeg,

    /// Round-robin position within the sequence.
    pub seq_position: u8,
    pub seq_length:   u8,

    /// Mute-group identifiers (0 = none).
    pub group_id: u32,
    pub off_by:   u32,
    pub off_mode: String,

    /// Trigger mode (`"attack"`, `"release"`, `"first"`, `"legato"`).
    pub trigger: String,
}

impl AudioSource {
    /// Build an `AudioSource` from a zone, evaluated at the given note, velocity,
    /// and CC state.
    pub fn from_zone(zone: &Zone, note: u8, vel: u8, cc: &[u8; 128]) -> Self {
        let pitch_ratio = Self::compute_pitch(zone, note, cc);
        let amplitude   = Self::compute_amplitude(zone, vel, cc);
        let pan         = Self::compute_pan(zone, cc);

        Self {
            sample:       zone.sample.clone(),
            pitch_ratio,
            amplitude,
            pan,
            volume_db:    zone.volume_db,
            loop_mode:    zone.loop_mode.clone(),
            loop_start:   zone.loop_start,
            loop_end:     zone.loop_end,
            ampeg:        zone.ampeg.clone(),
            seq_position: zone.seq_position,
            seq_length:   zone.seq_length,
            group_id:     zone.group_id,
            off_by:       zone.off_by,
            off_mode:     zone.off_mode.clone(),
            trigger:      zone.trigger.clone(),
        }
    }

    // ── pitch ──────────────────────────────────────────────────────────────────

    fn compute_pitch(zone: &Zone, note: u8, cc: &[u8; 128]) -> f64 {
        // Base semitone offset: played note relative to root key
        let mut cents = (note as f64 - zone.root_key as f64) * 100.0
            + zone.transpose as f64 * 100.0
            + zone.tune_cents as f64;

        // tune_ccN  — each mod adds (cc[N]/127) × value cents
        for (k, &val) in &zone.cc_mods {
            if let Some(n_str) = k.strip_prefix("tune_cc")
                .or_else(|| k.strip_prefix("tune_oncc"))
            {
                if let Ok(n) = n_str.parse::<usize>() {
                    if n < 128 {
                        cents += (cc[n] as f64 / 127.0) * val;
                    }
                }
            }
        }

        // Convert cents to ratio: 2^(cents/1200)
        2.0f64.powf(cents / 1200.0)
    }

    // ── amplitude ─────────────────────────────────────────────────────────────

    fn compute_amplitude(zone: &Zone, vel: u8, cc: &[u8; 128]) -> f64 {
        // CC amplitude modulations (already handles amplitude_ccN)
        let cc_amp = zone.amplitude_with_cc(cc);

        // Velocity scaling via amp_velcurve breakpoints or default linear curve
        let vel_scale = if zone.amp_velcurve.is_empty() {
            vel as f64 / 127.0   // SFZ default: linear
        } else {
            Self::interp_velcurve(&zone.amp_velcurve, vel)
        };

        // Combine: cc_amp already includes base amplitude; multiply by vel_scale
        // Re-apply correctly: amplitude_with_cc divides amplitude by itself then
        // multiplies by cc factor, so scale cc_amp by vel separately.
        (cc_amp * vel_scale).clamp(0.0, 1.0)
    }

    /// Linear interpolation through `amp_velcurve` breakpoints.
    /// Keys are velocity strings ("0"–"127"), values are 0.0–1.0 scalars.
    fn interp_velcurve(curve: &HashMap<String, f64>, vel: u8) -> f64 {
        // Collect and sort breakpoints
        let mut pts: Vec<(u8, f64)> = curve.iter()
            .filter_map(|(k, &v)| k.parse::<u8>().ok().map(|n| (n, v)))
            .collect();
        pts.sort_by_key(|(n, _)| *n);

        if pts.is_empty() { return vel as f64 / 127.0; }
        if vel <= pts[0].0 { return pts[0].1; }
        if vel >= pts[pts.len() - 1].0 { return pts[pts.len() - 1].1; }

        // Find surrounding pair and interpolate
        for win in pts.windows(2) {
            let (v0, a0) = win[0];
            let (v1, a1) = win[1];
            if vel >= v0 && vel <= v1 {
                let t = (vel - v0) as f64 / (v1 - v0) as f64;
                return a0 + (a1 - a0) * t;
            }
        }
        vel as f64 / 127.0
    }

    // ── pan ────────────────────────────────────────────────────────────────────

    fn compute_pan(zone: &Zone, cc: &[u8; 128]) -> f64 {
        // Base pan: SFZ range is −100..+100, normalise to −1..+1
        let mut pan = zone.pan / 100.0;

        // pan_ccN  — each mod shifts pan by (cc[N]/127) × value/100
        for (k, &val) in &zone.cc_mods {
            if let Some(n_str) = k.strip_prefix("pan_cc")
                .or_else(|| k.strip_prefix("pan_oncc"))
            {
                if let Ok(n) = n_str.parse::<usize>() {
                    if n < 128 {
                        pan += (cc[n] as f64 / 127.0) * (val / 100.0);
                    }
                }
            }
        }

        pan.clamp(-1.0, 1.0)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MINI_JSON: &str = r#"{
        "name": "test_kit",
        "cc_labels": { "70": "Kick mic" },
        "cc_init":   { "70": 100 },
        "defines":   { "$kickkey": "36" },
        "zones": [
            {
                "sample": "samples/kick_vl1_rr1.flac",
                "lo_key": 36, "hi_key": 36, "root_key": 36,
                "lo_vel": 0,  "hi_vel": 63,
                "loop_mode": "one_shot",
                "seq_length": 2, "seq_position": 1,
                "group_id": 1, "off_by": 1,
                "amplitude": 100.0,
                "ampeg": { "attack": 0.001, "hold": 0.2, "decay": 1.6,
                           "sustain": 100.0, "release": 0.01 },
                "cc_conditions": { "70": [1, 127] },
                "cc_mods": { "amplitude_cc70": 100, "tune_cc75": 1200 }
            },
            {
                "sample": "samples/kick_vl1_rr2.flac",
                "lo_key": 36, "hi_key": 36, "root_key": 36,
                "lo_vel": 0,  "hi_vel": 63,
                "loop_mode": "one_shot",
                "seq_length": 2, "seq_position": 2,
                "group_id": 1, "off_by": 1,
                "amplitude": 100.0,
                "ampeg": { "attack": 0.001, "hold": 0.2, "decay": 1.6,
                           "sustain": 100.0, "release": 0.01 },
                "cc_conditions": { "70": [1, 127] },
                "cc_mods": { "amplitude_cc70": 100, "tune_cc75": 1200 }
            },
            {
                "sample": "samples/snare_vl1_rr1.flac",
                "lo_key": 38, "hi_key": 38, "root_key": 38,
                "lo_vel": 0,  "hi_vel": 127,
                "loop_mode": "one_shot",
                "seq_length": 1, "seq_position": 1,
                "amplitude": 100.0,
                "ampeg": { "attack": 0.001, "hold": 0.2, "decay": 1.9,
                           "sustain": 100.0, "release": 0.01 }
            }
        ]
    }"#;

    fn mini() -> SfzJson {
        serde_json::from_str(MINI_JSON).expect("parse MINI_JSON")
    }

    #[test]
    fn parse_document() {
        let doc = mini();
        assert_eq!(doc.name, "test_kit");
        assert_eq!(doc.zones.len(), 3);
        assert_eq!(doc.cc_labels["70"], "Kick mic");
        assert_eq!(doc.cc_init["70"], 100);
    }

    #[test]
    fn zone_fields() {
        let doc   = mini();
        let kick  = &doc.zones[0];
        assert_eq!(kick.sample, "samples/kick_vl1_rr1.flac");
        assert_eq!(kick.lo_key, 36);
        assert_eq!(kick.hi_vel, 63);
        assert_eq!(kick.seq_length, 2);
        assert_eq!(kick.seq_position, 1);
        assert_eq!(kick.ampeg.hold, 0.2);
        assert_eq!(kick.cc_conditions["70"], [1, 127]);
        assert_eq!(kick.cc_mods["amplitude_cc70"], 100.0);
    }

    #[test]
    fn query_note_vel_cc() {
        let doc = mini();
        let map = ZoneMap::new(&doc);
        let mut cc = [0u8; 128];

        // CC70 off → kick zones filtered out
        let hits = map.query(36, 50, &cc);
        assert!(hits.is_empty(), "CC70=0 should filter kick (locc70=1)");

        // CC70 on → two kick RR zones
        cc[70] = 100;
        let hits = map.query(36, 50, &cc);
        assert_eq!(hits.len(), 2);

        // Snare has no CC conditions → always visible
        let snare_hits = map.query(38, 100, &cc);
        assert_eq!(snare_hits.len(), 1);
        assert_eq!(snare_hits[0].sample, "samples/snare_vl1_rr1.flac");
    }

    #[test]
    fn query_vel_layer() {
        let doc = mini();
        let map = ZoneMap::new(&doc);
        let mut cc = [100u8; 128];
        cc[70] = 100;

        // vel=100 > hi_vel=63 → no kick
        let hits = map.query(36, 100, &cc);
        assert!(hits.is_empty());
    }

    #[test]
    fn initial_cc() {
        let doc = mini();
        let cc  = doc.initial_cc();
        assert_eq!(cc[70], 100);
        assert_eq!(cc[0], 0);
    }

    #[test]
    fn amplitude_with_cc() {
        let doc  = mini();
        let zone = &doc.zones[0];
        let mut cc = [0u8; 128];

        // CC70 = 0  →  amplitude × (0/127) × (100/100) = 0
        assert_eq!(zone.amplitude_with_cc(&cc), 0.0);

        // CC70 = 127 → amplitude × (127/127) × 1 = 1.0
        cc[70] = 127;
        let amp = zone.amplitude_with_cc(&cc);
        assert!((amp - 1.0).abs() < 1e-6, "expected ~1.0, got {amp}");
    }

    #[test]
    fn zone_count_per_note() {
        let doc    = mini();
        let map    = ZoneMap::new(&doc);
        let counts = map.zone_count_per_note();
        assert_eq!(counts[36], 2); // two kick RR zones
        assert_eq!(counts[38], 1); // one snare zone
        assert_eq!(counts[40], 0); // nothing on note 40
    }

    // ── AudioSource tests ──────────────────────────────────────────────────────

    #[test]
    fn audio_source_pitch_root_note() {
        let doc  = mini();
        let zone = &doc.zones[0]; // root_key = 36
        let cc   = [0u8; 128];

        // Playing at root key → pitch_ratio must be 1.0
        let src = AudioSource::from_zone(zone, 36, 50, &cc);
        assert!((src.pitch_ratio - 1.0).abs() < 1e-9, "got {}", src.pitch_ratio);
    }

    #[test]
    fn audio_source_pitch_octave_up() {
        let doc  = mini();
        let zone = &doc.zones[0]; // root_key = 36
        let cc   = [0u8; 128];

        // note 48 = root + 12 semitones → pitch_ratio = 2.0
        let src = AudioSource::from_zone(zone, 48, 50, &cc);
        assert!((src.pitch_ratio - 2.0).abs() < 1e-6, "got {}", src.pitch_ratio);
    }

    #[test]
    fn audio_source_pitch_tune_cc() {
        let doc  = mini();
        let zone = &doc.zones[0]; // has tune_cc75=1200 (1200 cents at CC75=127)
        let mut cc = [0u8; 128];
        cc[75] = 127;

        // At root key with CC75=127 (full 1200 cents) → pitch_ratio = 2.0
        let src = AudioSource::from_zone(zone, 36, 50, &cc);
        assert!((src.pitch_ratio - 2.0).abs() < 1e-6, "got {}", src.pitch_ratio);
    }

    #[test]
    fn audio_source_amplitude_vel_linear() {
        let doc  = mini();
        let zone = &doc.zones[2]; // snare — no CC conditions, no cc_mods
        let cc   = [0u8; 128];

        let src_half = AudioSource::from_zone(zone, 38, 64, &cc);
        let src_full = AudioSource::from_zone(zone, 38, 127, &cc);

        // Amplitude at vel=64 ≈ 64/127 ≈ 0.504
        assert!((src_half.amplitude - 64.0 / 127.0).abs() < 1e-6);
        // Amplitude at vel=127 = 1.0
        assert!((src_full.amplitude - 1.0).abs() < 1e-6);
    }

    #[test]
    fn audio_source_amplitude_cc_gate() {
        let doc  = mini();
        let zone = &doc.zones[0]; // kick: cc_conditions cc70=[1,127], amplitude_cc70=100
        let mut cc = [0u8; 128];

        // CC70=0 → zone filtered out; but test AudioSource directly: cc_amp=0
        let src = AudioSource::from_zone(zone, 36, 50, &cc);
        assert_eq!(src.amplitude, 0.0, "expected 0 when CC70=0");

        cc[70] = 127;
        let src = AudioSource::from_zone(zone, 36, 50, &cc);
        // amplitude(100) * cc_scale(127/127 * 100/100) * vel(50/127) ≈ 0.394
        let expected = (50.0_f64 / 127.0).clamp(0.0, 1.0);
        assert!((src.amplitude - expected).abs() < 1e-6, "got {}", src.amplitude);
    }

    #[test]
    fn audio_source_pan_default() {
        let doc  = mini();
        let zone = &doc.zones[0]; // pan=0
        let cc   = [0u8; 128];
        let src  = AudioSource::from_zone(zone, 36, 50, &cc);
        assert_eq!(src.pan, 0.0);
    }

    #[test]
    fn audio_source_fields() {
        let doc  = mini();
        let zone = &doc.zones[0];
        let mut cc = [0u8; 128];
        cc[70] = 100;
        let src = AudioSource::from_zone(zone, 36, 50, &cc);

        assert_eq!(src.sample, "samples/kick_vl1_rr1.flac");
        assert_eq!(src.loop_mode, "one_shot");
        assert_eq!(src.seq_length, 2);
        assert_eq!(src.seq_position, 1);
        assert_eq!(src.group_id, 1);
        assert_eq!(src.off_by, 1);
        assert_eq!(src.trigger, "attack");
        assert_eq!(src.ampeg.hold, 0.2);
    }

    #[test]
    fn audio_sources_via_zonemap() {
        let doc = mini();
        let map = ZoneMap::new(&doc);
        let mut cc = [0u8; 128];
        cc[70] = 127;

        let sources = map.audio_sources(36, 50, &cc);
        assert_eq!(sources.len(), 2);
        assert!(sources[0].pitch_ratio > 0.9 && sources[0].pitch_ratio < 1.1);
    }
}
