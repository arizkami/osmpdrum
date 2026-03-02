import React, { useState } from 'react';
import { Copy, Trash2, FileAudio, Clipboard, ArrowLeftRight, Play } from 'lucide-react';
import { PadData } from '../types';
import { ContextMenu, ContextMenuItem } from './ContextMenu';

interface PadGridProps {
    pads: PadData[];
    onSelect: (id: number) => void;
    onToggle: (id: number, type: 'mute' | 'solo') => void;
    onPlay?: (id: number) => void;
    onFileLoadToPad?: (padId: number, buffer: any, fileName: string) => void;
    onSwapPads?: (fromId: number, toId: number) => void;
    onCopyPadTo?: (fromId: number, toId: number) => void;
    onClearPad?: (id: number) => void;
    onPathDrop?: (padId: number, path: string) => void;
}

const KEY_HINTS = [
    'Q','W','E','R','T','Y','U','I',
    'A','S','D','F','G','H','J','K',
    'Z','X','C','V','B','N','M',',',
];

const ROW_CONFIGS = [
    { letter: 'A', accent: '#7df9ff', dimBg: '#0c1517', ringColor: '#7df9ff33' },
    { letter: 'B', accent: '#4ade80', dimBg: '#0c1510', ringColor: '#4ade8033' },
    { letter: 'C', accent: '#fb923c', dimBg: '#160f0b', ringColor: '#fb923c33' },
    { letter: 'D', accent: '#c084fc', dimBg: '#110c17', ringColor: '#c084fc33' },
];

export const PadGrid: React.FC<PadGridProps> = ({
    pads, onSelect, onToggle, onPlay,
    onFileLoadToPad, onSwapPads, onCopyPadTo, onClearPad, onPathDrop,
}) => {
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number; padId: number } | null>(null);
    const [dragOverPadId, setDragOverPadId] = useState<number | null>(null);
    const [draggingPadId, setDraggingPadId] = useState<number | null>(null);
    const [copiedPadId, setCopiedPadId] = useState<number | null>(null);

    const handlePadClick = (id: number) => {
        onSelect(id);
        if (onPlay) onPlay(id);
    };

    const handlePadDragStart = (e: React.DragEvent, padId: number) => {
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('pad-id', String(padId));
        setDraggingPadId(padId);
    };

    const handlePadDragEnd = () => {
        setDraggingPadId(null);
        setDragOverPadId(null);
    };

    const handleDrop = async (e: React.DragEvent, padId: number) => {
        e.preventDefault();
        e.stopPropagation();
        setDragOverPadId(null);
        setDraggingPadId(null);

        // 1. Pad-to-pad swap
        const sourcePadId = e.dataTransfer.getData('pad-id');
        if (sourcePadId !== '' && sourcePadId !== String(padId)) {
            onSwapPads?.(Number(sourcePadId), padId);
            return;
        }

        // 2. Path from sidebar (custom MIME)
        const audioPath = e.dataTransfer.getData('application/x-audio-path');
        if (audioPath) {
            onPathDrop?.(padId, audioPath);
            return;
        }

        // 3. OS file drop
        const files = Array.from(e.dataTransfer.files);
        const audioFile = files.find(f =>
            f.type.startsWith('audio/') ||
            /\.(wav|mp3|ogg|flac|aiff)$/i.test(f.name)
        );
        if (audioFile && onFileLoadToPad) {
            try {
                const arrayBuffer = await audioFile.arrayBuffer();
                const AudioContextClass = (window as any).AudioContext || (window as any).webkitAudioContext;
                const audioContext = new AudioContextClass();
                const buffer = await audioContext.decodeAudioData(arrayBuffer);
                onFileLoadToPad(padId, buffer, audioFile.name);
            } catch (err) {
                console.error('Error loading audio file:', err);
            }
        }
    };

    const handleDragOver = (e: React.DragEvent, padId: number) => {
        e.preventDefault();
        e.stopPropagation();
        e.dataTransfer.dropEffect = 'move';
        setDragOverPadId(padId);
    };

    const handleDragLeave = (e: React.DragEvent, padId: number) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) {
            if (dragOverPadId === padId) setDragOverPadId(null);
        }
    };

    const handleContextMenu = (e: React.MouseEvent, padId: number) => {
        e.preventDefault();
        setContextMenu({ x: e.clientX, y: e.clientY, padId });
    };

    const getContextMenuItems = (padId: number): ContextMenuItem[] => {
        const pad = pads[padId];
        const hasCopy = copiedPadId !== null && copiedPadId !== padId;
        return [
            {
                label: 'Play',
                icon: <Play size={13} />,
                onClick: () => onPlay?.(padId),
                disabled: !pad.filePath,
            },
            { divider: true } as ContextMenuItem,
            {
                label: 'Copy Sample',
                icon: <Copy size={13} />,
                onClick: () => setCopiedPadId(padId),
                disabled: !pad.filePath,
            },
            {
                label: hasCopy ? `Paste (from ${String(copiedPadId! + 1).padStart(2, '0')})` : 'Paste Sample',
                icon: <Clipboard size={13} />,
                onClick: () => { if (hasCopy) onCopyPadTo?.(copiedPadId!, padId); },
                disabled: !hasCopy,
            },
            {
                label: 'Swap With…',
                icon: <ArrowLeftRight size={13} />,
                onClick: () => {},
                disabled: true,
            },
            { divider: true } as ContextMenuItem,
            {
                label: 'Clear Sample',
                icon: <Trash2 size={13} />,
                onClick: () => onClearPad?.(padId),
                disabled: !pad.filePath,
            },
            { divider: true } as ContextMenuItem,
            { label: pad.isMuted ? 'Unmute' : 'Mute',  onClick: () => onToggle(padId, 'mute') },
            { label: pad.isSolo  ? 'Unsolo' : 'Solo',  onClick: () => onToggle(padId, 'solo') },
        ];
    };

    const loadedCount = pads.filter(p => !!p.waveformPeaks).length;

    const isPadSwapTarget = (padId: number) =>
        dragOverPadId === padId && draggingPadId !== null && draggingPadId !== padId;

    return (
        <>
            <div className="flex-1 flex flex-col min-h-0 bg-[#080808]">

                {/* Kit header */}
                <div className="h-7 flex items-center px-3 gap-3 shrink-0 border-t border-b border-[#171717]">
                    <span className="text-[9px] font-bold text-[#3a3a3a] uppercase tracking-[0.18em]">Pad Kit</span>
                    <div className="w-px h-3 bg-[#1e1e1e]" />
                    <div className="flex items-center gap-1.5">
                        {ROW_CONFIGS.map(row => (
                            <div key={row.letter} className="flex items-center gap-1">
                                <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: row.accent, opacity: 0.45 }} />
                                <span className="text-[8px] font-mono" style={{ color: row.accent + '55' }}>{row.letter}</span>
                            </div>
                        ))}
                    </div>
                    <div className="flex-1" />
                    {draggingPadId !== null && (
                        <span className="text-[8px] font-mono text-[#333] uppercase tracking-wider">
                            drag to swap
                        </span>
                    )}
                    <span className="text-[9px] font-mono text-[#2e2e2e]">
                        {loadedCount}<span className="text-[#222]">/{pads.length}</span>
                    </span>
                </div>

                {/* Pad rows */}
                <div className="flex-1 flex flex-col gap-[1px] bg-[#111] min-h-0">
                    {ROW_CONFIGS.map((row, rowIdx) => {
                        const rowPads = pads.slice(rowIdx * 8, rowIdx * 8 + 8);
                        return (
                            <div key={row.letter} className="flex flex-1 gap-[1px] min-h-0">

                                {/* Row label column */}
                                <div className="w-5 shrink-0 flex items-center justify-center" style={{ backgroundColor: '#0a0a0a' }}>
                                    <span
                                        className="text-[9px] font-bold tracking-widest"
                                        style={{ color: row.accent + '40', writingMode: 'vertical-rl', transform: 'rotate(180deg)' }}
                                    >
                                        {row.letter}
                                    </span>
                                </div>

                                {/* 8 pads */}
                                {rowPads.map((pad) => {
                                    const keyHint = KEY_HINTS[pad.id];
                                    const isDragOver  = dragOverPadId === pad.id;
                                    const isDragging  = draggingPadId === pad.id;
                                    const isSwapTarget = isPadSwapTarget(pad.id);
                                    const isDropTarget = isDragOver && !isDragging;

                                    return (
                                        <div
                                            key={pad.id}
                                            draggable={!!pad.filePath}
                                            onClick={() => handlePadClick(pad.id)}
                                            onContextMenu={(e) => handleContextMenu(e, pad.id)}
                                            onDragStart={(e) => handlePadDragStart(e, pad.id)}
                                            onDragEnd={handlePadDragEnd}
                                            onDrop={(e) => handleDrop(e, pad.id)}
                                            onDragOver={(e) => handleDragOver(e, pad.id)}
                                            onDragLeave={(e) => handleDragLeave(e, pad.id)}
                                            className="flex-1 relative cursor-pointer group overflow-hidden transition-colors"
                                            style={{
                                                backgroundColor: isDropTarget
                                                    ? row.dimBg
                                                    : pad.isActive
                                                        ? row.dimBg
                                                        : '#0c0c0c',
                                                boxShadow: isSwapTarget
                                                    ? `inset 0 0 0 1px ${row.accent}99`
                                                    : isDropTarget
                                                        ? `inset 0 0 0 1px ${row.accent}55`
                                                        : pad.isActive
                                                            ? `inset 0 0 0 1px ${row.ringColor}`
                                                            : undefined,
                                                opacity: isDragging ? 0.35 : 1,
                                                transition: 'opacity 0.15s, background-color 0.1s',
                                            }}
                                        >
                                            {/* Active top accent bar */}
                                            {pad.isActive && (
                                                <div className="absolute top-0 left-0 right-0 h-[2px] z-10"
                                                    style={{ backgroundColor: row.accent, opacity: 0.75 }} />
                                            )}

                                            {/* Hover top micro-line */}
                                            {!pad.isActive && (
                                                <div className="absolute top-0 left-0 right-0 h-px opacity-0 group-hover:opacity-100 transition-opacity z-10"
                                                    style={{ backgroundColor: row.accent + '30' }} />
                                            )}

                                            {/* Swap-target overlay */}
                                            {isSwapTarget && (
                                                <div className="absolute inset-0 z-30 flex items-center justify-center pointer-events-none">
                                                    <div className="flex items-center gap-1 px-2 py-0.5 rounded"
                                                        style={{ backgroundColor: row.accent + '22', border: `1px solid ${row.accent}55` }}>
                                                        <ArrowLeftRight size={9} style={{ color: row.accent }} />
                                                        <span className="text-[8px] font-bold uppercase tracking-wider" style={{ color: row.accent }}>swap</span>
                                                    </div>
                                                </div>
                                            )}

                                            {/* Drop-path overlay (sidebar/OS drop, not pad-swap) */}
                                            {isDropTarget && !isSwapTarget && (
                                                <div className="absolute inset-0 z-30 flex items-center justify-center pointer-events-none">
                                                    <div className="flex items-center gap-1 px-2 py-0.5 rounded"
                                                        style={{ backgroundColor: row.accent + '18', border: `1px solid ${row.accent}44` }}>
                                                        <FileAudio size={9} style={{ color: row.accent }} />
                                                        <span className="text-[8px] font-bold uppercase tracking-wider" style={{ color: row.accent }}>load</span>
                                                    </div>
                                                </div>
                                            )}

                                            {/* Mute / Solo dots */}
                                            <div className="absolute top-1.5 right-1.5 flex gap-1 z-20">
                                                {pad.isSolo  && <div className="w-1.5 h-1.5 rounded-full bg-yellow-400" style={{ boxShadow: '0 0 4px #facc15aa' }} />}
                                                {pad.isMuted && <div className="w-1.5 h-1.5 rounded-full bg-red-500"    style={{ boxShadow: '0 0 4px #ef4444aa' }} />}
                                            </div>

                                            {/* Pad number — top left */}
                                            <div className="absolute top-1 left-1.5 text-[8px] font-mono pointer-events-none z-10 leading-none"
                                                style={{ color: pad.isActive ? row.accent + '70' : '#282828' }}>
                                                {String(pad.id + 1).padStart(2, '0')}
                                            </div>

                                            {/* Centre content */}
                                            <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none gap-0.5">
                                                {pad.waveformPeaks ? (
                                                    <>
                                                        <div className="flex items-center gap-px" style={{ height: 18 }}>
                                                            {Array.from({ length: 16 }).map((_, i) => {
                                                                const si = Math.floor((i / 16) * pad.waveformPeaks!.length);
                                                                const h = Math.max(0.1, pad.waveformPeaks![si] ?? 0.1);
                                                                return (
                                                                    <div key={i} style={{
                                                                        width: 2,
                                                                        height: Math.round(h * 16) + 2,
                                                                        borderRadius: 1,
                                                                        backgroundColor: row.accent,
                                                                        opacity: pad.isActive ? 0.75 : 0.28,
                                                                    }} />
                                                                );
                                                            })}
                                                        </div>
                                                        {pad.label && (
                                                            <span className="text-[8px] font-bold tracking-widest uppercase leading-none"
                                                                style={{ color: pad.isActive ? row.accent : row.accent + '55' }}>
                                                                {pad.label}
                                                            </span>
                                                        )}
                                                        {pad.duration != null && pad.duration > 0 && (
                                                            <span className="text-[7px] font-mono leading-none" style={{ color: '#303030' }}>
                                                                {pad.duration.toFixed(2)}s
                                                            </span>
                                                        )}
                                                    </>
                                                ) : (
                                                    <div className="flex flex-col items-center gap-0.5 opacity-[0.14] group-hover:opacity-25 transition-opacity">
                                                        <FileAudio size={13} strokeWidth={1.5} style={{ color: row.accent }} />
                                                        <span className="text-[7px] font-mono tracking-wider" style={{ color: '#555' }}>EMPTY</span>
                                                    </div>
                                                )}
                                            </div>

                                            {/* Keyboard shortcut — bottom right */}
                                            {keyHint && (
                                                <div className="absolute bottom-1 right-1.5 text-[7px] font-mono leading-none pointer-events-none"
                                                    style={{ color: pad.isActive ? row.accent + '40' : '#242424' }}>
                                                    {keyHint}
                                                </div>
                                            )}

                                            {/* Hover shimmer */}
                                            <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none bg-gradient-to-b from-white/[0.015] to-transparent" />
                                        </div>
                                    );
                                })}
                            </div>
                        );
                    })}
                </div>
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