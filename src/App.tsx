
import React, { useState, useRef, useEffect } from 'react';
import { Layout, Music, Settings, HelpCircle, FileAudio, Folder } from 'lucide-react';
import { WaveformDisplay } from './components/WaveformDisplay';
import { Knob } from './components/Knob';
import { PadGrid } from './components/PadGrid';
import { EffectsRack } from './components/EffectsRack';
import { PadData } from './types';
import Logo from './assets/logo.svg';
import { audioEngine } from './lib/audioEngine';

const App: React.FC = () => {
  const [selectedPadId, setSelectedPadId] = useState<number>(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playProgress, setPlayProgress] = useState(0);
  const [masterVolume, setMasterVolume] = useState(100);
  const animationFrameRef = useRef<number>(0);
  const playStartTimeRef = useRef<number>(0);

  // Mock data for pads
  const [pads, setPads] = useState<PadData[]>(
    Array.from({ length: 32 }).map((_, i) => ({
      id: i,
      label: '',
      isMuted: false,
      isSolo: false,
      isActive: i === 0,
      filePath: undefined,
      waveformPeaks: undefined,
      duration: 0
    }))
  );

  const [envelope, setEnvelope] = useState({
    attack: 30,
    decay: 50,
    sustain: 80
  });

  // Handle master volume changes
  const handleMasterVolumeChange = (value: number) => {
    setMasterVolume(value);
    audioEngine.setMasterVolume(value / 100);
  };

  // Listen for Rust events
  useEffect(() => {
    const handleDrop = (e: CustomEvent) => {
      const { path } = e.detail;
      if (!path) return;

      // Load to selected pad
      // Update state first to store path
      setPads(prev => prev.map(p => {
        if (p.id === selectedPadId) {
          // Extract filename from path for label
          const name = path.split(/[\\/]/).pop() || 'SAMPLE';
          const cleanName = name.replace(/\.(wav|mp3|flac|ogg)$/i, '');
          return {
            ...p,
            filePath: path,
            label: cleanName.substring(0, 8).toUpperCase()
          };
        }
        return p;
      }));

      // Trigger Rust to process waveform
      audioEngine.load(selectedPadId, path);
    };

    const handleWaveformReady = (e: CustomEvent) => {
      const data = e.detail; // { pad_id, peaks, duration }
      if (!data) return;

      setPads(prev => prev.map(p => {
        if (p.id === data.pad_id) {
          return {
            ...p,
            waveformPeaks: data.peaks,
            duration: data.duration
          };
        }
        return p;
      }));
    };

    window.addEventListener('rust-file-drop', handleDrop as any);
    window.addEventListener('rust-waveform-ready', handleWaveformReady as any);

    return () => {
      window.removeEventListener('rust-file-drop', handleDrop as any);
      window.removeEventListener('rust-waveform-ready', handleWaveformReady as any);
    };
  }, [selectedPadId]);

  const handlePadSelect = (id: number) => {
    setSelectedPadId(id);
    setPads(prev => prev.map(p => ({
      ...p,
      isActive: p.id === id
    })));
  };

  const handlePadToggle = (id: number, type: 'mute' | 'solo') => {
    setPads(prev => prev.map(p => {
      if (p.id !== id) return p;
      return {
        ...p,
        isMuted: type === 'mute' ? !p.isMuted : p.isMuted,
        isSolo: type === 'solo' ? !p.isSolo : p.isSolo
      };
    }));
  };

  // Deprecated direct file load (handled by Rust drop)
  const handleFileLoadToPad = (padId: number, buffer: AudioBuffer, fileName: string) => {
    // No-op or reimplement if needed for browser-based drop (not used now)
  };

  const playPad = (padId: number) => {
    const pad = pads[padId];
    if (!pad.filePath || pad.isMuted) return;

    // Trigger backend
    audioEngine.play(padId, pad.filePath, 1.0, 0.0);

    // Stop previous animation
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }

    playStartTimeRef.current = performance.now() / 1000;
    const duration = pad.duration || 1.0;

    setIsPlaying(true);
    setPlayProgress(0);

    const animate = () => {
      const now = performance.now() / 1000;
      const elapsed = now - playStartTimeRef.current;
      const progress = Math.min(elapsed / duration, 1);

      setPlayProgress(progress);

      if (progress < 1) {
        animationFrameRef.current = requestAnimationFrame(animate);
      } else {
        setIsPlaying(false);
        setPlayProgress(0);
      }
    };

    animate();
  };

  // Keyboard support
  useEffect(() => {
    const handleKeyPress = (e: KeyboardEvent) => {
      const keyMap: { [key: string]: number } = {
        'q': 0, 'w': 1, 'e': 2, 'r': 3, 't': 4, 'y': 5, 'u': 6, 'i': 7,
        'a': 8, 's': 9, 'd': 10, 'f': 11, 'g': 12, 'h': 13, 'j': 14, 'k': 15,
        'z': 16, 'x': 17, 'c': 18, 'v': 19, 'b': 20, 'n': 21, 'm': 22, ',': 23
      };

      const padId = keyMap[e.key.toLowerCase()];
      if (padId !== undefined) {
        playPad(padId);
      }
    };

    window.addEventListener('keydown', handleKeyPress);
    return () => window.removeEventListener('keydown', handleKeyPress);
  }, [pads]);

  const selectedPad = pads[selectedPadId];

  return (
    <div className="bg-app-bg text-text-main font-mona h-screen flex flex-col overflow-hidden select-none text-sm">
      {/* Header */}
      <header className="h-10 bg-app-bg border-b border-border-dark flex items-center px-4 shrink-0 z-20">
        <div className="font-extrabold tracking-tight mr-8 text-base flex items-center">
          <img src={Logo} alt="" />
        </div>
        <nav className="flex gap-5 text-xs font-semibold text-text-muted">
          <button className="hover:text-text-main transition-colors focus:outline-none">FILE</button>
          <button className="hover:text-text-main transition-colors focus:outline-none">EDIT</button>
          <button className="hover:text-text-main transition-colors focus:outline-none">TOOL</button>
          <button className="hover:text-text-main transition-colors focus:outline-none">HELP</button>
        </nav>
      </header>

      <div className="flex flex-1 overflow-hidden">

        {/* Sidebar */}
        <aside className="w-64 bg-panel-bg border-r border-border-dark flex flex-col shrink-0">
          <div className="px-3 py-2 text-xs text-text-muted bg-header-bg border-b border-border-dark font-semibold uppercase tracking-wider">
            File Browser
          </div>
          <div className="p-3 text-xs text-text-muted space-y-1 overflow-y-auto flex-1">
            <div className="flex items-center gap-2 cursor-pointer hover:text-text-main transition-colors">
              <Folder size={14} className="text-gray-500" />
              <span>Drums_Vol_1</span>
            </div>
            <div className="px-4 py-2 text-gray-600 italic">
              Drag files directly from OS into the window
            </div>
          </div>
        </aside>

        {/* Main Content */}
        <main className="flex-1 flex flex-col bg-app-bg min-w-0">

          {/* Top Control Panel */}
          <div className="h-control-h border-border-dark flex shrink-0">

            {/* Waveform Section */}
            <div className="flex-1 flex flex-col border-r border-border-dark min-w-0 relative group">
              <div className="absolute top-0 left-0 bg-accent-cyan text-black px-2.5 py-1 text-xs font-bold z-10">
                {selectedPad?.label || 'EMPTY_SLOT'}
              </div>
              <WaveformDisplay
                color="text-accent-cyan"
                peaks={selectedPad?.waveformPeaks}
                isPlaying={isPlaying && selectedPad?.filePath !== undefined}
                playProgress={playProgress}
              />
              <div className="flex justify-between text-[10px] text-text-main px-2 py-0.5 border-t border-border-dark bg-header-bg">
                <span className="font-mono opacity-70">
                  Start: 000000 ms
                </span>
                <span className="font-mono opacity-70">
                  Duration: {selectedPad?.duration ? selectedPad.duration.toFixed(3) : '0.000'} s
                </span>
              </div>
            </div>

            {/* Envelope Section */}
            <div className="w-96 bg-panel-bg border-r border-border-dark flex flex-col shrink-0">
              <div className="bg-header-bg text-text-muted text-[10px] h-7 font-bold px-2 py-1 border-b border-border-dark uppercase tracking-wider">
                Envelope Generator
              </div>

              <div className="flex items-center justify-around flex-1 px-2 pb-2">
                <Knob
                  label="Attack"
                  value={envelope.attack}
                  onChange={(v) => setEnvelope(e => ({ ...e, attack: v }))}
                  suffix="ms"
                />
                <Knob
                  label="Decay"
                  value={envelope.decay}
                  onChange={(v) => setEnvelope(e => ({ ...e, decay: v }))}
                  suffix="ms"
                />
                <Knob
                  label="Sustain"
                  value={envelope.sustain}
                  onChange={(v) => setEnvelope(e => ({ ...e, sustain: v }))}
                  suffix="%"
                />
              </div>
            </div>

            {/* Options Section */}
            <div className="w-52 bg-panel-bg border-r border-border-dark text-xs flex flex-col shrink-0">
              <div className="bg-header-bg text-text-muted text-[10px] h-7 font-bold px-2 py-1 border-b border-border-dark uppercase tracking-wider">
                Sample Properties
              </div>
              <div className="p-3 space-y-2">
                <div className="flex justify-between items-center">
                  <span className="text-text-muted">Root Note</span>
                  <span className="text-accent-cyan font-mono bg-black/30 px-1 rounded">C3</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-text-muted">Tune</span>
                  <span className="text-accent-cyan font-mono bg-black/30 px-1 rounded">+0 st</span>
                </div>
              </div>
            </div>

            {/* Master Volume Section */}
            <div className="w-32 bg-panel-bg flex flex-col shrink-0">
              <div className="bg-header-bg text-text-muted text-[10px] h-7 font-bold px-2 py-1 border-b border-border-dark uppercase tracking-wider">
                Master
              </div>
              <div className="flex items-center justify-center flex-1 px-2 pb-2">
                <Knob
                  label="Volume"
                  value={masterVolume}
                  onChange={handleMasterVolumeChange}
                  suffix="%"
                />
              </div>
            </div>
          </div>

          {/* Pads Grid */}
          <PadGrid
            pads={pads}
            onSelect={handlePadSelect}
            onToggle={handlePadToggle}
            onPlay={playPad}
            onFileLoadToPad={handleFileLoadToPad as any}
          />

          {/* Bottom Effects Rack */}
          <EffectsRack />

        </main>
      </div>
    </div>
  );
};

export default App;