import React, { useState, useRef, useEffect } from 'react';
import { Layout, Music, Settings, HelpCircle, FileAudio, Folder } from 'lucide-react';
import { WaveformDisplay } from './components/WaveformDisplay';
import { Knob } from './components/Knob';
import { PadGrid } from './components/PadGrid';
import { EffectsRack } from './components/EffectsRack';
import { PadData } from './types';
import Logo from './assets/logo.svg';

const App: React.FC = () => {
  const [selectedPadId, setSelectedPadId] = useState<number>(0);
  const audioContextRef = useRef<AudioContext | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playProgress, setPlayProgress] = useState(0);
  const animationFrameRef = useRef<number>(0);
  const playStartTimeRef = useRef<number>(0);
  const currentSourceRef = useRef<AudioBufferSourceNode | null>(null);
  
  // Initialize Web Audio API
  useEffect(() => {
    try {
      const AudioContextClass = (window as any).AudioContext || (window as any).webkitAudioContext;
      audioContextRef.current = new AudioContextClass();
    } catch (e) {
      console.error('Web Audio API not supported', e);
    }
    return () => {
      audioContextRef.current?.close();
    };
  }, []);

  // Mock data for pads
  const [pads, setPads] = useState<PadData[]>(
    Array.from({ length: 32 }).map((_, i) => ({
      id: i,
      label: i === 0 ? 'KICK' : '',
      isMuted: false,
      isSolo: false,
      isActive: i === 0,
      audioBuffer: undefined,
      fileName: undefined
    }))
  );

  const [envelope, setEnvelope] = useState({
    attack: 30,
    decay: 50,
    sustain: 80
  });

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

  // Handle audio file loading
  const handleFileLoad = (buffer: AudioBuffer, fileName: string) => {
    setPads(prev => prev.map(p => {
      if (p.id !== selectedPadId) return p;
      return {
        ...p,
        audioBuffer: buffer,
        fileName: fileName,
        label: fileName.replace(/\.[^/.]+$/, '').toUpperCase().substring(0, 10)
      };
    }));
  };

  // Handle audio file loading to specific pad
  const handleFileLoadToPad = (padId: number, buffer: AudioBuffer, fileName: string) => {
    setPads(prev => prev.map(p => {
      if (p.id !== padId) return p;
      return {
        ...p,
        audioBuffer: buffer,
        fileName: fileName,
        label: fileName.replace(/\.[^/.]+$/, '').toUpperCase().substring(0, 10)
      };
    }));
    // Also select the pad
    setSelectedPadId(padId);
    setPads(prev => prev.map(p => ({
      ...p,
      isActive: p.id === padId
    })));
  };

  // Play audio sample
  const playPad = (padId: number) => {
    const pad = pads[padId];
    if (!pad.audioBuffer || !audioContextRef.current || pad.isMuted) return;

    // Stop previous playback
    if (currentSourceRef.current) {
      currentSourceRef.current.stop();
      currentSourceRef.current = null;
    }
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }

    const source = audioContextRef.current.createBufferSource();
    const gainNode = audioContextRef.current.createGain();
    
    source.buffer = pad.audioBuffer;
    source.connect(gainNode);
    gainNode.connect(audioContextRef.current.destination);
    
    // Apply envelope
    const now = audioContextRef.current.currentTime;
    const attackTime = envelope.attack / 1000;
    const decayTime = envelope.decay / 1000;
    const sustainLevel = envelope.sustain / 100;
    
    gainNode.gain.setValueAtTime(0, now);
    gainNode.gain.linearRampToValueAtTime(1, now + attackTime);
    gainNode.gain.linearRampToValueAtTime(sustainLevel, now + attackTime + decayTime);
    
    currentSourceRef.current = source;
    playStartTimeRef.current = now;
    const duration = pad.audioBuffer.duration;
    
    setIsPlaying(true);
    setPlayProgress(0);

    // Animate playhead
    const animate = () => {
      if (!audioContextRef.current) return;
      
      const elapsed = audioContextRef.current.currentTime - playStartTimeRef.current;
      const progress = Math.min(elapsed / duration, 1);
      
      setPlayProgress(progress);
      
      if (progress < 1) {
        animationFrameRef.current = requestAnimationFrame(animate);
      } else {
        setIsPlaying(false);
        setPlayProgress(0);
      }
    };

    source.onended = () => {
      setIsPlaying(false);
      setPlayProgress(0);
      currentSourceRef.current = null;
    };

    source.start(0);
    animate();
  };

  // Add keyboard support
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
  }, [pads, envelope]);

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
            <div className="flex items-center gap-2 cursor-pointer hover:text-text-main ml-4 transition-colors">
              <Folder size={14} className="text-gray-500" />
              <span>Kicks</span>
            </div>
            <div className="flex items-center gap-2 cursor-pointer text-accent-cyan ml-8 font-medium">
              <FileAudio size={14} />
              <span>KICK_AM04.wav</span>
            </div>
            <div className="flex items-center gap-2 cursor-pointer hover:text-text-main ml-8">
              <FileAudio size={14} className="text-gray-500" />
              <span>KICK_AM05.wav</span>
            </div>
            <div className="flex items-center gap-2 cursor-pointer hover:text-text-main ml-8">
              <FileAudio size={14} className="text-gray-500" />
              <span>KICK_AM06.wav</span>
            </div>
             <div className="flex items-center gap-2 cursor-pointer hover:text-text-main ml-4 transition-colors">
              <Folder size={14} className="text-gray-500" />
              <span>Snares</span>
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
                  {selectedPad?.fileName || 'EMPTY_SLOT'}
               </div>
               <WaveformDisplay 
                 color="text-accent-cyan" 
                 audioBuffer={selectedPad?.audioBuffer}
                 onFileLoad={handleFileLoad}
                 isPlaying={isPlaying && selectedPad?.audioBuffer !== undefined}
                 playProgress={playProgress}
               />
               <div className="flex justify-between text-[10px] text-text-main px-2 py-0.5 border-t border-border-dark bg-header-bg">
                  <span className="font-mono opacity-70">
                    Start: 000000 smp
                  </span>
                  <span className="font-mono opacity-70">
                    End: {selectedPad?.audioBuffer ? selectedPad.audioBuffer.length.toString().padStart(6, '0') : '000000'} smp
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
                    onChange={(v) => setEnvelope(e => ({...e, attack: v}))} 
                    suffix="ms"
                  />
                  <Knob 
                    label="Decay" 
                    value={envelope.decay} 
                    onChange={(v) => setEnvelope(e => ({...e, decay: v}))} 
                    suffix="ms"
                  />
                  <Knob 
                    label="Sustain" 
                    value={envelope.sustain} 
                    onChange={(v) => setEnvelope(e => ({...e, sustain: v}))} 
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
                     <div className="flex justify-between items-center">
                        <span className="text-text-muted">Pan</span>
                        <span className="text-accent-cyan font-mono bg-black/30 px-1 rounded">C</span>
                    </div>
                     <div className="flex justify-between items-center">
                        <span className="text-text-muted">Gain</span>
                        <span className="text-accent-cyan font-mono bg-black/30 px-1 rounded">-0.5 dB</span>
                    </div>
                </div>
            </div>
          </div>

          {/* Pads Grid */}
          <PadGrid 
            pads={pads} 
            onSelect={handlePadSelect} 
            onToggle={handlePadToggle}
            onPlay={playPad}
            onFileLoadToPad={handleFileLoadToPad}
          />

          {/* Bottom Effects Rack */}
          <EffectsRack />

        </main>
      </div>
    </div>
  );
};

export default App;