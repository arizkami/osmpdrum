#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
use wry::{WebView, http::Request};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "command", content = "payload")]
enum AudioCommand {
    Play { pad_id: usize, file_path: String, volume: f32, pan: f32 },
    Stop { pad_id: usize },
    Load { pad_id: usize, file_path: String },
    SetMasterVolume { volume: f32 },
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
}

impl AudioBuffer {
    fn new(samples: Vec<f32>, volume: f32) -> Self {
        Self {
            samples,
            position: 0,
            volume,
            playing: true,
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
    device: Device,
    config: StreamConfig,
    buffers: Arc<Mutex<HashMap<usize, AudioBuffer>>>,
    stream: Option<Stream>,
    master_volume: Arc<Mutex<f32>>,
}

impl AudioEngine {
    fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;
        
        let config = device.default_output_config()?;
        let config: StreamConfig = config.into();
        
        let buffers: Arc<Mutex<HashMap<usize, AudioBuffer>>> = Arc::new(Mutex::new(HashMap::new()));
        let master_volume = Arc::new(Mutex::new(1.0f32));
        
        Ok(Self {
            device,
            config,
            buffers,
            stream: None,
            master_volume,
        })
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
                let mut buffers = buffers.lock().unwrap();
                let master_vol = *master_volume.lock().unwrap();
                
                for frame in data.chunks_mut(channels) {
                    let mut mixed_sample = 0.0f32;
                    
                    // Mix all playing buffers
                    for buffer in buffers.values_mut() {
                        mixed_sample += buffer.next_sample();
                    }
                    
                    // Apply master volume with 2x gain boost
                    mixed_sample *= master_vol * 2.0;
                    
                    // Clamp to prevent distortion
                    mixed_sample = mixed_sample.clamp(-1.0, 1.0);
                    
                    // Write to all channels
                    for sample in frame.iter_mut() {
                        *sample = mixed_sample;
                    }
                }
                
                // Remove finished buffers
                buffers.retain(|_, buffer| !buffer.is_finished());
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

        // Load WAV file
        let samples = load_wav_file(file_path, self.config.sample_rate)?;
        println!("Loaded {} samples from {}", samples.len(), file_path);
        
        let buffer = AudioBuffer::new(samples, volume);
        
        let mut buffers = self.buffers.lock().unwrap();
        buffers.insert(pad_id, buffer);
        println!("Playing pad {} with {} active buffers", pad_id, buffers.len());
        
        Ok(())
    }

    fn stop(&mut self, pad_id: usize) {
        let mut buffers = self.buffers.lock().unwrap();
        if let Some(buffer) = buffers.get_mut(&pad_id) {
            buffer.stop();
        }
        buffers.remove(&pad_id);
    }

    fn set_master_volume(&mut self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        *self.master_volume.lock().unwrap() = clamped;
        println!("Master volume set to {}", clamped);
    }
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
}

struct App {
    window: Option<Window>,
    webview: Option<WebView>,
    audio_engine: Option<Arc<Mutex<AudioEngine>>>,
    event_rx: Option<Receiver<AppEvent>>,
    event_tx: Option<Sender<AppEvent>>,
    html_content: String,
    is_ready: bool,
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

        let ipc_engine = engine.clone();
        let ipc_tx = self.event_tx.as_ref().unwrap().clone();

        let handler = move |req: Request<String>| {
            if let Ok(command) = serde_json::from_str::<AudioCommand>(&req.body()) {
                println!("IPC Command: {:?}", command);
                
                match command {
                    AudioCommand::Play { pad_id, file_path, volume, pan } => {
                        if let Ok(mut eng) = ipc_engine.lock() {
                            if let Err(e) = eng.play(pad_id, &file_path, volume, pan) {
                                eprintln!("Error playing: {}", e);
                            }
                        }
                    },
                    AudioCommand::Stop { pad_id } => {
                        if let Ok(mut eng) = ipc_engine.lock() {
                            eng.stop(pad_id);
                        }
                    },
                    AudioCommand::SetMasterVolume { volume } => {
                        if let Ok(mut eng) = ipc_engine.lock() {
                            eng.set_master_volume(volume);
                        }
                    },
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
                                            _ => vec![],
                                        }
                                    }
                                };
                                
                                // Convert stereo to mono for waveform display
                                let mono_samples: Vec<f32> = if spec.channels == 2 {
                                    samples.chunks(2)
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
                                
                                println!("Waveform generated: {} peaks, duration: {}s", peaks.len(), duration);
                                
                                let data = WaveformData {
                                    pad_id,
                                    peaks,
                                    duration
                                };
                                
                                let _ = tx_clone.send(AppEvent::WaveformReady(data));
                            }
                        });
                    }
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
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
                    }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
