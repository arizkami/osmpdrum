#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::BufferSize;
use cpal::{Device, Stream, StreamConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::sync::OnceLock;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use wry::{WebView, http::Request};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallWindowProcW, CreateMenu, CreatePopupMenu, DefWindowProcW, DrawMenuBar,
    GetWindowLongPtrW, SetMenu, SetWindowLongPtrW, GWLP_WNDPROC, MF_POPUP, MF_STRING, WM_COMMAND,
    WNDPROC,
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", content = "payload")]
enum AudioCommand {
    Play { pad_id: usize, file_path: String, volume: f32, pan: f32 },
    Stop { pad_id: usize },
    Load { pad_id: usize, file_path: String },
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            playback_latency_ms: 50,
            playback_backend: None,
            playback_device_name: None,
            buffer_size_frames: None,
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
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
struct WaveformData {
    pad_id: usize,
    peaks: Vec<f32>,
    duration: f32,
}

struct AudioBuffer {
    samples: Vec<f32>,
    position: usize,
    volume: f32,
    playing: bool,
    pad_id: usize,
    voice_id: usize,
}

impl AudioBuffer {
    fn new(samples: Vec<f32>, volume: f32, pad_id: usize, voice_id: usize) -> Self {
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

        let config = device.default_output_config()?;
        let config: StreamConfig = config.into();
        let config = apply_stream_tuning_to_config(config, &settings);

        self.host_id = host_id;
        self.device = device;
        self.config = config;

        Ok(())
    }

    fn restart_stream(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.start_stream()
    }

    fn start_stream(&mut self) -> Result<()> {
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
        
        let buffer = AudioBuffer::new(samples, volume, pad_id, voice_id);
        
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
        Some(s) if s == "ks" => cpal::HostId::Wasapi,
        Some(s) if s == "wasapi" => cpal::HostId::Wasapi,
        _ => cpal::default_host().id(),
    }
}

fn available_backends() -> Vec<String> {
    let mut out = Vec::new();
    let mut has_wasapi = false;
    for host_id in cpal::available_hosts() {
        // These are the only ones you asked to expose in UI.
        match host_id {
            cpal::HostId::Wasapi => {
                has_wasapi = true;
                out.push("WASAPI".to_string());
            }
            _ => {}
        }
    }

    if has_wasapi {
        out.push("KS".to_string());
    }
    out
}

#[cfg(target_os = "windows")]
const MENU_ID_AUDIO_SETTINGS: usize = 1001;

#[cfg(target_os = "windows")]
static MENU_TX: OnceLock<Sender<AppEvent>> = OnceLock::new();

#[cfg(target_os = "windows")]
static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();

#[cfg(target_os = "windows")]
unsafe extern "system" fn menu_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_COMMAND {
        let id = (wparam & 0xffff) as usize;
        if id == MENU_ID_AUDIO_SETTINGS {
            if let Some(tx) = MENU_TX.get() {
                let _ = tx.send(AppEvent::OpenAudioSettings);
            }
            return 0;
        }
    }

    if let Some(prev) = ORIGINAL_WNDPROC.get() {
        let prev: WNDPROC = std::mem::transmute(*prev);
        return CallWindowProcW(prev, hwnd, msg, wparam, lparam);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
unsafe fn install_native_menu(window: &Window, tx: Sender<AppEvent>) {
    let hwnd = match window.window_handle().ok().map(|h| h.as_raw()) {
        Some(RawWindowHandle::Win32(h)) => h.hwnd.get() as HWND,
        _ => return,
    };
    let _ = MENU_TX.set(tx);

    // Subclass window proc to receive WM_COMMAND.
    let prev = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
    let _ = ORIGINAL_WNDPROC.set(prev);
    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, menu_wndproc as isize);

    let menu = CreateMenu();
    let file_menu = CreatePopupMenu();

    let audio_settings = to_wide("Audio Settings...");
    AppendMenuW(file_menu, MF_STRING, MENU_ID_AUDIO_SETTINGS, audio_settings.as_ptr());

    let file_label = to_wide("File");
    AppendMenuW(menu, MF_POPUP, file_menu as usize, file_label.as_ptr());

    SetMenu(hwnd, menu);
    DrawMenuBar(hwnd);
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

enum AppEvent {
    FileDropped { path: String, x: f64, y: f64 },
    WaveformReady(WaveformData),
    CloseRequested,
    OpenAudioSettings,
    AudioSettings { settings: AppSettings },
    AudioBackends { backends: Vec<String> },
    AudioDevices { devices: Vec<String> },
}

struct App {
    window: Option<Window>,
    webview: Option<WebView>,
    audio_engine: Option<Arc<Mutex<AudioEngine>>>,
    event_rx: Option<Receiver<AppEvent>>,
    event_tx: Option<Sender<AppEvent>>,
    html_content: String,
    is_ready: bool,
    pending_close: bool,
}

impl App {
    fn new() -> Result<Self> {
        let (tx, rx) = channel();
        
        // Pre-load HTML content at startup
        let html_content = include_str!("../../dist/index.html").to_string();
        println!("[Init] HTML content pre-loaded ({} bytes)", html_content.len());
        
        Ok(Self {
            window: None,
            webview: None,
            audio_engine: None,
            event_rx: Some(rx),
            event_tx: Some(tx),
            html_content,
            is_ready: false,
            pending_close: false,
        })
    }
    
    fn initialize_audio_engine(&mut self) -> Result<Arc<Mutex<AudioEngine>>> {
        let start = std::time::Instant::now();
        
        let engine = AudioEngine::new()?;
        let engine = Arc::new(Mutex::new(engine));
        
        println!("[Init] Audio engine ready ({:?})", start.elapsed());
        Ok(engine)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let total_start = std::time::Instant::now();

        // Pre-initialize audio engine before creating window
        println!("[Init] Initializing audio engine...");
        let engine = match self.initialize_audio_engine() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Failed to initialize audio engine: {}", e);
                return;
            }
        };
        
        self.audio_engine = Some(engine.clone());

        // Create window (hidden)
        println!("[Init] Creating window...");
        let window_attributes = Window::default_attributes()
            .with_title("Opensampler Drummer")
            .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 800.0))
            .with_visible(false); // Keep hidden until fully loaded
        
        let window = match event_loop.create_window(window_attributes) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create window: {}", e);
                return;
            }
        };

        #[cfg(target_os = "windows")]
        {
            // Native Win32 menu is intentionally disabled.
            // Menus are implemented in the React headerbar.
        }

        let ipc_engine = engine.clone();
        let ipc_tx = self.event_tx.as_ref().unwrap().clone();

        let handler = move |req: Request<String>| {
            let command = match serde_json::from_str::<AudioCommand>(&req.body()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to parse IPC command: {}. Body: {}", e, req.body());
                    return;
                }
            };

            println!("IPC Command: {:?}", command);
            match command {
                AudioCommand::Play { pad_id, file_path, volume, pan } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        if let Err(e) = eng.play(pad_id, &file_path, volume, pan) {
                            eprintln!("Error playing: {}", e);
                        }
                    }
                }
                AudioCommand::Stop { pad_id } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        eng.stop(pad_id);
                    }
                }
                AudioCommand::SetMasterVolume { volume } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        eng.set_master_volume(volume);
                    }
                }
                AudioCommand::SetPlaybackLatency { latency_ms } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        eng.set_playback_latency_ms(latency_ms);
                    }
                }
                AudioCommand::SetBufferSizeFrames { frames } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        eng.set_buffer_size_frames(frames);
                    }
                }
                AudioCommand::GetAudioSettings {} => {
                    if let Ok(eng) = ipc_engine.lock() {
                        let settings = eng.get_settings_snapshot();
                        let _ = ipc_tx.send(AppEvent::AudioSettings { settings });
                    }
                }
                AudioCommand::SetPlaybackBackend { backend } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        eng.set_playback_backend(&backend);
                    }
                }
                AudioCommand::SetPlaybackDevice { device_name } => {
                    if let Ok(mut eng) = ipc_engine.lock() {
                        eng.set_playback_device(&device_name);
                    }
                }
                AudioCommand::GetAudioBackends {} => {
                    let backends = available_backends();
                    let _ = ipc_tx.send(AppEvent::AudioBackends { backends });
                }
                AudioCommand::GetAudioDevices { backend } => {
                    let devices = available_output_devices(&backend);
                    let _ = ipc_tx.send(AppEvent::AudioDevices { devices });
                }
                AudioCommand::Load { pad_id, file_path } => {
                    let tx_clone = ipc_tx.clone();
                    thread::spawn(move || {
                        if let Ok(mut reader) = hound::WavReader::open(&file_path) {
                            let spec = reader.spec();
                            let duration = reader.duration() as f32 / spec.sample_rate as f32;

                            let samples: Vec<f32> = match spec.sample_format {
                                hound::SampleFormat::Float => {
                                    reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
                                }
                                hound::SampleFormat::Int => match spec.bits_per_sample {
                                    16 => reader
                                        .samples::<i16>()
                                        .map(|s| s.unwrap_or(0) as f32 / 32768.0)
                                        .collect(),
                                    24 => reader
                                        .samples::<i32>()
                                        .map(|s| s.unwrap_or(0) as f32 / 8388608.0)
                                        .collect(),
                                    32 => reader
                                        .samples::<i32>()
                                        .map(|s| s.unwrap_or(0) as f32 / 2147483648.0)
                                        .collect(),
                                    _ => vec![],
                                },
                            };

                            let mono_samples: Vec<f32> = if spec.channels == 2 {
                                samples
                                    .chunks(2)
                                    .map(|chunk| (chunk[0] + chunk.get(1).unwrap_or(&0.0)) / 2.0)
                                    .collect()
                            } else {
                                samples
                            };

                            let total_samples = mono_samples.len();
                            let points = 200;
                            let chunk_size = (total_samples / points).max(1);
                            let mut peaks = Vec::with_capacity(points);

                            for chunk in mono_samples.chunks(chunk_size) {
                                let max = chunk.iter().fold(0.0f32, |a, b| a.max(b.abs()));
                                peaks.push(max);
                            }

                            println!(
                                "Waveform generated: {} peaks, duration: {}s",
                                peaks.len(),
                                duration
                            );

                            let data = WaveformData {
                                pad_id,
                                peaks,
                                duration,
                            };

                            let _ = tx_clone.send(AppEvent::WaveformReady(data));
                        }
                    });
                }
                AudioCommand::CheckUnsavedChanges => {
                    // This is handled by frontend
                }
                AudioCommand::ConfirmExit => {
                    std::process::exit(0);
                }
            }
        };

        let drop_tx = self.event_tx.as_ref().unwrap().clone();
        
        // Build webview
        println!("[Init] Building webview...");

        let webview_builder = wry::WebViewBuilder::new()
            .with_html(&self.html_content)
            .with_ipc_handler(handler)
            .with_initialization_script(
                r#"
                console.log('[Preload] WebView initialized');
                window.__OSMP_PRELOAD_TIME__ = Date.now();
                "#
            )
            .with_drag_drop_handler(move |event| {
                match event {
                    wry::DragDropEvent::Drop { paths, position, .. } => {
                        if let Some(path) = paths.first() {
                            let path_str = path.to_string_lossy().to_string();
                            let _ = drop_tx.send(AppEvent::FileDropped { 
                                path: path_str, 
                                x: position.0 as f64, 
                                y: position.1 as f64 
                            });
                        }
                    }
                    _ => {}
                }
                true 
            });

        let webview = match webview_builder.build(&window) {
            Ok(wv) => wv,
            Err(e) => {
                eprintln!("Failed to create webview: {}", e);
                return;
            }
        };
        
        println!("[Init] Webview created");
        
        #[cfg(debug_assertions)]
        let _ = webview.open_devtools();

        self.window = Some(window);
        self.webview = Some(webview);
        self.is_ready = true;
        
        // Show window now that everything is ready
        if let Some(window) = &self.window {
            window.set_visible(true);
            println!("[Init] Window shown - Total startup: {:?}", total_start.elapsed());
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Don't close immediately, send event to check for unsaved changes
                if let Some(tx) = &self.event_tx {
                    let _ = tx.send(AppEvent::CloseRequested);
                }
            }
            _ => (),
        }
    }
    
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(rx) = &self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    AppEvent::FileDropped { path, x, y } => {
                        let path_esc = path.replace("\\", "\\\\");
                        let js = format!(
                            "window.dispatchEvent(new CustomEvent('rust-file-drop', {{ detail: {{ path: '{}', x: {}, y: {} }} }}));", 
                            path_esc, x, y
                        );
                        if let Some(webview) = &self.webview {
                            let _ = webview.evaluate_script(&js);
                        }
                    },
                    AppEvent::WaveformReady(data) => {
                        if let Ok(json) = serde_json::to_string(&data) {
                            let js = format!(
                                "window.dispatchEvent(new CustomEvent('rust-waveform-ready', {{ detail: {} }}));",
                                json
                            );
                            if let Some(webview) = &self.webview {
                                let _ = webview.evaluate_script(&js);
                            }
                        }
                    },
                    AppEvent::CloseRequested => {
                        // Check for unsaved changes and show dialog if needed
                        if let Some(webview) = &self.webview {
                            let js = r#"
                                (function() {
                                    const hasChanges = window.__hasUnsavedChanges || false;
                                    if (hasChanges && window.__showExitDialog) {
                                        window.__showExitDialog();
                                    } else {
                                        window.ipc.postMessage(JSON.stringify({ command: 'ConfirmExit' }));
                                    }
                                })();
                            "#;
                            let _ = webview.evaluate_script(js);
                        }
                    }
                    AppEvent::OpenAudioSettings => {
                        if let Some(webview) = &self.webview {
                            let js = "window.dispatchEvent(new CustomEvent('rust-open-audio-settings'));";
                            let _ = webview.evaluate_script(js);
                        }
                    }
                    AppEvent::AudioBackends { backends } => {
                        if let Ok(json) = serde_json::to_string(&backends) {
                            let js = format!(
                                "window.dispatchEvent(new CustomEvent('rust-audio-backends', {{ detail: {} }}));",
                                json
                            );
                            if let Some(webview) = &self.webview {
                                let _ = webview.evaluate_script(&js);
                            }
                        }
                    }
                    AppEvent::AudioSettings { settings } => {
                        if let Ok(json) = serde_json::to_string(&settings) {
                            let js = format!(
                                "window.dispatchEvent(new CustomEvent('rust-audio-settings', {{ detail: {} }}));",
                                json
                            );
                            if let Some(webview) = &self.webview {
                                let _ = webview.evaluate_script(&js);
                            }
                        }
                    }
                    AppEvent::AudioDevices { devices } => {
                        if let Ok(json) = serde_json::to_string(&devices) {
                            let js = format!(
                                "window.dispatchEvent(new CustomEvent('rust-audio-devices', {{ detail: {} }}));",
                                json
                            );
                            if let Some(webview) = &self.webview {
                                let _ = webview.evaluate_script(&js);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Panic: {}", info);
    }));
    let event_loop = EventLoop::new()?;
    let mut app = App::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
