import React, { useState } from 'react';
import { Volume2, Copy, Trash2, FileAudio } from 'lucide-react';
import { PadData } from '../types';
import { ContextMenu, ContextMenuItem } from './ContextMenu';

interface PadGridProps {
    pads: PadData[];
    onSelect: (id: number) => void;
    onToggle: (id: number, type: 'mute' | 'solo') => void;
    onPlay?: (id: number) => void;
    onFileLoadToPad?: (padId: number, buffer: any, fileName: string) => void;
}

export const PadGrid: React.FC<PadGridProps> = ({ pads, onSelect, onToggle, onPlay }) => {
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number; padId: number } | null>(null);

    const handlePadClick = (id: number) => {
        onSelect(id);
        if (onPlay) {
            onPlay(id);
        }
    };

    const handleContextMenu = (e: React.MouseEvent, padId: number) => {
        e.preventDefault();
        setContextMenu({ x: e.clientX, y: e.clientY, padId });
    };

    const getContextMenuItems = (padId: number): ContextMenuItem[] => {
        const pad = pads[padId];
        return [
            {
                label: 'Clear Sample',
                icon: <Trash2 size={14} />,
                onClick: () => console.log('Clear sample', padId),
                disabled: !pad.filePath,
            },
            {
                label: 'Copy',
                icon: <Copy size={14} />,
                onClick: () => console.log('Copy pad', padId),
                disabled: !pad.filePath,
            },
            { divider: true } as ContextMenuItem,
            {
                label: pad.isMuted ? 'Unmute' : 'Mute',
                onClick: () => onToggle(padId, 'mute'),
            },
            {
                label: pad.isSolo ? 'Unsolo' : 'Solo',
                onClick: () => onToggle(padId, 'solo'),
            },
        ];
    };

    return (
        <>
            <div className="flex-1 border-y border-[#3e3e3e] grid grid-cols-8 grid-rows-4 gap-[1px] bg-[#0a0a0a] p-[1px] min-h-0">
                {pads.map((pad) => (
                    <div
                        key={pad.id}
                        data-pad-id={pad.id}
                        onClick={() => handlePadClick(pad.id)}
                        onContextMenu={(e) => handleContextMenu(e, pad.id)}
                        className={`
                            relative transition-all cursor-pointer group overflow-hidden
                            ${pad.isActive
                                ? 'bg-gradient-to-br from-[#1a1a1a] to-[#0f0f0f] ring-2 ring-inset ring-accent-cyan shadow-lg shadow-accent-cyan/20'
                                : 'bg-gradient-to-br from-[#151515] to-[#0d0d0d] hover:from-[#1a1a1a] hover:to-[#121212]'
                            }
                            ${pad.waveformPeaks ? 'ring-1 ring-inset ring-green-500/20' : ''}
                        `}
                    >
                        {/* Subtle grid pattern */}
                        <div className="absolute inset-0 opacity-[0.02]" style={{
                            backgroundImage: 'repeating-linear-gradient(0deg, transparent, transparent 2px, white 2px, white 3px), repeating-linear-gradient(90deg, transparent, transparent 2px, white 2px, white 3px)'
                        }} />

                        {/* Status indicators */}
                        <div className="absolute top-1.5 right-1.5 flex gap-1 z-20">
                            {pad.isMuted && (
                                <div className="w-1.5 h-1.5 rounded-full bg-red-500 shadow-sm shadow-red-500/50" />
                            )}
                            {pad.isSolo && (
                                <div className="w-1.5 h-1.5 rounded-full bg-yellow-400 shadow-sm shadow-yellow-400/50" />
                            )}
                        </div>

                        {/* Pad Content */}
                        <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
                            {pad.waveformPeaks ? (
                                <>
                                    <Volume2
                                        size={20}
                                        className={`mb-1 ${pad.isActive ? 'text-accent-cyan' : 'text-green-500/60'}`}
                                        strokeWidth={1.5}
                                    />
                                    {pad.label && (
                                        <span className={`text-[10px] font-semibold tracking-wider ${pad.isActive ? 'text-accent-cyan' : 'text-text-muted'}`}>
                                            {pad.label}
                                        </span>
                                    )}
                                </>
                            ) : (
                                <div className="flex flex-col items-center opacity-30 group-hover:opacity-50 transition-opacity">
                                    <FileAudio size={18} className="text-text-muted mb-1" strokeWidth={1.5} />
                                    <span className="text-[9px] text-text-muted font-mono">
                                        EMPTY
                                    </span>
                                </div>
                            )}
                        </div>

                        {/* Corner ID with better styling */}
                        <div className={`absolute bottom-1 left-1.5 text-[9px] font-mono font-semibold pointer-events-none ${
                            pad.isActive ? 'text-accent-cyan/40' : 'text-[#404040]'
                        }`}>
                            {String(pad.id + 1).padStart(2, '0')}
                        </div>

                        {/* Hover glow effect */}
                        <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none bg-gradient-to-t from-accent-cyan/5 to-transparent" />
                    </div>
                ))}
            </div>

            {contextMenu && (
                <ContextMenu
                    x={contextMenu.x}
                    y={contextMenu.y}
                    items={getContextMenuItems(contextMenu.padId)}
                    onClose={() => setContextMenu(null)}
                />
            )}
        </>
    );
};