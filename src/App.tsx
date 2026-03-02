import React, { useState, useRef, useEffect } from 'react';
import { Layout, Music, Settings, HelpCircle, FileAudio } from 'lucide-react';
import { WaveformDisplay } from './components/WaveformDisplay';
import { Knob } from './components/Knob';
import { PadGrid } from './components/PadGrid';
import { EffectsRack } from './components/EffectsRack';
import { Dialog, DialogFooter, DialogButton } from './components/Dialog';
import { DropdownMenu } from './components/DropdownMenu';
import { MenuSelect } from './components/MenuSelect';
import { VUMeter } from './components/VUMeter';
import { Sidebar } from './components/Sidebar';
import { PadData } from './types';
import { audioEngine } from './lib/audioEngine';

// don't remove logo and skip error!
import Logo from './assets/logo.svg';

const App: React.FC = () => {
  const [selectedPadId, setSelectedPadId] = useState<number>(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playProgress, setPlayProgress] = useState(0);
  const [masterVolume, setMasterVolume] = useState(100);
  const [playbackLatencyMs, setPlaybackLatencyMs] = useState(50);
  const [showAudioSettings, setShowAudioSettings] = useState(false);
  const [audioBackends, setAudioBackends] = useState<string[]>([]);
  const [selectedBackend, setSelectedBackend] = useState<string>('');
  const [audioDevices, setAudioDevices] = useState<string[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string>('');
  const [bufferSizeFrames, setBufferSizeFrames] = useState(0);
  const [midiInputPorts, setMidiInputPorts] = useState<string[]>([]);
  const [selectedMidiInput, setSelectedMidiInput] = useState<string>('');
  const [wasapiExclusive, setWasapiExclusive] = useState(false);
  const [sampleRate, setSampleRate] = useState(0);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [showExitDialog, setShowExitDialog] = useState(false);
  const [vuLevel, setVuLevel] = useState(0);
  const animationFrameRef = useRef<number>(0);
  const playStartTimeRef = useRef<number>(0);

  const safeGetLocalStorageNumber = (key: string): number | undefined => {
    try {
      const stored = window.localStorage.getItem(key);
      if (stored == null) return undefined;
      const parsed = Number(stored);
      return Number.isFinite(parsed) ? parsed : undefined;
    } catch {
      return undefined;
    }
  };

  const safeSetLocalStorage = (key: string, value: string) => {
    try {
      window.localStorage.setItem(key, value);
    } catch {
      // Ignore: localStorage may be blocked in Wry/about:blank origin.
    }
  };

  useEffect(() => {
    const initial = safeGetLocalStorageNumber('osmpdrum.playbackLatencyMs') ?? 50;
    const clamped = Math.max(5, Math.min(500, Math.round(initial)));
    setPlaybackLatencyMs(clamped);
    audioEngine.setPlaybackLatency(clamped);
  }, []);

  const openAudioSettings = () => {
    setShowAudioSettings(true);
    audioEngine.getAudioSettings();
    audioEngine.getAudioBackends();
    audioEngine.getMidiInputs();
    if (selectedBackend) {
      audioEngine.getAudioDevices(selectedBackend);
    }
  };

  const applyMidiInput = (portName: string) => {
    setSelectedMidiInput(portName);
    audioEngine.setMidiInput(portName || null);
  };

  const applyBackend = (backend: string) => {
    setSelectedBackend(backend);
    setSelectedDevice('');
    setAudioDevices([]);
    audioEngine.setPlaybackBackend(backend);
    audioEngine.getAudioDevices(backend);
  };

  const applyDevice = (deviceName: string) => {
    setSelectedDevice(deviceName);
    audioEngine.setPlaybackDevice(deviceName);
  };

  const applyBufferFrames = (frames: number) => {
    const clamped = Math.max(0, Math.min(8192, Math.round(frames)));
    setBufferSizeFrames(clamped);
    audioEngine.setBufferSizeFrames(clamped);
  };

  const applyWasapiExclusive = (exclusive: boolean) => {
    setWasapiExclusive(exclusive);
    audioEngine.setWasapiExclusive(exclusive);
  };

  const applySampleRate = (rate: number) => {
    setSampleRate(rate);
    audioEngine.setSampleRate(rate);
  };

  useEffect(() => {
    const handleOpen = () => {
      openAudioSettings();
    };

    const handleBackends = (e: CustomEvent) => {
      const listRaw = Array.isArray(e.detail) ? (e.detail as any[]).filter(x => typeof x === 'string') as string[] : [];
      const list = listRaw.filter(x => x === 'WASAPI' || x === 'KS');
      setAudioBackends(list);
      if (!selectedBackend && list.length > 0) {
        setSelectedBackend(list[0]);
        audioEngine.getAudioDevices(list[0]);
      }
    };

    const handleDevices = (e: CustomEvent) => {
      const listRaw = Array.isArray(e.detail) ? (e.detail as any[]).filter(x => typeof x === 'string') as string[] : [];
      setAudioDevices(listRaw);
      if (!selectedDevice && listRaw.length > 0) {
        setSelectedDevice(listRaw[0]);
      }
    };

    const handleSettings = (e: CustomEvent) => {
      const s = (e.detail ?? {}) as any;
      const frames = typeof s.buffer_size_frames === 'number' ? s.buffer_size_frames : 0;
      setBufferSizeFrames(Number.isFinite(frames) ? frames : 0);
      if (typeof s.midi_input_port === 'string' && s.midi_input_port) {
        setSelectedMidiInput(s.midi_input_port);
      }
      if (typeof s.wasapi_exclusive === 'boolean') {
        setWasapiExclusive(s.wasapi_exclusive);
      }
      if (typeof s.sample_rate === 'number' && s.sample_rate > 0) {
        setSampleRate(s.sample_rate);
      }
    };

    const handleMidiInputs = (e: CustomEvent) => {
      const ports = Array.isArray(e.detail) ? (e.detail as string[]) : [];
      setMidiInputPorts(ports);
    };

    window.addEventListener('rust-open-audio-settings', handleOpen as any);
    window.addEventListener('rust-audio-backends', handleBackends as any);
    window.addEventListener('rust-audio-devices', handleDevices as any);
    window.addEventListener('rust-audio-settings', handleSettings as any);
    window.addEventListener('rust-midi-inputs', handleMidiInputs as any);
    return () => {
      window.removeEventListener('rust-open-audio-settings', handleOpen as any);
      window.removeEventListener('rust-audio-backends', handleBackends as any);
      window.removeEventListener('rust-audio-devices', handleDevices as any);
      window.removeEventListener('rust-audio-settings', handleSettings as any);
      window.removeEventListener('rust-midi-inputs', handleMidiInputs as any);
    };
  }, [selectedBackend, selectedDevice]);

  useEffect(() => {
    if (!showAudioSettings) return;
    audioEngine.getAudioSettings();
    audioEngine.getAudioBackends();
    audioEngine.getMidiInputs();
    if (selectedBackend) {
      audioEngine.getAudioDevices(selectedBackend);
    }
  }, [showAudioSettings, selectedBackend]);

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
      duration: 0,
      startPoint: 0,
      endPoint: 1
    }))
  );

  const [envelope, setEnvelope] = useState({
    attack: 30,
    decay: 50,
    sustain: 80
  });

  // Handle beforeunload for unsaved changes
  useEffect(() => {
    // Expose to window for Rust backend
    (window as any).__hasUnsavedChanges = hasUnsavedChanges;
    (window as any).__showExitDialog = () => {
      console.log('__showExitDialog called from Rust');
      setShowExitDialog(true);
    };

    const handleBeforeUnload = (e: BeforeUnloadEvent) => {
      if (hasUnsavedChanges) {
        e.preventDefault();
        e.returnValue = '';
        return '';
      }
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  }, [hasUnsavedChanges]);

  const handleConfirmExit = () => {
    console.log('Confirm exit clicked');
    audioEngine.confirmExit();
  };

  const handleCancelExit = () => {
    console.log('Cancel exit clicked');
    setShowExitDialog(false);
  };

  // Handle master volume changes
  const handleMasterVolumeChange = (value: number) => {
    setMasterVolume(value);
    audioEngine.setMasterVolume(value / 100);
  };

  const handlePlaybackLatencyChange = (value: number) => {
    const clamped = Math.max(5, Math.min(500, Math.round(value)));
    setPlaybackLatencyMs(clamped);
    safeSetLocalStorage('osmpdrum.playbackLatencyMs', String(clamped));
    audioEngine.setPlaybackLatency(clamped);
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

    const handleMidiNote = (e: CustomEvent) => {
      const { note } = e.detail as { note: number; velocity: number; channel: number };
      const padId = note - 36;
      if (padId >= 0 && padId < 32) {
        playPad(padId);
      }
    };

    window.addEventListener('rust-file-drop', handleDrop as any);
    window.addEventListener('rust-waveform-ready', handleWaveformReady as any);
    window.addEventListener('rust-midi-note', handleMidiNote as any);

    return () => {
      window.removeEventListener('rust-file-drop', handleDrop as any);
      window.removeEventListener('rust-waveform-ready', handleWaveformReady as any);
      window.removeEventListener('rust-midi-note', handleMidiNote as any);
    };
  }, [selectedPadId, pads]);

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
    setHasUnsavedChanges(true);
  };

  // Handle start/end point changes
  const handleStartPointChange = (value: number) => {
    setPads(prev => prev.map(p => {
      if (p.id === selectedPadId) {
        return { ...p, startPoint: value };
      }
      return p;
    }));
    setHasUnsavedChanges(true);
  };

  const handleSidebarFileSelect = (path: string) => {
    const name = path.split(/[\\/]/).pop() || 'SAMPLE';
    const cleanName = name.replace(/\.(wav|mp3|flac|ogg|aiff)$/i, '');
    setPads(prev => prev.map(p => {
      if (p.id === selectedPadId) {
        return { ...p, filePath: path, label: cleanName.substring(0, 8).toUpperCase() };
      }
      return p;
    }));
    audioEngine.load(selectedPadId, path);
    setHasUnsavedChanges(true);
  };

  const handleSwapPads = (fromId: number, toId: number) => {
    setPads(prev => {
      const fromPad = prev[fromId];
      const toPad   = prev[toId];
      setTimeout(() => {
        if (toPad.filePath)  audioEngine.load(fromId, toPad.filePath);
        if (fromPad.filePath) audioEngine.load(toId, fromPad.filePath);
      }, 0);
      return prev.map(p => {
        if (p.id === fromId) return { ...toPad,  id: fromId, isActive: p.isActive };
        if (p.id === toId)   return { ...fromPad, id: toId,   isActive: p.isActive };
        return p;
      });
    });
    setHasUnsavedChanges(true);
  };

  const handleCopyPadTo = (fromId: number, toId: number) => {
    setPads(prev => {
      const fromPad = prev[fromId];
      setTimeout(() => {
        if (fromPad.filePath) audioEngine.load(toId, fromPad.filePath);
      }, 0);
      return prev.map(p => {
        if (p.id !== toId) return p;
        return { ...fromPad, id: toId, isActive: p.isActive };
      });
    });
    setHasUnsavedChanges(true);
  };

  const handleClearPad = (padId: number) => {
    setPads(prev => prev.map(p => {
      if (p.id !== padId) return p;
      return { ...p, filePath: undefined, label: '', waveformPeaks: undefined, duration: 0, audioBuffer: undefined, startPoint: 0, endPoint: 1 };
    }));
    setHasUnsavedChanges(true);
  };

  const handlePathDrop = (padId: number, path: string) => {
    const name = path.split(/[\\/]/).pop() || 'SAMPLE';
    const cleanName = name.replace(/\.(wav|mp3|flac|ogg|aiff)$/i, '');
    setPads(prev => prev.map(p => {
      if (p.id !== padId) return p;
      return { ...p, filePath: path, label: cleanName.substring(0, 8).toUpperCase() };
    }));
    audioEngine.load(padId, path);
    setHasUnsavedChanges(true);
  };

  const handleEndPointChange = (value: number) => {
    setPads(prev => prev.map(p => {
      if (p.id === selectedPadId) {
        return { ...p, endPoint: value };
      }
      return p;
    }));
    setHasUnsavedChanges(true);
  };

  // Handle file load to specific pad (browser drag-drop)
  const handleFileLoadToPad = async (padId: number, buffer: AudioBuffer, fileName: string) => {
    // For browser-based drops, we need to save the file temporarily or use a different approach
    // Since we're using Rust backend, we'll need to handle this differently
    // For now, update the UI state
    const cleanName = fileName.replace(/\.(wav|mp3|flac|ogg)$/i, '');
    
    setPads(prev => prev.map(p => {
      if (p.id === padId) {
        // Generate waveform peaks from buffer
        const rawData = buffer.getChannelData(0);
        const samples = 200;
        const blockSize = Math.floor(rawData.length / samples);
        const peaks: number[] = [];
        
        for (let i = 0; i < samples; i++) {
          let blockStart = blockSize * i;
          let sum = 0;
          for (let j = 0; j < blockSize; j++) {
            sum += Math.abs(rawData[blockStart + j]);
          }
          peaks.push(sum / blockSize);
        }
        
        // Normalize peaks
        const max = Math.max(...peaks, 0.001);
        const normalizedPeaks = peaks.map(p => p / max);
        
        return {
          ...p,
          label: cleanName.substring(0, 8).toUpperCase(),
          waveformPeaks: normalizedPeaks,
          duration: buffer.duration,
          startPoint: 0,
          endPoint: 1,
          // Store buffer temporarily for playback
          audioBuffer: buffer
        };
      }
      return p;
    }));
    
    // Select the pad
    setSelectedPadId(padId);
    setPads(prev => prev.map(p => ({
      ...p,
      isActive: p.id === padId
    })));
    
    setHasUnsavedChanges(true);
  };

  const playPad = (padId: number) => {
    const pad = pads[padId];
    if (!pad.filePath || pad.isMuted) return;

    setVuLevel(prev => Math.min(1, Math.max(prev, 0.85)));

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

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      setVuLevel(v => Math.max(0, v * 0.92));
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  // Debug: log dialog state
  useEffect(() => {
    console.log('showExitDialog state:', showExitDialog);
  }, [showExitDialog]);

  return (
    <div className="bg-app-bg text-text-main font-mona h-screen flex flex-col overflow-hidden select-none text-sm">
      {/* Header */}
      <header className="h-10 bg-app-bg border-b border-border-dark flex items-center px-4 shrink-0 z-20">
        <div className="font-extrabold tracking-tight mr-8 text-base flex items-center">
          <img src={Logo} alt="" />
        </div>
        <nav className="flex gap-5 text-xs font-semibold text-text-muted flex items-center">
          <DropdownMenu
            label="FILE"
            items={[
              {
                label: 'Audio Settings...',
                onClick: () => openAudioSettings(),
                icon: <FileAudio size={14} />,
              },
              { divider: true } as any,
              {
                label: 'Exit',
                onClick: () => setShowExitDialog(true),
              },
            ]}
          />
          <DropdownMenu
            label="EDIT"
            items={[
              {
                label: 'Preferences',
                onClick: () => {},
                icon: <Settings size={14} />,
                disabled: true,
              },
            ]}
          />
          <DropdownMenu
            label="TOOL"
            items={[
              {
                label: 'Open Mixer',
                onClick: () => {},
                icon: <Layout size={14} />,
                disabled: true,
              },
            ]}
          />
          <DropdownMenu
            label="HELP"
            items={[
              {
                label: 'About',
                onClick: () => {},
                icon: <HelpCircle size={14} />,
                disabled: true,
              },
            ]}
          />
        </nav>
      </header>

      <div className="flex flex-1 overflow-hidden">

        <Sidebar onFileSelect={handleSidebarFileSelect} />

        {/* Main Content */}
        <main className="flex-1 flex flex-col bg-app-bg min-w-0">

          {/* Top Control Panel */}
          <div className="h-control-h border-border-dark flex shrink-0 border-b border-[#1e1e1e]">

            {/* Waveform Section */}
            <div className="flex-1 flex flex-col border-r border-[#1e1e1e] min-w-0 relative group">
              <div className="absolute top-0 left-0 bg-accent-cyan text-black px-2.5 py-0.5 text-[10px] font-bold z-10 tracking-wider uppercase">
                {selectedPad?.label || 'EMPTY_SLOT'}
              </div>
              <WaveformDisplay
                color="text-accent-cyan"
                peaks={selectedPad?.waveformPeaks}
                isPlaying={isPlaying && selectedPad?.filePath !== undefined}
                playProgress={playProgress}
                startPoint={selectedPad?.startPoint || 0}
                endPoint={selectedPad?.endPoint || 1}
                onStartPointChange={handleStartPointChange}
                onEndPointChange={handleEndPointChange}
              />
              <div className="flex justify-between text-[10px] text-text-main px-2.5 py-1 border-t border-[#1e1e1e] bg-[#0f0f0f]">
                <span className="font-mono text-gray-600 text-[9px] uppercase tracking-wider">
                  Start: 000000 ms
                </span>
                <span className="font-mono text-gray-500 text-[9px]">
                  Duration: {selectedPad?.duration ? selectedPad.duration.toFixed(3) : '0.000'} s
                </span>
              </div>
            </div>

            {/* Envelope Section */}
            <div className="w-80 bg-panel-bg border-r border-[#1e1e1e] flex flex-col shrink-0">
              <div className="bg-[#161616] text-text-muted text-[10px] h-7 font-bold px-2.5 py-1 border-b border-[#1e1e1e] uppercase tracking-widest flex items-center">
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
            <div className="w-48 bg-panel-bg border-r border-[#1e1e1e] text-xs flex flex-col shrink-0">
              <div className="bg-[#161616] text-text-muted text-[10px] h-7 font-bold px-2.5 py-1 border-b border-[#1e1e1e] uppercase tracking-widest flex items-center">
                Sample Properties
              </div>
              <div className="p-3 flex flex-col gap-2.5">
                {[
                  { label: 'Root Note', value: 'C3' },
                  { label: 'Tune', value: '+0 st' },
                  { label: 'Pitch', value: '0.00' },
                  { label: 'Pan', value: 'C' },
                ].map(({ label, value }) => (
                  <div key={label} className="flex justify-between items-center">
                    <span className="text-[10px] text-gray-600 uppercase tracking-wider">{label}</span>
                    <span className="text-[11px] text-accent-cyan font-mono bg-black/40 px-1.5 py-0.5 rounded text-right min-w-[36px]">{value}</span>
                  </div>
                ))}
              </div>
            </div>

            {/* Master Volume Section */}
            <div className="w-52 bg-panel-bg flex flex-col shrink-0">
              <div className="bg-[#161616] text-text-muted text-[10px] h-7 font-bold px-2.5 py-1 border-b border-[#1e1e1e] uppercase tracking-widest flex items-center">
                Master
              </div>
              <div className="flex items-center justify-around flex-1 px-3 pb-2">
                <div className="flex flex-col items-center gap-1">
                  <VUMeter level={vuLevel} width={48} height={60} />
                  <span className="text-[9px] font-bold text-gray-600 uppercase tracking-widest">VU</span>
                </div>
                <Knob
                  label="Master"
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
            onSwapPads={handleSwapPads}
            onCopyPadTo={handleCopyPadTo}
            onClearPad={handleClearPad}
            onPathDrop={handlePathDrop}
          />

          {/* Bottom Effects Rack */}
          <EffectsRack />

        </main>
      </div>

      {/* Exit Confirmation Dialog */}
      <Dialog
        isOpen={showExitDialog}
        onClose={handleCancelExit}
        title="Unsaved Changes"
        size="sm"
      >
        <p className="mb-4 text-text-muted leading-relaxed">
          You have unsaved changes. Are you sure you want to exit?
        </p>
        <DialogFooter>
          <DialogButton onClick={handleCancelExit} variant="secondary">
            Cancel
          </DialogButton>
          <DialogButton onClick={handleConfirmExit} variant="danger">
            Exit Without Saving
          </DialogButton>
        </DialogFooter>
      </Dialog>

      <Dialog
        isOpen={showAudioSettings}
        onClose={() => setShowAudioSettings(false)}
        title="Audio Settings"
        size="md"
      >
        <div className="space-y-4">
          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-text-muted mb-2">
              Playback API
            </div>
            <MenuSelect
              value={selectedBackend}
              placeholder="(No backends reported)"
              items={audioBackends.map(b => ({ label: b, value: b }))}
              onChange={(v) => applyBackend(v)}
            />
          </div>

          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-text-muted mb-2">
              Output Device
            </div>
            <MenuSelect
              value={selectedDevice}
              placeholder="(No devices reported)"
              items={audioDevices.map(d => ({ label: d, value: d }))}
              onChange={(v) => applyDevice(v)}
            />
          </div>

          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-text-muted mb-2">
              Buffer size (frames)
            </div>
            <MenuSelect
              value={String(bufferSizeFrames)}
              placeholder="Auto"
              items={[
                { label: 'Auto', value: '0' },
                { label: '64', value: '64' },
                { label: '128', value: '128' },
                { label: '256', value: '256' },
                { label: '512', value: '512' },
                { label: '1024', value: '1024' },
                { label: '2048', value: '2048' },
                { label: '4096', value: '4096' },
                { label: '8192', value: '8192' },
              ]}
              onChange={(v) => applyBufferFrames(Number(v))}
            />
            <div className="mt-2 text-[11px] text-text-muted leading-relaxed">
              Set to 0 for auto (uses latency). Setting an explicit frame size overrides latency.
            </div>
          </div>

          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-text-muted mb-2">
              Sample Rate
            </div>
            <MenuSelect
              value={String(sampleRate)}
              placeholder="Device default"
              items={[
                { label: 'Device default', value: '0' },
                { label: '44100 Hz', value: '44100' },
                { label: '48000 Hz', value: '48000' },
                { label: '96000 Hz', value: '96000' },
              ]}
              onChange={(v) => applySampleRate(Number(v))}
            />
            <div className="mt-2 text-[11px] text-text-muted leading-relaxed">
              Custom rates require WASAPI Exclusive mode.
            </div>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <div className="text-[10px] font-bold uppercase tracking-wider text-text-muted">
                WASAPI Exclusive Mode
              </div>
              <div className="mt-1 text-[11px] text-text-muted leading-relaxed">
                Direct hardware access for minimum latency.
              </div>
            </div>
            <button
              onClick={() => applyWasapiExclusive(!wasapiExclusive)}
              className={`w-10 h-5 rounded-full transition-colors shrink-0 ml-4 ${
                wasapiExclusive ? 'bg-accent-cyan' : 'bg-[#2a2a2a]'
              }`}
            >
              <span
                className={`block w-4 h-4 rounded-full bg-white shadow mx-0.5 transition-transform ${
                  wasapiExclusive ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>

          <div>
            <div className="text-[10px] font-bold uppercase tracking-wider text-text-muted mb-2">
              MIDI Input Device
            </div>
            <MenuSelect
              value={selectedMidiInput}
              placeholder="(No MIDI inputs detected)"
              items={[
                { label: '— None —', value: '' },
                ...midiInputPorts.map(p => ({ label: p, value: p })),
              ]}
              onChange={(v) => applyMidiInput(v)}
            />
            <div className="mt-2 text-[11px] text-text-muted leading-relaxed">
              MIDI notes 36–67 trigger pads 1–32 (GM drum map).
            </div>
          </div>
        </div>
      </Dialog>
    </div>
  );
};

export default App;