import React from 'react';
import { Plus } from 'lucide-react';

const ModuleHeader: React.FC<{ label: string; dotColor: string; dotGlow: string }> = ({ label, dotColor, dotGlow }) => (
    <div className="bg-[#181818] px-2.5 py-1.5 text-[10px] font-bold border-b border-[#232323] text-gray-300 flex justify-between items-center uppercase tracking-widest shrink-0">
        <span>{label}</span>
        <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: dotColor, boxShadow: `0 0 5px ${dotGlow}` }} />
    </div>
);

const SaturationModule: React.FC = () => (
    <div className="w-44 flex flex-col border border-[#232323] bg-[#0d0d0d] shrink-0 rounded-sm overflow-hidden">
        <ModuleHeader label="Saturation" dotColor="#7df9ff" dotGlow="#7df9ff" />
        <div className="flex-1 p-3 flex flex-col gap-2.5 justify-center">
            {/* Saturation transfer curve */}
            <svg width="100%" height="42" viewBox="0 0 116 42" preserveAspectRatio="none">
                <rect width="116" height="42" fill="#0a0a0a" rx="2" />
                {/* Grid lines */}
                <line x1="58" y1="0" x2="58" y2="42" stroke="#1e1e1e" strokeWidth="1" />
                <line x1="0" y1="21" x2="116" y2="21" stroke="#1e1e1e" strokeWidth="1" />
                {/* Transfer curve - soft clip shape */}
                <path d="M 4 38 C 20 36 32 28 44 21 C 56 14 60 8 76 5 C 88 3 100 3 112 3" fill="none" stroke="#3a8a9a" strokeWidth="1.5" strokeLinecap="round" />
                {/* Linear reference */}
                <line x1="4" y1="38" x2="112" y2="4" stroke="#333" strokeWidth="0.5" strokeDasharray="2,2" />
            </svg>
            {/* Drive slider */}
            <div>
                <div className="flex justify-between mb-1">
                    <span className="text-[9px] text-gray-600 uppercase tracking-wider">Drive</span>
                    <span className="text-[9px] text-gray-500 font-mono">62%</span>
                </div>
                <div className="w-full h-1 bg-[#222] rounded-full relative">
                    <div className="absolute left-0 top-0 h-full rounded-full bg-gradient-to-r from-[#2a6a78] to-[#7df9ff]" style={{ width: '62%' }} />
                    <div className="absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 bg-white rounded-full shadow" style={{ left: 'calc(62% - 5px)' }} />
                </div>
            </div>
        </div>
    </div>
);

const CompressorModule: React.FC = () => {
    const totalBars = 18;
    const activeBars = 5;
    return (
        <div className="w-44 flex flex-col border border-[#232323] bg-[#0d0d0d] shrink-0 rounded-sm overflow-hidden">
            <ModuleHeader label="Compressor" dotColor="#22c55e" dotGlow="#22c55e88" />
            <div className="flex-1 p-2.5 flex gap-2">
                {/* GR Meter */}
                <div className="flex flex-col justify-between items-center w-5">
                    <span className="text-[7px] text-gray-600 uppercase">GR</span>
                    <div className="flex-1 flex flex-col-reverse gap-px py-1 w-full">
                        {Array.from({ length: totalBars }).map((_, i) => {
                            const active = i < activeBars;
                            const color = i < 10 ? '#22c55e' : i < 14 ? '#eab308' : '#ef4444';
                            return (
                                <div
                                    key={i}
                                    className="w-full rounded-[1px] transition-opacity"
                                    style={{ height: '4px', backgroundColor: color, opacity: active ? 0.9 : 0.1 }}
                                />
                            );
                        })}
                    </div>
                    <span className="text-[7px] text-gray-600 font-mono">-5</span>
                </div>
                {/* Parameters */}
                <div className="flex-1 flex flex-col justify-around gap-0.5">
                    {[
                        { label: 'Thresh', val: '−18', pct: '55%' },
                        { label: 'Ratio', val: '4:1', pct: '42%' },
                        { label: 'Attack', val: '10ms', pct: '35%' },
                        { label: 'Release', val: '80ms', pct: '60%' },
                    ].map(({ label, val, pct }) => (
                        <div key={label}>
                            <div className="flex justify-between mb-0.5">
                                <span className="text-[8px] text-gray-600 uppercase tracking-wider">{label}</span>
                                <span className="text-[8px] text-gray-500 font-mono">{val}</span>
                            </div>
                            <div className="w-full h-0.5 bg-[#222] rounded-full relative">
                                <div className="absolute left-0 top-0 h-full bg-[#2a6a78] rounded-full" style={{ width: pct }} />
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
};

const EQ3Module: React.FC = () => {
    const eqPath = "M 4 34 C 12 34 18 32 26 28 C 34 24 40 30 50 26 C 58 23 62 12 72 10 C 80 8 84 16 92 20 C 100 24 106 28 112 30";
    const eqFill = eqPath + " L 112 38 L 4 38 Z";
    return (
        <div className="w-44 flex flex-col border border-[#232323] bg-[#0d0d0d] shrink-0 rounded-sm overflow-hidden">
            <ModuleHeader label="EQ-3" dotColor="#eab308" dotGlow="#eab30888" />
            <div className="flex-1 p-2.5 flex flex-col gap-2 justify-center">
                {/* Frequency response */}
                <svg width="100%" height="44" viewBox="0 0 116 44" preserveAspectRatio="none">
                    <rect width="116" height="44" fill="#0a0a0a" rx="2" />
                    {/* Horizontal grid */}
                    {[11, 22, 33].map(y => (
                        <line key={y} x1="4" y1={y} x2="112" y2={y} stroke="#181818" strokeWidth="0.5" />
                    ))}
                    {/* Vertical grid */}
                    {[29, 58, 87].map(x => (
                        <line key={x} x1={x} y1="2" x2={x} y2="40" stroke="#181818" strokeWidth="0.5" />
                    ))}
                    {/* 0dB reference */}
                    <line x1="4" y1="22" x2="112" y2="22" stroke="#2a2a2a" strokeWidth="0.5" strokeDasharray="3,2" />
                    {/* EQ curve fill */}
                    <path d={eqFill} fill="#7df9ff" fillOpacity="0.06" />
                    {/* EQ curve */}
                    <path d={eqPath} fill="none" stroke="#7df9ff" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    {/* Band handles */}
                    <circle cx="26" cy="28" r="2.5" fill="#7df9ff" opacity="0.5" />
                    <circle cx="72" cy="10" r="2.5" fill="#eab308" opacity="0.7" />
                    <circle cx="92" cy="20" r="2.5" fill="#7df9ff" opacity="0.5" />
                </svg>
                {/* Band labels */}
                <div className="flex justify-between px-0.5">
                    {['LO', 'MID', 'HI'].map(b => (
                        <span key={b} className="text-[8px] text-gray-600 uppercase tracking-widest">{b}</span>
                    ))}
                </div>
            </div>
        </div>
    );
};

export const EffectsRack: React.FC = () => {
    return (
        <div className="h-44 bg-[#080808] border-t border-[#1c1c1c] px-2.5 py-2 flex gap-2 shrink-0 overflow-x-auto">
            <SaturationModule />
            <CompressorModule />
            <EQ3Module />
            <button className="w-20 border border-dashed border-[#252525] text-[#3a3a3a] flex flex-col items-center justify-center text-xs hover:text-[#666] hover:border-[#444] hover:bg-[#111] cursor-pointer transition-all shrink-0 rounded-sm gap-1">
                <Plus size={15} />
                <span className="text-[9px] uppercase tracking-widest">Add FX</span>
            </button>
        </div>
    );
};