# 🥁 Opensampler Drummer (osmpdrum)

A lightweight, high-performance desktop drum sampler application. Built with a custom **Rust** native audio engine and a modern **React + TypeScript** frontend, packaged together using a lean WebView (`wry` + `winit`) 
## ✨ Features

### Audio Engine (Rust Backend)
* **Low-Latency Playback**: Built on `cpal` for direct, raw access to the system's native audio output.
* **Polyphonic Mixing**: Supports multiple overlapping drum voices simultaneously.
* **Soft Clipping Protection**: Built-in audio limiting to prevent harsh digital distortion when stacking multiple loud samples.
* **Zero-Latency Caching**: Audio files are parsed (`hound`) and cached in memory upon first load for instant playback.
* **Native Waveform Analysis**: The backend calculates peak data in background threads and streams it to the frontend.

### User Interface (React Frontend)
* **32-Pad Grid System**: Interactive drum pads with Mute, Solo, and Active states.
* **OS Drag & Drop**: Seamlessly drop `.wav`, `.mp3`, or `.ogg` files directly from your file explorer onto any pad.
* **Interactive Waveforms**: Real-time audio visualization using **D3.js** with draggable start/end points and a synchronized playhead.
* **Hardware-Style Controls**: Custom rotary knob components for Envelope (Attack/Decay/Sustain) and Master Volume.
* **Keyboard Integration**: Play the 32 pads instantly using your computer keyboard (QWERTY layout).
* **Unsaved Changes Protection**: Native exit confirmation dialog intercepts accidental window closures if work is unsaved.

## 🛠️ Tech Stack

**Backend (Core & Audio)**
* [Rust](https://www.rust-lang.org/) - Core logic and audio processing.
* `cpal` - Cross-platform audio I/O.
* `hound` - WAV encoding/decoding.
* `wry` & `winit` - Minimalist WebView windowing.
* `serde` - IPC message serialization.

**Frontend (UI)**
* [React 18](https://reactjs.org/) + [TypeScript](https://www.typescriptlang.org/)
* [Tailwind CSS](https://tailwindcss.com/) - Utility-first styling with a dark-mode studio theme.
* [D3.js](https://d3js.org/) - SVG waveform rendering and playhead animation.
* [Lucide React](https://lucide.dev/) - UI Iconography.

## 🏗️ Architecture & IPC

The application communicates between the Rust host and the WebView frontend using a lightweight JSON protocol:

1. **Frontend to Backend**: Sends commands via `window.ipc.postMessage()` (e.g., `Play`, `Stop`, `Load`, `SetMasterVolume`, `ConfirmExit`).
2. **Backend to Frontend**: Dispatches CustomEvents to the `window` object (e.g., `rust-file-drop`, `rust-waveform-ready`).

## 🎹 Keyboard Mapping

Play the pads using your keyboard's typing keys:
* **Row 1 (Pads 1-8):** `Q W E R T Y U I`
* **Row 2 (Pads 9-16):** `A S D F G H J K`
* **Row 3 (Pads 17-24):** `Z X C V B N M ,`

## 🚀 Getting Started

### Prerequisites
* Rust toolchain (`cargo`)
* Node.js and npm/yarn (for building the frontend)

### Build & Run

1. **Build the Frontend**
   Ensure the frontend is built and output to the `dist` folder, as the Rust backend expects to load `dist/index.html`.
   ```bash
   bun install
   bun run build
    ```

2. **Run the Application**
It is highly recommended to run the app in release mode to ensure the audio thread performs without stuttering.
```bash
cargo run --release

```
