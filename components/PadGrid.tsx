import React from 'react';
import { Volume2 } from 'lucide-react';
import { PadData } from '../types';

interface PadGridProps {
    pads: PadData[];
    onSelect: (id: number) => void;
    onToggle: (id: number, type: 'mute' | 'solo') => void;
    onPlay?: (id: number) => void;
    onFileLoadToPad?: (padId: number, buffer: AudioBuffer, fileName: string) => void;
}

export const PadGrid: React.FC<PadGridProps> = ({ pads, onSelect, onToggle, onPlay, onFileLoadToPad }) => {
    const [dragOverPadId, setDragOverPadId] = React.useState<number | null>(null);
    const [audioContext] = React.useState(() => new ((window as any).AudioContext || (window as any).webkitAudioContext)());

    const handlePadClick = (id: number) => {
        onSelect(id);
        if (onPlay) {
            onPlay(id);
        }
    };

    const handleDrop = async (e: React.DragEvent, padId: number) => {
        e.preventDefault();
        e.stopPropagation();
        setDragOverPadId(null);

        const files = Array.from(e.dataTransfer.files);
        const audioFile = files.find(f => 
            f.type.startsWith('audio/') || 
            f.name.endsWith('.wav') || 
            f.name.endsWith('.mp3') || 
            f.name.endsWith('.ogg')
        );

        if (audioFile && onFileLoadToPad) {
            try {
                const arrayBuffer = await audioFile.arrayBuffer();
                const buffer = await audioContext.decodeAudioData(arrayBuffer);
                onFileLoadToPad(padId, buffer, audioFile.name);
            } catch (error) {
                console.error('Error loading audio file:', error);
                alert('Failed to load audio file. Make sure it\'s a valid audio format.');
            }
        }
    };

    const handleDragOver = (e: React.DragEvent, padId: number) => {
        e.preventDefault();
        e.stopPropagation();
        setDragOverPadId(padId);
    };

    const handleDragLeave = (e: React.DragEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setDragOverPadId(null);
    };

    return (
        <div className="flex-1 grid grid-cols-8 grid-rows-4 gap-px bg-border-dark p-[2px] min-h-0">
            {pads.map((pad) => (
                <div 
                    key={pad.id}
                    onClick={() => handlePadClick(pad.id)}
                    onDrop={(e) => handleDrop(e, pad.id)}
                    onDragOver={(e) => handleDragOver(e, pad.id)}
                    onDragLeave={handleDragLeave}
                    className={`
                        relative transition-colors cursor-pointer group
                        ${pad.isActive 
                            ? 'bg-[#1a1a1a] ring-1 ring-inset ring-accent-cyan z-10' 
                            : 'bg-pad-bg hover:bg-[#1f1f1f]'
                        }
                        ${pad.audioBuffer ? 'ring-1 ring-inset ring-green-500/30' : ''}
                        ${dragOverPadId === pad.id ? 'ring-2 ring-inset ring-cyan-500 bg-cyan-500/10' : ''}
                    `}
                >
                    {/* Controls overlay */}
                    <div className="absolute top-1 right-1 flex gap-1 z-20 opacity-0 group-hover:opacity-100 transition-opacity">
                        <button 
                            className={`text-[9px] font-bold px-1 rounded-sm focus:outline-none ${pad.isMuted ? 'bg-red-500/20 text-red-500' : 'text-[#555] hover:text-[#888]'}`}
                            onClick={(e) => { e.stopPropagation(); onToggle(pad.id, 'mute'); }}
                        >
                            M
                        </button>
                        <button 
                            className={`text-[9px] font-bold px-1 rounded-sm focus:outline-none ${pad.isSolo ? 'bg-yellow-400/20 text-yellow-400' : 'text-[#555] hover:text-[#888]'}`}
                            onClick={(e) => { e.stopPropagation(); onToggle(pad.id, 'solo'); }}
                        >
                            S
                        </button>
                    </div>

                    {/* Pad Content */}
                    <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
                        {pad.audioBuffer && (
                            <Volume2 
                                size={16} 
                                className={`absolute top-2 left-2 ${pad.isActive ? 'text-accent-cyan' : 'text-green-500/50'}`}
                            />
                        )}
                        {pad.label && (
                            <span className={`text-xs font-bold tracking-widest ${pad.isActive ? 'text-accent-cyan' : 'text-text-muted'}`}>
                                {pad.label}
                            </span>
                        )}
                        {!pad.label && pad.isActive && (
                            <span className="text-[10px] text-accent-cyan opacity-50 font-mono">
                                PAD {pad.id + 1}
                            </span>
                        )}
                    </div>
                    
                    {/* Corner ID */}
                    <div className="absolute bottom-1 left-1 text-[8px] text-[#333] font-mono pointer-events-none">
                        {pad.id + 1}
                    </div>
                </div>
            ))}
        </div>
    );
};