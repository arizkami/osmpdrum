mod player;

use anyhow::Result;
use axum::{
    extract::{State, WebSocketUpgrade},
    extract::ws::{Message, WebSocket},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, Stream, StreamConfig};
use futures_util::{SinkExt, StreamExt};
use midir::{MidiInput, MidiInputConnection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

const FRONTEND_HTML: &str = include_str!("../../dist/index.html");

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", content = "payload")]
enum AudioCommand {
    Play { pad_id: usize, file_path: String, volume: f32, pan: f32 },
    Stop { pad_id: usize },
    Load { pad_id: usize, file_path: String },
    /// Load a v2 .osmp instrument file (replaces current player)
    LoadOsmp { path: String },
    /// Trigger note from frontend (bypasses WAV pad, hits OsmpPlayer)
    NoteOn { note: u8, vel: u8 },
    /// Pre-warm the OSMP decode cache
    WarmOsmpCache {},
    /// Return OsmpPlayer info as rust-osmp-info event
    GetOsmpInfo {},
    SetOsmpCC { cc_num: u8, value: u8 },
    GetOsmpZones {},
    SetMasterVolume { volume: f32 },
    SetPlaybackLatency { latency_ms: u32 },
    GetAudioSettings {},
    GetAudioBackends {},
    GetAudioDevices { backend: String },
    SetPlaybackBackend { backend: String },
    SetPlaybackDevice { device_name: String },
    SetBufferSizeFrames { frames: u32 },
    CheckUnsavedChanges,
    ConfirmExit,
    ListDirectory { path: Option<String>, filter: Option<Vec<String>> },
    GetDrives {},
    GetPresets {},
    GetLibrary {},
    GetMidiInputs {},
    SetMidiInput { port_name: Option<String> },
    SetWasapiExclusive { exclusive: bool },
    SetSampleRate { rate: u32 },
}

fn available_output_devices(backend: &str) -> Vec<String> {
    let host_id = parse_backend_host_id(Some(backend));
    let host = cpal::host_from_id(host_id).unwrap_or_else(|_| cpal::default_host());
    let mut out = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Ok(name) = dev.name() {
                out.push(name);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppSettings {
    master_volume: f32,
    playback_latency_ms: u32,
    playback_backend: Option<String>,
    playback_device_name: Option<String>,
    buffer_size_frames: Option<u32>,
    midi_input_port: Option<String>,
    wasapi_exclusive: bool,
    sample_rate: Option<u32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            playback_latency_ms: 50,
            playback_backend: None,
            playback_device_name: None,
            buffer_size_frames: None,
            midi_input_port: None,
            wasapi_exclusive: false,
            sample_rate: None,
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".config"))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        });

    base.join("osmpdrum")
}

fn settings_toml_path() -> std::path::PathBuf {
    settings_path().join("settings.toml")
}

fn settings_json_path() -> std::path::PathBuf {
    settings_path().join("settings.json")
}

fn normalize_settings(mut s: AppSettings) -> AppSettings {
    s.master_volume = s.master_volume.clamp(0.0, 1.0);
    s.playback_latency_ms = s.playback_latency_ms.clamp(5, 500);
    s.buffer_size_frames = s.buffer_size_frames.map(|f| f.clamp(64, 8192));
    s
}

fn load_settings() -> AppSettings {
    let toml_path = settings_toml_path();
    match std::fs::read_to_string(&toml_path) {
        Ok(data) => match toml::from_str::<AppSettings>(&data) {
            Ok(s) => return normalize_settings(s),
            Err(e) => {
                eprintln!("Failed parsing settings {}: {}", toml_path.display(), e);
                return AppSettings::default();
            }
        },
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                eprintln!("Failed reading settings {}: {}", toml_path.display(), e);
                return AppSettings::default();
            }
        }
    }

    let json_path = settings_json_path();
    let data = match std::fs::read_to_string(&json_path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                eprintln!("Failed reading settings {}: {}", json_path.display(), e);
            }
            return AppSettings::default();
        }
    };

    match serde_json::from_str::<AppSettings>(&data) {
        Ok(s) => {
            let s = normalize_settings(s);
            save_settings(&s);
            s
        }
        Err(e) => {
            eprintln!("Failed parsing settings {}: {}", json_path.display(), e);
            AppSettings::default()
        }
    }
}

fn save_settings(settings: &AppSettings) {
    let base = settings_path();
    if let Err(e) = std::fs::create_dir_all(&base) {
        eprintln!("Failed creating settings dir {}: {}", base.display(), e);
        return;
    }

    let path = settings_toml_path();
    let data = match toml::to_string_pretty(settings) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed serializing settings: {}", e);
            return;
        }
    };

    if let Err(e) = std::fs::write(&path, data) {
        eprintln!("Failed writing settings {}: {}", path.display(), e);
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct FsEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PresetInfo {
    name: String,
    path: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct LibraryEntry {
    name: String,
    path: String,
    size: u64,
    ext: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct WaveformData {
    pad_id: usize,
    peaks: Vec<f32>,
    duration: f32,
}

struct AudioBuffer {
    samples: Arc<Vec<f32>>,
    position: usize,
    volume: f32,
    playing: bool,
    pad_id: usize,
    voice_id: usize,
}

impl AudioBuffer {
    fn new(samples: Arc<Vec<f32>>, volume: f32, pad_id: usize, voice_id: usize) -> Self {
        Self {
            samples,
            position: 0,
            volume,
            playing: true,
            pad_id,
            voice_id,
        }
    }

    fn next_sample(&mut self) -> f32 {
        if !self.playing || self.position >= self.samples.len() {
            return 0.0;
        }
        let sample = self.samples[self.position] * self.volume;
        self.position += 1;
        sample
    }

    fn is_finished(&self) -> bool {
        self.position >= self.samples.len()
    }

    fn stop(&mut self) {
        self.playing = false;
    }
}

struct AudioEngine {
    host_id: cpal::HostId,
    device: Device,
    config: StreamConfig,
    buffers: Arc<Mutex<Vec<AudioBuffer>>>,
    stream: Option<Stream>,
    exclusive_stop_flag: Option<Arc<AtomicBool>>,
    master_volume: Arc<Mutex<f32>>,
    next_voice_id: Arc<Mutex<usize>>,
    sample_cache: Arc<Mutex<HashMap<String, Vec<f32>>>>,
    settings: Arc<Mutex<AppSettings>>,
}

impl AudioEngine {
    fn new() -> Result<Self> {
        let settings = load_settings();
        let host_id = parse_backend_host_id(settings.playback_backend.as_deref());
        let host = cpal::host_from_id(host_id).unwrap_or_else(|_| cpal::default_host());
        let device = select_output_device(&host, settings.playback_device_name.as_deref())?
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;
        
        let config = device.default_output_config()?;
        let config: StreamConfig = config.into();

        let config = apply_stream_tuning_to_config(config, &settings);
        
        let buffers: Arc<Mutex<Vec<AudioBuffer>>> = Arc::new(Mutex::new(Vec::new()));
        let master_volume = Arc::new(Mutex::new(settings.master_volume));
        let next_voice_id = Arc::new(Mutex::new(0usize));
        let sample_cache = Arc::new(Mutex::new(HashMap::new()));
        let settings = Arc::new(Mutex::new(settings));
        
        Ok(Self {
            host_id,
            device,
            config,
            buffers,
            stream: None,
            exclusive_stop_flag: None,
            master_volume,
            next_voice_id,
            sample_cache,
            settings,
        })
    }

    fn get_settings_snapshot(&self) -> AppSettings {
        match self.settings.lock() {
            Ok(g) => g.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn rebuild_device_and_config(&mut self) -> Result<()> {
        let settings = {
            match self.settings.lock() {
                Ok(g) => g.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        };

        let host_id = parse_backend_host_id(settings.playback_backend.as_deref());
        let host = cpal::host_from_id(host_id).unwrap_or_else(|_| cpal::default_host());
        let device = select_output_device(&host, settings.playback_device_name.as_deref())?
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;

        let config: StreamConfig = if let Some(sr) = settings.sample_rate {
            device.supported_output_configs()
                .ok()
                .and_then(|configs| {
                    configs
                        .filter(|c| c.min_sample_rate() <= sr && c.max_sample_rate() >= sr)
                        .find(|c| c.sample_format() == cpal::SampleFormat::F32)
                })
                .map(|c| c.with_sample_rate(sr).into())
                .unwrap_or_else(|| device.default_output_config()
                    .map(|c| c.into())
                    .unwrap_or_else(|_| StreamConfig {
                        channels: 2,
                        sample_rate: 48000,
                        buffer_size: BufferSize::Default,
                    }))
        } else {
            device.default_output_config()?.into()
        };
        let config = apply_stream_tuning_to_config(config, &settings);

        self.host_id = host_id;
        self.device = device;
        self.config = config;

        Ok(())
    }

    fn restart_stream(&mut self) -> Result<()> {
        if let Some(flag) = self.exclusive_stop_flag.take() {
            flag.store(true, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.start_stream()
    }

    fn start_stream(&mut self) -> Result<()> {
        let settings = self.get_settings_snapshot();

        #[cfg(target_os = "windows")]
        {
            let is_wasapi = settings.playback_backend
                .as_deref()
                .map(|b| b.to_ascii_uppercase() == "WASAPI")
                .unwrap_or(true);

            if settings.wasapi_exclusive && is_wasapi {
                if self.exclusive_stop_flag.is_none() {
                    return self.start_exclusive_stream_wasapi();
                }
                return Ok(());
            }
        }

        if self.stream.is_some() {
            return Ok(());
        }

        let buffers = self.buffers.clone();
        let master_volume = self.master_volume.clone();
        let channels = self.config.channels as usize;
        
        let stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buffers = match buffers.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let master_vol = match master_volume.lock() {
                    Ok(g) => *g,
                    Err(poisoned) => *poisoned.into_inner(),
                };
                
                for frame in data.chunks_mut(channels) {
                    let mut mixed_sample = 0.0f32;
                    
                    // Mix all playing buffers
                    for buffer in buffers.iter_mut() {
                        mixed_sample += buffer.next_sample();
                    }
                    
                    // Apply master volume
                    mixed_sample *= master_vol;
                    
                    // Soft clipping to prevent harsh distortion
                    mixed_sample = if mixed_sample > 1.0 {
                        1.0 - (1.0 / (mixed_sample + 1.0))
                    } else if mixed_sample < -1.0 {
                        -1.0 + (1.0 / (-mixed_sample + 1.0))
                    } else {
                        mixed_sample
                    };
                    
                    // Write to all channels
                    for sample in frame.iter_mut() {
                        *sample = mixed_sample;
                    }
                }
                
                // Remove finished buffers
                buffers.retain(|buffer| !buffer.is_finished());
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;
        
        stream.play()?;
        self.stream = Some(stream);
        println!("Audio stream started successfully");
        Ok(())
    }

    fn play(&mut self, pad_id: usize, file_path: &str, volume: f32, _pan: f32) -> Result<()> {
        if !std::path::Path::new(file_path).exists() {
            eprintln!("File not found: {}", file_path);
            return Ok(());
        }

        // Ensure stream is running
        self.start_stream()?;

        // Check cache first
        let samples = {
            let mut cache = self.sample_cache.lock().unwrap();
            if let Some(cached) = cache.get(file_path) {
                cached.clone()
            } else {
                // Load and cache
                let loaded = load_wav_file(file_path, self.config.sample_rate)?;
                cache.insert(file_path.to_string(), loaded.clone());
                loaded
            }
        };
        
        // Get next voice ID
        let voice_id = {
            let mut next_id = self.next_voice_id.lock().unwrap();
            let id = *next_id;
            *next_id = next_id.wrapping_add(1);
            id
        };
        
        let buffer = AudioBuffer::new(Arc::new(samples), volume, pad_id, voice_id);
        
        let mut buffers = self.buffers.lock().unwrap();
        buffers.push(buffer);
        
        println!("Playing pad {} (voice {}) with {} active voices", pad_id, voice_id, buffers.len());
        
        Ok(())
    }

    fn stop(&mut self, pad_id: usize) {
        let mut buffers = self.buffers.lock().unwrap();
        buffers.retain(|buffer| buffer.pad_id != pad_id);
        println!("Stopped all voices for pad {}", pad_id);
    }

    fn set_master_volume(&mut self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        *self.master_volume.lock().unwrap() = clamped;
        if let Ok(mut s) = self.settings.lock() {
            s.master_volume = clamped;
            save_settings(&s);
        }
        println!("Master volume set to {}", clamped);
    }

    fn set_playback_latency_ms(&mut self, latency_ms: u32) {
        let clamped = latency_ms.clamp(5, 500);
        if let Ok(mut s) = self.settings.lock() {
            s.playback_latency_ms = clamped;
            // If user sets latency explicitly, clear explicit frame size.
            s.buffer_size_frames = None;
            save_settings(&s);
        }

        let settings = {
            match self.settings.lock() {
                Ok(g) => g.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        };
        self.config = apply_stream_tuning_to_config(self.config.clone(), &settings);

        if let Err(e) = self.restart_stream() {
            eprintln!("Failed restarting audio stream after latency change: {}", e);
        } else {
            println!("Playback latency set to {}ms", clamped);
        }
    }

    fn set_buffer_size_frames(&mut self, frames: u32) {
        if let Ok(mut s) = self.settings.lock() {
            if frames == 0 {
                s.buffer_size_frames = None;
            } else {
                let clamped = frames.clamp(64, 8192);
                s.buffer_size_frames = Some(clamped);
            }
            save_settings(&s);
        }

        let settings = {
            match self.settings.lock() {
                Ok(g) => g.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        };
        self.config = apply_stream_tuning_to_config(self.config.clone(), &settings);

        if let Err(e) = self.restart_stream() {
            eprintln!("Failed restarting audio stream after buffer size change: {}", e);
        } else {
            if frames == 0 {
                println!("Buffer size set to Auto");
            } else {
                let clamped = frames.clamp(64, 8192);
                println!("Buffer size set to {} frames", clamped);
            }
        }
    }

    fn set_playback_backend(&mut self, backend: &str) {
        if let Ok(mut s) = self.settings.lock() {
            s.playback_backend = Some(backend.to_string());
            s.playback_device_name = None;
            save_settings(&s);
        }

        if let Err(e) = self.rebuild_device_and_config() {
            eprintln!("Failed selecting backend {}: {}", backend, e);
            return;
        }

        if let Err(e) = self.restart_stream() {
            eprintln!("Failed restarting audio stream after backend change: {}", e);
        } else {
            println!("Playback backend set to {}", backend);
        }
    }

    fn set_playback_device(&mut self, device_name: &str) {
        if let Ok(mut s) = self.settings.lock() {
            s.playback_device_name = Some(device_name.to_string());
            save_settings(&s);
        }

        if let Err(e) = self.rebuild_device_and_config() {
            eprintln!("Failed selecting device {}: {}", device_name, e);
            return;
        }

        if let Err(e) = self.restart_stream() {
            eprintln!("Failed restarting audio stream after device change: {}", e);
        } else {
            println!("Playback device set to {}", device_name);
        }
    }

    fn set_wasapi_exclusive(&mut self, exclusive: bool) {
        if let Ok(mut s) = self.settings.lock() {
            s.wasapi_exclusive = exclusive;
            save_settings(&s);
        }
        if let Ok(mut cache) = self.sample_cache.lock() {
            cache.clear();
        }
        if let Err(e) = self.restart_stream() {
            eprintln!("Failed restarting stream after exclusive mode change: {}", e);
        } else {
            println!("WASAPI exclusive mode: {}", exclusive);
        }
    }

    fn set_sample_rate(&mut self, rate: u32) {
        if let Ok(mut s) = self.settings.lock() {
            s.sample_rate = if rate == 0 { None } else { Some(rate) };
            save_settings(&s);
        }
        if let Ok(mut cache) = self.sample_cache.lock() {
            cache.clear();
        }
        if let Err(e) = self.rebuild_device_and_config() {
            eprintln!("Failed rebuilding config after sample rate change: {}", e);
        }
        if let Err(e) = self.restart_stream() {
            eprintln!("Failed restarting stream after sample rate change: {}", e);
        } else {
            println!("Sample rate set to {}Hz", rate);
        }
    }

    #[cfg(target_os = "windows")]
    fn start_exclusive_stream_wasapi(&mut self) -> Result<()> {
        let settings = self.get_settings_snapshot();
        let sample_rate = settings.sample_rate.unwrap_or(48000) as usize;
        self.config.sample_rate = sample_rate as u32;
        if let Ok(mut cache) = self.sample_cache.lock() {
            cache.clear();
        }
        let buffers = self.buffers.clone();
        let master_volume = self.master_volume.clone();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        thread::spawn(move || {
            if let Err(e) = run_exclusive_render_loop(sample_rate, buffers, master_volume, stop_flag_clone) {
                eprintln!("WASAPI exclusive render error: {}", e);
            }
        });
        self.exclusive_stop_flag = Some(stop_flag);
        println!("WASAPI Exclusive stream started at {}Hz", sample_rate);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn run_exclusive_render_loop(
    sample_rate: usize,
    buffers: Arc<Mutex<Vec<AudioBuffer>>>,
    master_volume: Arc<Mutex<f32>>,
    stop_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    use wasapi::{initialize_mta, get_default_device, Direction, ShareMode, SampleType, WaveFormat};

    initialize_mta().map_err(|e| anyhow::anyhow!("COM init: {}", e))?;
    let device = get_default_device(&Direction::Render)
        .map_err(|e| anyhow::anyhow!("WASAPI device: {}", e))?;
    let mut audio_client = device.get_iaudioclient()
        .map_err(|e| anyhow::anyhow!("IAudioClient: {}", e))?;

    let wave_fmt = WaveFormat::new(32, 32, &SampleType::Float, sample_rate, 2, None);
    let (_, min_period) = audio_client.get_periods()
        .map_err(|e| anyhow::anyhow!("get_periods: {}", e))?;

    audio_client.initialize_client(
        &wave_fmt,
        min_period,
        &Direction::Render,
        &ShareMode::Exclusive,
        true,
    ).map_err(|e| anyhow::anyhow!("initialize_client: {}", e))?;

    let h_event = audio_client.set_get_eventhandle()
        .map_err(|e| anyhow::anyhow!("set_get_eventhandle: {}", e))?;
    let mut render_client = audio_client.get_audiorenderclient()
        .map_err(|e| anyhow::anyhow!("get_audiorenderclient: {}", e))?;
    let buffer_frame_count = audio_client.get_bufferframecount()
        .map_err(|e| anyhow::anyhow!("get_bufferframecount: {}", e))?;

    audio_client.start_stream()
        .map_err(|e| anyhow::anyhow!("start_stream: {}", e))?;

    let blockalign = wave_fmt.get_blockalign() as usize;
    let frames_available = buffer_frame_count as usize;

    while !stop_flag.load(Ordering::Relaxed) {
        if h_event.wait_for_event(200).is_err() {
            continue;
        }

        let mut audio_bytes: Vec<u8> = Vec::with_capacity(frames_available * blockalign);
        {
            let mut bufs = match buffers.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let master_vol = match master_volume.lock() {
                Ok(g) => *g,
                Err(p) => *p.into_inner(),
            };
            for _ in 0..frames_available {
                let mut mixed = 0.0f32;
                for buf in bufs.iter_mut() {
                    mixed += buf.next_sample();
                }
                mixed *= master_vol;
                mixed = if mixed > 1.0 {
                    1.0 - (1.0 / (mixed + 1.0))
                } else if mixed < -1.0 {
                    -1.0 + (1.0 / (-mixed + 1.0))
                } else {
                    mixed
                };
                audio_bytes.extend_from_slice(&mixed.to_le_bytes()); // left
                audio_bytes.extend_from_slice(&mixed.to_le_bytes()); // right
            }
            bufs.retain(|b| !b.is_finished());
        }

        if let Err(e) = render_client.write_to_device(
            frames_available,
            blockalign,
            &audio_bytes,
            None,
        ) {
            eprintln!("write_to_device: {}", e);
        }
    }

    let _ = audio_client.stop_stream();
    Ok(())
}

fn select_output_device(host: &cpal::Host, preferred_name: Option<&str>) -> Result<Option<Device>> {
    if let Some(name) = preferred_name {
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                if let Ok(dev_name) = dev.name() {
                    if dev_name == name {
                        return Ok(Some(dev));
                    }
                }
            }
        }
    }

    Ok(host.default_output_device())
}

fn apply_stream_tuning_to_config(mut config: StreamConfig, settings: &AppSettings) -> StreamConfig {
    let frames = if let Some(frames) = settings.buffer_size_frames {
        frames.clamp(64, 8192)
    } else {
        let ms = settings.playback_latency_ms.clamp(5, 500) as f32;
        let sr = config.sample_rate as f32;
        let frames = (sr * (ms / 1000.0)).round() as u32;
        frames.clamp(64, 8192)
    };

    config.buffer_size = BufferSize::Fixed(frames);
    config
}

fn parse_backend_host_id(backend: Option<&str>) -> cpal::HostId {
    match backend.map(|s| s.to_ascii_lowercase()) {
        #[cfg(feature = "asio")]
        Some(s) if s == "asio" => cpal::HostId::Asio,
        #[cfg(target_os = "windows")]
        Some(s) if s == "ks" => cpal::HostId::Wasapi,
        #[cfg(target_os = "windows")]
        Some(s) if s == "wasapi" => cpal::HostId::Wasapi,
        #[cfg(target_os = "linux")]
        Some(s) if s == "alsa" => cpal::HostId::Alsa,
        #[cfg(all(target_os = "linux", feature = "jack"))]
        Some(s) if s == "jack" => cpal::HostId::Jack,
        _ => cpal::default_host().id(),
    }
}

fn available_backends() -> Vec<String> {
    let mut out = Vec::new();
    let mut has_wasapi = false;
    for host_id in cpal::available_hosts() {
        match host_id {
            #[cfg(target_os = "windows")]
            cpal::HostId::Wasapi => {
                has_wasapi = true;
                out.push("WASAPI".to_string());
            }
            #[cfg(target_os = "linux")]
            cpal::HostId::Alsa => {
                out.push("ALSA".to_string());
            }
            #[cfg(all(target_os = "linux", feature = "jack"))]
            cpal::HostId::Jack => {
                out.push("JACK".to_string());
            }
            #[cfg(not(target_os = "windows"))]
            _ => {}
        }
    }
    if has_wasapi {
        out.push("KS".to_string());
    }
    out
}


fn load_wav_file(file_path: &str, target_sample_rate: u32) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(file_path)?;
    let spec = reader.spec();
    
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
        }
        hound::SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => reader.samples::<i16>()
                    .map(|s| s.unwrap_or(0) as f32 / 32768.0)
                    .collect(),
                24 => reader.samples::<i32>()
                    .map(|s| s.unwrap_or(0) as f32 / 8388608.0)
                    .collect(),
                32 => reader.samples::<i32>()
                    .map(|s| s.unwrap_or(0) as f32 / 2147483648.0)
                    .collect(),
                _ => return Err(anyhow::anyhow!("Unsupported bit depth")),
            }
        }
    };
    
    // Convert stereo to mono if needed
    let mono_samples: Vec<f32> = if spec.channels == 2 {
        samples.chunks(2).map(|chunk| (chunk[0] + chunk.get(1).unwrap_or(&0.0)) / 2.0).collect()
    } else {
        samples
    };
    
    // Simple resampling if needed
    if spec.sample_rate != target_sample_rate {
        let ratio = spec.sample_rate as f32 / target_sample_rate as f32;
        let new_len = (mono_samples.len() as f32 / ratio) as usize;
        let resampled: Vec<f32> = (0..new_len)
            .map(|i| {
                let pos = i as f32 * ratio;
                let idx = pos as usize;
                if idx < mono_samples.len() {
                    mono_samples[idx]
                } else {
                    0.0
                }
            })
            .collect();
        Ok(resampled)
    } else {
        Ok(mono_samples)
    }
}

async fn serve_frontend() -> Html<&'static str> {
    Html(FRONTEND_HTML)
}

// ── Server state ─────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    engine:      Arc<Mutex<AudioEngine>>,
    event_tx:    broadcast::Sender<String>,
    midi_conn:   Arc<Mutex<Option<MidiInputConnection<()>>>>,
    osmp_player: Arc<Mutex<Option<player::OsmpPlayer>>>,
}

// ── WebSocket handler ─────────────────────────────────────────────────────────

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_tx.subscribe();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = event_rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => handle_command(text.as_str(), &state).await,
            Message::Close(_)   => break,
            _                   => {}
        }
    }
    send_task.abort();
}

// ── IPC command router ────────────────────────────────────────────────────────

async fn handle_command(text: &str, state: &AppState) {
    let cmd: AudioCommand = match serde_json::from_str(text) {
        Ok(c)  => c,
        Err(e) => { eprintln!("Bad IPC command: {}: {}", e, text); return; }
    };
    let tx = &state.event_tx;

    match cmd {
        AudioCommand::Play { pad_id, file_path, volume, pan } => {
            if let Ok(mut eng) = state.engine.lock() {
                if let Err(e) = eng.play(pad_id, &file_path, volume, pan) {
                    eprintln!("Play error: {}", e);
                }
            }
        }
        AudioCommand::Stop { pad_id } => {
            if let Ok(mut eng) = state.engine.lock() { eng.stop(pad_id); }
        }
        AudioCommand::SetMasterVolume { volume } => {
            if let Ok(mut eng) = state.engine.lock() { eng.set_master_volume(volume); }
        }
        AudioCommand::SetPlaybackLatency { latency_ms } => {
            if let Ok(mut eng) = state.engine.lock() { eng.set_playback_latency_ms(latency_ms); }
        }
        AudioCommand::SetBufferSizeFrames { frames } => {
            if let Ok(mut eng) = state.engine.lock() { eng.set_buffer_size_frames(frames); }
        }
        AudioCommand::GetAudioSettings {} => {
            if let Ok(eng) = state.engine.lock() {
                let s = eng.get_settings_snapshot();
                if let Ok(json) = serde_json::to_string(&s) {
                    let _ = tx.send(format!(r#"{{"type":"rust-audio-settings","detail":{}}}"#, json));
                }
            }
        }
        AudioCommand::GetAudioBackends {} => {
            let backends = available_backends();
            if let Ok(json) = serde_json::to_string(&backends) {
                let _ = tx.send(format!(r#"{{"type":"rust-audio-backends","detail":{}}}"#, json));
            }
        }
        AudioCommand::GetAudioDevices { backend } => {
            let devices = available_output_devices(&backend);
            if let Ok(json) = serde_json::to_string(&devices) {
                let _ = tx.send(format!(r#"{{"type":"rust-audio-devices","detail":{}}}"#, json));
            }
        }
        AudioCommand::SetPlaybackBackend { backend } => {
            if let Ok(mut eng) = state.engine.lock() { eng.set_playback_backend(&backend); }
        }
        AudioCommand::SetPlaybackDevice { device_name } => {
            if let Ok(mut eng) = state.engine.lock() { eng.set_playback_device(&device_name); }
        }
        AudioCommand::Load { pad_id, file_path } => {
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                if let Ok(mut reader) = hound::WavReader::open(&file_path) {
                    let spec    = reader.spec();
                    let duration = reader.duration() as f32 / spec.sample_rate as f32;
                    let samples: Vec<f32> = match spec.sample_format {
                        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
                        hound::SampleFormat::Int => match spec.bits_per_sample {
                            16 => reader.samples::<i16>().map(|s| s.unwrap_or(0) as f32 / 32768.0).collect(),
                            24 => reader.samples::<i32>().map(|s| s.unwrap_or(0) as f32 / 8388608.0).collect(),
                            32 => reader.samples::<i32>().map(|s| s.unwrap_or(0) as f32 / 2147483648.0).collect(),
                            _  => vec![],
                        },
                    };
                    let mono: Vec<f32> = if spec.channels == 2 {
                        samples.chunks(2).map(|c| (c[0] + c.get(1).unwrap_or(&0.0)) / 2.0).collect()
                    } else { samples };
                    let points     = 200usize;
                    let chunk_size = (mono.len() / points).max(1);
                    let peaks: Vec<f32> = mono.chunks(chunk_size)
                        .map(|c| c.iter().fold(0.0f32, |a, &b| a.max(b.abs())))
                        .collect();
                    let data = WaveformData { pad_id, peaks, duration };
                    if let Ok(json) = serde_json::to_string(&data) {
                        let _ = tx2.send(format!(r#"{{"type":"rust-waveform-ready","detail":{}}}"#, json));
                    }
                }
            });
        }
        AudioCommand::CheckUnsavedChanges => { /* handled in frontend */ }
        AudioCommand::ConfirmExit => { std::process::exit(0); }
        AudioCommand::ListDirectory { path, filter } => {
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                let dir = path.unwrap_or_else(|| {
                    std::env::var("HOME")
                        .or_else(|_| std::env::var("USERPROFILE"))
                        .unwrap_or_else(|_| ".".to_string())
                });
                let allowed: Option<Vec<String>> = filter;
                let mut entries: Vec<FsEntry> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for e in rd.filter_map(|e| e.ok()) {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.starts_with('.') { continue; }
                        let meta   = e.metadata().ok();
                        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let size   = if !is_dir { meta.map(|m| m.len()) } else { None };
                        let path   = e.path().to_string_lossy().to_string();
                        let ext    = e.path().extension().map(|x| x.to_string_lossy().to_lowercase().to_string()).unwrap_or_default();
                        let show = is_dir || match &allowed {
                            Some(exts) => exts.iter().any(|x| x == &ext),
                            None => matches!(ext.as_str(), "wav" | "mp3" | "flac" | "ogg" | "aiff" | "osmp" | "osmpd" | "json"),
                        };
                        if show {
                            entries.push(FsEntry { name, path, is_dir, size });
                        }
                    }
                }
                entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                });
                // Add parent entry when not at root
                let p = std::path::Path::new(&dir);
                let parent = p.parent().map(|x| x.to_string_lossy().to_string());
                #[derive(Serialize)]
                struct DirPayload { path: String, parent: Option<String>, entries: Vec<FsEntry> }
                let payload = DirPayload { path: dir, parent, entries };
                if let Ok(json) = serde_json::to_string(&payload) {
                    let _ = tx2.send(format!(r#"{{"type":"rust-dir-listing","detail":{}}}"#, json));
                }
            });
        }
        AudioCommand::GetDrives {} => {
            let tx2 = tx.clone();
            tokio::task::spawn_blocking(move || {
                #[cfg(target_os = "windows")]
                let drives: Vec<String> = (b'A'..=b'Z')
                    .map(|c| format!("{}:\\", c as char))
                    .filter(|d| std::path::Path::new(d).exists())
                    .collect();
                #[cfg(not(target_os = "windows"))]
                let drives: Vec<String> = vec!["/".to_string()];
                if let Ok(json) = serde_json::to_string(&drives) {
                    let _ = tx2.send(format!(r#"{{"type":"rust-drives","detail":{}}}"#, json));
                }
            });
        }
        AudioCommand::GetPresets {} => {
            let tx2  = tx.clone();
            let base = settings_path().join("presets");
            tokio::task::spawn_blocking(move || {
                let _ = std::fs::create_dir_all(&base);
                let mut presets: Vec<PresetInfo> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&base) {
                    for e in rd.filter_map(|e| e.ok()) {
                        let ext = e.path().extension().map(|x| x.to_string_lossy().to_lowercase().to_string()).unwrap_or_default();
                        if ext == "json" || ext == "toml" {
                            let name = e.path().file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            let path = e.path().to_string_lossy().to_string();
                            presets.push(PresetInfo { name, path });
                        }
                    }
                }
                presets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                if let Ok(json) = serde_json::to_string(&presets) {
                    let _ = tx2.send(format!(r#"{{"type":"rust-presets","detail":{}}}"#, json));
                }
            });
        }
        AudioCommand::GetLibrary {} => {
            let tx2  = tx.clone();
            let base = settings_path().join("library");
            tokio::task::spawn_blocking(move || {
                let _ = std::fs::create_dir_all(&base);
                let mut entries: Vec<LibraryEntry> = Vec::new();
                scan_audio_dir(&base, &mut entries);
                entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                if let Ok(json) = serde_json::to_string(&entries) {
                    let _ = tx2.send(format!(r#"{{"type":"rust-library","detail":{}}}"#, json));
                }
            });
        }
        AudioCommand::GetMidiInputs {} => {
            let ports = list_midi_inputs();
            if let Ok(json) = serde_json::to_string(&ports) {
                let _ = tx.send(format!(r#"{{"type":"rust-midi-inputs","detail":{}}}"#, json));
            }
        }
        AudioCommand::SetMidiInput { port_name } => {
            let mut conn_guard = state.midi_conn.lock().unwrap();
            *conn_guard = None;
            if let Some(ref name) = port_name {
                let tx2          = tx.clone();
                let (bufs, vids) = {
                    let eng = state.engine.lock().unwrap_or_else(|p| p.into_inner());
                    (eng.buffers.clone(), eng.next_voice_id.clone())
                };
                if let Some(conn) = connect_midi_input(name, tx2, bufs, vids, state.osmp_player.clone()) {
                    *conn_guard = Some(conn);
                    println!("MIDI connected: {}", name);
                    if let Ok(eng) = state.engine.lock() {
                        if let Ok(mut s) = eng.settings.lock() {
                            s.midi_input_port = port_name;
                            save_settings(&s);
                        }
                    }
                } else {
                    eprintln!("MIDI connect failed: {}", name);
                }
            } else {
                if let Ok(eng) = state.engine.lock() {
                    if let Ok(mut s) = eng.settings.lock() {
                        s.midi_input_port = None;
                        save_settings(&s);
                    }
                }
                println!("MIDI disconnected");
            }
        }
        AudioCommand::SetWasapiExclusive { exclusive } => {
            #[cfg(target_os = "windows")]
            if let Ok(mut eng) = state.engine.lock() {
                eng.set_wasapi_exclusive(exclusive);
            }
            #[cfg(not(target_os = "windows"))]
            eprintln!("SetWasapiExclusive ignored on non-Windows (exclusive={})", exclusive);
        }
        AudioCommand::SetSampleRate { rate } => {
            if let Ok(mut eng) = state.engine.lock() { eng.set_sample_rate(rate); }
        }

        // ── OSMP Player commands ──────────────────────────────────────────────
        AudioCommand::LoadOsmp { path } => {
            let tx2     = tx.clone();
            let player2 = state.osmp_player.clone();
            let engine2 = state.engine.clone();
            tokio::task::spawn_blocking(move || {
                // ── Free old resources BEFORE allocating the new mmap ──────────
                // 1. Release old player (drops mmap + decode_cache)
                {
                    let mut g = player2.lock().unwrap_or_else(|e| e.into_inner());
                    if g.is_some() {
                        println!("[LoadOsmp] dropping old OsmpPlayer");
                        *g = None;
                    }
                }
                // 2. Drain audio buffers so their Arc<Vec<f32>> refs are freed
                if let Ok(eng) = engine2.lock() {
                    if let Ok(mut bufs) = eng.buffers.lock() {
                        let n = bufs.len();
                        bufs.clear();
                        if n > 0 { println!("[LoadOsmp] cleared {} audio buffers", n); }
                    }
                }
                // ───────────────────────────────────────────────────────────────

                #[derive(Serialize)]
                struct OsmpLoadedInfo { name: String, samples: usize, zones: usize, size_mb: usize, json_len: u32 }
                match player::OsmpPlayer::load(&path) {
                    Ok(p) => {
                        let info = OsmpLoadedInfo {
                            name:     p.name().to_owned(),
                            samples:  p.sample_count(),
                            zones:    p.zone_count(),
                            size_mb:  p.file_size_mb(),
                            json_len: p.json_len(),
                        };
                        let zones_json = build_osmp_zones_json(&p);
                        *player2.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
                        if let Ok(detail) = serde_json::to_string(&info) {
                            let _ = tx2.send(format!(r#"{{"type":"rust-osmp-loaded","detail":{detail}}}"#));
                        }
                        let _ = tx2.send(format!(r#"{{"type":"rust-osmp-zones","detail":{zones_json}}}"#));
                    }
                    Err(e) => {
                        eprintln!("LoadOsmp error: {}", e);
                        let msg = format!(
                            r#"{{"type":"rust-osmp-error","detail":{{"error":"{}"}}}}"#,
                            e.to_string().replace('"', "'")
                        );
                        let _ = tx2.send(msg);
                    }
                }
            });
        }

        AudioCommand::NoteOn { note, vel } => {
            let engine2 = state.engine.clone();
            let player2 = state.osmp_player.clone();
            let sources = {
                let guard = player2.lock().unwrap_or_else(|p| p.into_inner());
                guard.as_ref().map(|p| p.trigger(note, vel, None))
            };
            if let Some(sources) = sources {
                if let Ok(mut eng) = engine2.lock() {
                    eng.start_stream().ok();
                    let mut voice = eng.next_voice_id.lock().unwrap_or_else(|p| p.into_inner());
                    let mut bufs  = eng.buffers.lock().unwrap_or_else(|p| p.into_inner());
                    for (amp, _pan, pcm) in sources {
                        let vid  = *voice;
                        *voice   = voice.wrapping_add(1);
                        let mono = Arc::new(player::stereo_to_mono(&pcm));
                        bufs.push(AudioBuffer::new(mono, amp.clamp(0.0, 1.0), note as usize, vid));
                    }
                }
            }
        }

        AudioCommand::WarmOsmpCache {} => {
            let player2 = state.osmp_player.clone();
            tokio::task::spawn_blocking(move || {
                let guard = player2.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(p) = guard.as_ref() { p.warm_cache(); }
            });
        }

        AudioCommand::GetOsmpInfo {} => {
            let guard = state.osmp_player.lock().unwrap_or_else(|p| p.into_inner());
            let info = if let Some(p) = guard.as_ref() {
                format!(
                    r#"{{"loaded":true,"name":"{}","samples":{},"zones":{},"size_mb":{}}}"#,
                    p.name(), p.sample_count(), p.zone_count(), p.file_size_mb()
                )
            } else {
                r#"{"loaded":false}"#.to_string()
            };
            let _ = tx.send(format!(r#"{{"type":"rust-osmp-info","detail":{}}}"#, info));
        }

        AudioCommand::SetOsmpCC { cc_num, value } => {
            let guard = state.osmp_player.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(p) = guard.as_ref() {
                p.set_cc(cc_num, value);
            }
        }

        AudioCommand::GetOsmpZones {} => {
            let guard = state.osmp_player.lock().unwrap_or_else(|p| p.into_inner());
            let json = if let Some(p) = guard.as_ref() {
                build_osmp_zones_json(p)
            } else {
                "null".to_string()
            };
            let _ = tx.send(format!(r#"{{"type":"rust-osmp-zones","detail":{}}}"#, json));
        }
    }
}

// ── OSMP zone helpers ─────────────────────────────────────────────────────────

fn derive_label(sample: &str) -> String {
    let stem = std::path::Path::new(sample)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(sample);
    let base = stem.split('_').next().unwrap_or(stem);
    let mut chars = base.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn build_osmp_zones_json(player: &player::OsmpPlayer) -> String {
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct ZoneSlot {
        note:       u8,
        label:      String,
        sample:     String,
        lo_key:     u8,
        hi_key:     u8,
        root_key:   u8,
        lo_vel:     u8,
        hi_vel:     u8,
        group_id:   u32,
        zone_count: usize,
        seq_length: u8,
        ampeg_attack:  f64,
        ampeg_hold:    f64,
        ampeg_decay:   f64,
        ampeg_sustain: f64,
        ampeg_release: f64,
        volume_db:  f64,
        pan:        f64,
    }

    #[derive(Serialize)]
    struct OsmpZonesPayload {
        name:      String,
        cc_labels: std::collections::HashMap<String, String>,
        cc_init:   std::collections::HashMap<String, u8>,
        cc_state:  Vec<(u8, u8)>,
        pad_slots: Vec<ZoneSlot>,
    }

    // Group zones by root_key using a BTreeMap so they come out sorted
    let mut by_root: BTreeMap<u8, Vec<&osmpcore::sfzjson::Zone>> = BTreeMap::new();
    for zone in &player.doc.zones {
        by_root.entry(zone.root_key).or_default().push(zone);
    }

    let pad_slots: Vec<ZoneSlot> = by_root.iter().take(32).map(|(&root, zones)| {
        let rep = zones[0];
        ZoneSlot {
            note:       root,
            label:      derive_label(&rep.sample),
            sample:     rep.sample.clone(),
            lo_key:     rep.lo_key,
            hi_key:     rep.hi_key,
            root_key:   root,
            lo_vel:     zones.iter().map(|z| z.lo_vel).min().unwrap_or(0),
            hi_vel:     zones.iter().map(|z| z.hi_vel).max().unwrap_or(127),
            group_id:   rep.group_id,
            zone_count: zones.len(),
            seq_length: rep.seq_length,
            ampeg_attack:  rep.ampeg.attack,
            ampeg_hold:    rep.ampeg.hold,
            ampeg_decay:   rep.ampeg.decay,
            ampeg_sustain: rep.ampeg.sustain,
            ampeg_release: rep.ampeg.release,
            volume_db:  rep.volume_db,
            pan:        rep.pan,
        }
    }).collect();

    let cc_state: Vec<(u8, u8)> = player.doc.cc_labels.keys()
        .filter_map(|k| k.parse::<u8>().ok())
        .map(|n| (n, player.get_cc()[n as usize]))
        .collect();

    let payload = OsmpZonesPayload {
        name:      player.doc.name.clone(),
        cc_labels: player.doc.cc_labels.clone(),
        cc_init:   player.doc.cc_init.clone(),
        cc_state,
        pad_slots,
    };

    serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string())
}

// ── MIDI helpers ─────────────────────────────────────────────────────────────

fn list_midi_inputs() -> Vec<String> {
    match MidiInput::new("osmpdrum-list") {
        Ok(midi_in) => midi_in.ports().iter()
            .filter_map(|p| midi_in.port_name(p).ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn connect_midi_input(
    port_name:    &str,
    tx:           broadcast::Sender<String>,
    engine_bufs:  Arc<Mutex<Vec<AudioBuffer>>>,
    voice_id_ctr: Arc<Mutex<usize>>,
    osmp_player:  Arc<Mutex<Option<player::OsmpPlayer>>>,
) -> Option<MidiInputConnection<()>> {
    let midi_in = MidiInput::new("osmpdrum").ok()?;
    let ports   = midi_in.ports();
    let port    = ports.iter().find(|p| midi_in.port_name(p).ok().as_deref() == Some(port_name))?;
    midi_in.connect(port, "osmpdrum-in", move |_ts, data, _| {
        if data.len() < 3 { return; }
        let kind    = data[0] & 0xF0;
        let channel = data[0] & 0x0F;
        let note    = data[1];
        let vel     = data[2];

        if kind == 0x90 && vel > 0 {
            // Notify UI
            let msg = format!(
                r#"{{"type":"rust-midi-note","detail":{{"note":{},"velocity":{},"channel":{}}}}}"#,
                note, vel, channel
            );
            let _ = tx.send(msg);

            // Trigger OsmpPlayer directly (no WS round-trip)
            let sources = {
                let guard = osmp_player.lock().unwrap_or_else(|p| p.into_inner());
                guard.as_ref().map(|p| p.trigger(note, vel, None))
            };
            if let Some(sources) = sources {
                let mut voice = voice_id_ctr.lock().unwrap_or_else(|p| p.into_inner());
                let mut bufs  = engine_bufs.lock().unwrap_or_else(|p| p.into_inner());
                for (amp, _pan, pcm) in sources {
                    let vid = *voice;
                    *voice  = voice.wrapping_add(1);
                    // Average stereo → mono
                    let mono: Arc<Vec<f32>> = Arc::new(player::stereo_to_mono(&pcm));
                    bufs.push(AudioBuffer::new(mono, amp.clamp(0.0, 1.0), note as usize, vid));
                }
            }
        }
    }, ()).ok()
}

fn scan_audio_dir(dir: &std::path::Path, entries: &mut Vec<LibraryEntry>) {
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                scan_audio_dir(&path, entries);
            } else if let Some(ext_os) = path.extension() {
                let ext = ext_os.to_string_lossy().to_lowercase();
                if matches!(ext.as_str(), "wav" | "mp3" | "flac" | "ogg" | "aiff") {
                    let name     = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let size     = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let path_str = path.to_string_lossy().to_string();
                    entries.push(LibraryEntry { name, path: path_str, size, ext: ext.to_string() });
                }
            }
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|info| eprintln!("Panic: {}", info)));

    println!("osmpdrum server starting...");

    let engine = AudioEngine::new()?;
    let engine = Arc::new(Mutex::new(engine));

    let (event_tx, _) = broadcast::channel::<String>(256);
    let midi_conn: Arc<Mutex<Option<MidiInputConnection<()>>>> = Arc::new(Mutex::new(None));

    let osmp_player: Arc<Mutex<Option<player::OsmpPlayer>>> = Arc::new(Mutex::new(None));

    // Reconnect saved MIDI port on startup
    {
        let saved = engine.lock().unwrap().get_settings_snapshot().midi_input_port.clone();
        if let Some(ref port) = saved {
            let tx   = event_tx.clone();
            let (bufs, vids) = {
                let eng = engine.lock().unwrap();
                (eng.buffers.clone(), eng.next_voice_id.clone())
            };
            if let Some(conn) = connect_midi_input(port, tx, bufs, vids, osmp_player.clone()) {
                *midi_conn.lock().unwrap() = Some(conn);
                println!("MIDI auto-connected: {}", port);
            }
        }
    }

    let state = AppState { engine, event_tx: event_tx.clone(), midi_conn, osmp_player };

    println!("Serving embedded frontend ({} bytes)", FRONTEND_HTML.len());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback(serve_frontend)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 7878));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    // Open browser after a short delay so the listener is ready
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if let Err(e) = open::that("http://127.0.0.1:7878") {
            eprintln!("Could not open browser: {}", e);
        }
    });

    axum::serve(listener, app).await?;
    Ok(())
}
