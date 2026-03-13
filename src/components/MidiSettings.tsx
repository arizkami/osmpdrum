import React, { useState, useEffect, useRef } from 'react';
import { RotateCcw, Zap } from 'lucide-react';
import { Dialog, DialogFooter, DialogButton } from './Dialog';
import { PadData } from '../types';

interface MidiSettingsProps {
    isOpen: boolean;
    onClose: () => void;
    pads: PadData[];
    padMidiNotes: number[];
    padKeys: (string | null)[];
    onChangeMidiNote: (padId: number, note: number) => void;
    onChangeKey: (padId: number, key: string | null) => void;
    onResetMidi: () => void;
    onResetKeys: () => void;
    defaultTab?: TabType;
}

const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

function midiNoteName(n: number): string {
    const oct = Math.floor(n / 12) - 1;
    return `${NOTE_NAMES[n % 12]}${oct}`;
}

function displayKey(k: string): string {
    const map: Record<string, string> = {
        ' ': 'Space', 'ArrowUp': '↑', 'ArrowDown': '↓',
        'ArrowLeft': '←', 'ArrowRight': '→',
        'Enter': '↵', 'Backspace': '⌫', 'Tab': 'Tab',
        'Escape': 'Esc', 'Delete': 'Del',
    };
    return map[k] ?? k.toUpperCase();
}

const ROW_COLORS = ['#7df9ff', '#4ade80', '#fb923c', '#c084fc'];
type TabType = 'midi' | 'key';
type LearnTarget = { padId: number; type: TabType } | null;

export const MidiSettings: React.FC<MidiSettingsProps> = ({
    isOpen, onClose, pads, padMidiNotes, padKeys,
    onChangeMidiNote, onChangeKey, onResetMidi, onResetKeys,
    defaultTab = 'midi',
}) => {
    const [tab, setTab] = useState<TabType>(defaultTab);

    useEffect(() => {
        if (isOpen) setTab(defaultTab);
    }, [isOpen, defaultTab]);
    const [learning, setLearning] = useState<LearnTarget>(null);
    const learnRef = useRef<LearnTarget>(null);

    useEffect(() => { learnRef.current = learning; }, [learning]);

    // MIDI note learn — listen for Rust MIDI event
    useEffect(() => {
        if (!isOpen) return;
        const handler = (e: CustomEvent) => {
            const tgt = learnRef.current;
            if (!tgt || tgt.type !== 'midi') return;
            const { note } = e.detail as { note: number };
            onChangeMidiNote(tgt.padId, note);
            setLearning(null);
        };
        window.addEventListener('rust-midi-note', handler as EventListener);
        return () => window.removeEventListener('rust-midi-note', handler as EventListener);
    }, [isOpen, onChangeMidiNote]);

    // Key learn — capture phase so we intercept before Dialog's Escape listener
    useEffect(() => {
        if (!isOpen) return;
        const handler = (e: KeyboardEvent) => {
            const tgt = learnRef.current;
            if (!tgt || tgt.type !== 'key') return;
            e.preventDefault();
            e.stopPropagation();
            if (e.key === 'Escape') { setLearning(null); return; }
            // Skip modifier-only presses
            if (['Shift', 'Control', 'Alt', 'Meta', 'CapsLock'].includes(e.key)) return;
            onChangeKey(tgt.padId, e.key);
            setLearning(null);
        };
        document.addEventListener('keydown', handler, { capture: true });
        return () => document.removeEventListener('keydown', handler, { capture: true });
    }, [isOpen, onChangeKey]);

    useEffect(() => { if (!isOpen) setLearning(null); }, [isOpen]);

    const rowColor = (padId: number) => ROW_COLORS[Math.floor(padId / 8)] ?? '#7df9ff';

    const switchTab = (t: TabType) => { setTab(t); setLearning(null); };

    return (
        <Dialog isOpen={isOpen} onClose={onClose} title="MIDI & Keyboard Map" size="lg">

            {/* ── Tabs ── */}
            <div className="flex gap-px mb-4 rounded overflow-hidden border border-[#181818] bg-[#0a0a0a]">
                {(['midi', 'key'] as const).map(t => (
                    <button
                        key={t}
                        type="button"
                        onClick={() => switchTab(t)}
                        className={`flex-1 py-2 text-[10px] font-bold uppercase tracking-widest transition-colors ${
                            tab === t
                                ? 'bg-[#0c1a1c] text-[#7df9ff]'
                                : 'text-[#333] hover:text-[#555] hover:bg-[#0d0d0d]'
                        }`}
                    >
                        {t === 'midi' ? 'MIDI Notes' : 'Keyboard Keys'}
                    </button>
                ))}
            </div>

            {/* ── Description ── */}
            <p className="text-[10px] text-[#2e2e2e] mb-3 leading-relaxed">
                {tab === 'midi'
                    ? 'Each pad can respond to a specific MIDI note. Click Learn, then play a note on your controller.'
                    : 'Each pad can be triggered by a keyboard key. Click Learn, then press a key to assign it.'
                }
            </p>

            {/* ── Table ── */}
            <div className="rounded border border-[#161616] overflow-hidden">

                {/* Column header */}
                <div className="grid grid-cols-[38px_88px_1fr_118px] bg-[#0c0c0c] border-b border-[#1a1a1a] px-3 py-1.5">
                    {['#', 'Label', tab === 'midi' ? 'MIDI Note' : 'Key', ''].map((h, i) => (
                        <span key={i} className="text-[9px] font-bold uppercase tracking-widest text-[#252525]">{h}</span>
                    ))}
                </div>

                {/* Rows */}
                <div className="overflow-y-auto max-h-[320px] bg-[#070707]">
                    {pads.map((pad) => {
                        const color = rowColor(pad.id);
                        const isLearning = learning?.padId === pad.id && learning.type === tab;
                        const assignment = tab === 'midi' ? padMidiNotes[pad.id] : padKeys[pad.id];
                        const hasAssignment = assignment != null;

                        return (
                            <div
                                key={pad.id}
                                className="grid grid-cols-[38px_88px_1fr_118px] items-center px-3 py-[5px] border-b border-[#0e0e0e] transition-colors hover:bg-[#090909]"
                                style={{ backgroundColor: isLearning ? '#091113' : undefined }}
                            >
                                {/* Pad number */}
                                <span className="text-[9px] font-mono" style={{ color: color + '40' }}>
                                    {String(pad.id + 1).padStart(2, '0')}
                                </span>

                                {/* Label */}
                                <span
                                    className="text-[9px] font-bold uppercase tracking-wider truncate pr-2"
                                    style={{ color: pad.filePath ? color + '88' : '#242424' }}
                                >
                                    {pad.label || 'EMPTY'}
                                </span>

                                {/* Assignment display */}
                                <span
                                    className={`text-[10px] font-mono ${isLearning ? 'animate-pulse' : ''}`}
                                    style={{
                                        color: isLearning ? '#7df9ff' : hasAssignment ? '#6aadb8' : '#242424',
                                    }}
                                >
                                    {isLearning
                                        ? (tab === 'midi' ? 'Play a MIDI note…' : 'Press any key…')
                                        : tab === 'midi'
                                            ? hasAssignment
                                                ? `${midiNoteName(assignment as number)}  ·  ${assignment}`
                                                : '—'
                                            : hasAssignment
                                                ? displayKey(assignment as string)
                                                : '—'
                                    }
                                </span>

                                {/* Action buttons */}
                                <div className="flex gap-1 justify-end items-center">
                                    {isLearning ? (
                                        <button
                                            type="button"
                                            onClick={() => setLearning(null)}
                                            className="px-2 py-0.5 text-[8px] font-bold uppercase tracking-widest rounded border border-[#7df9ff30] text-[#7df9ff66] hover:text-[#7df9ff] hover:border-[#7df9ff55] transition-colors"
                                        >
                                            Cancel
                                        </button>
                                    ) : (
                                        <>
                                            <button
                                                type="button"
                                                onClick={() => setLearning({ padId: pad.id, type: tab })}
                                                className="flex items-center gap-1 px-2 py-0.5 text-[8px] font-bold uppercase tracking-widest rounded border border-[#1a1a1a] text-[#383838] hover:text-[#7df9ff] hover:border-[#7df9ff33] transition-colors"
                                            >
                                                <Zap size={8} />
                                                Learn
                                            </button>
                                            {hasAssignment && (
                                                <button
                                                    type="button"
                                                    onClick={() => tab === 'midi'
                                                        ? onChangeMidiNote(pad.id, pad.id + 36)
                                                        : onChangeKey(pad.id, null)}
                                                    className="w-6 h-5 text-[11px] font-bold rounded border border-[#1a1a1a] text-[#2e2e2e] hover:text-red-500 hover:border-red-900 transition-colors flex items-center justify-center"
                                                >
                                                    ×
                                                </button>
                                            )}
                                        </>
                                    )}
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>

            {/* ── Status bar ── */}
            <div className="mt-2.5 h-4 flex items-center justify-center">
                {learning ? (
                    <p className="text-[10px] text-[#7df9ff88] animate-pulse flex items-center gap-1.5">
                        <Zap size={10} />
                        {learning.type === 'midi'
                            ? 'Listening for MIDI — play a note on your controller'
                            : 'Listening for key — press any key  ·  Esc to cancel'
                        }
                    </p>
                ) : (
                    <p className="text-[10px] text-[#1e1e1e]">
                        Click <span className="text-[#2e2e2e]">Learn</span> next to a pad to reassign its trigger.
                        {tab === 'midi' ? ' MIDI notes 36–67 are GM drum defaults.' : ' Keys Q–, and A–K and Z–, are defaults.'}
                    </p>
                )}
            </div>

            <DialogFooter>
                <DialogButton
                    onClick={() => { tab === 'midi' ? onResetMidi() : onResetKeys(); setLearning(null); }}
                    variant="secondary"
                >
                    <RotateCcw size={11} className="inline mr-1.5" />
                    Reset {tab === 'midi' ? 'MIDI' : 'Keys'} to Default
                </DialogButton>
                <DialogButton onClick={onClose} variant="secondary">Close</DialogButton>
            </DialogFooter>
        </Dialog>
    );
};
