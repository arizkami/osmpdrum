import React, { useState, useRef, useCallback, useEffect } from 'react';
import { PadData } from '../types';
import type { OsmpZonesData } from '../types/osmp';

const NOTE_NAMES = ['C','C#','D','D#','E','F','F#','G','G#','A','A#','B'];
const noteName = (n: number) => `${NOTE_NAMES[n % 12]}${Math.floor(n / 12) - 1}`;

interface DrumViewProps {
  pads: PadData[];
  onSelect: (id: number) => void;
  onPlay?: (id: number) => void;
  osmpBar?: React.ReactNode;
  onOsmpTrigger?: (padId: number) => void;
  osmpZones?: OsmpZonesData | null;
  padMidiNotes?: number[];
  activePadId?: number | null;
}

type PieceKind = 'kick' | 'snare' | 'tom' | 'floor' | 'cymbal' | 'hihat';

type Piece = {
  key: string;
  name: string;
  padId: number;
  kind: PieceKind;
  cx: number;
  cy: number;
  rx: number;
  ry: number;
};

const CYAN = '#7df9ff';
const CYAN_DIM = '#1e2d31';

// Module-level constant — no re-allocation on every render
const PIECES: Piece[] = [
  { key: 'crashL', name: 'Crash L', padId: 7,  kind: 'cymbal', cx: 255, cy: 162, rx: 112, ry: 46 },
  { key: 'splash', name: 'Splash',  padId: 9,  kind: 'cymbal', cx: 472, cy: 144, rx:  50, ry: 24 },
  { key: 'crashR', name: 'Crash R', padId: 10, kind: 'cymbal', cx: 638, cy: 162, rx: 112, ry: 46 },
  { key: 'ride',   name: 'Ride',    padId: 8,  kind: 'cymbal', cx: 782, cy: 218, rx: 136, ry: 56 },
  { key: 'hihat',  name: 'Hi-Hat',  padId: 2,  kind: 'hihat',  cx: 172, cy: 310, rx:  94, ry: 40 },
  { key: 'tomH',   name: 'Tom 1',   padId: 4,  kind: 'tom',    cx: 432, cy: 262, rx:  72, ry: 64 },
  { key: 'tomM',   name: 'Tom 2',   padId: 5,  kind: 'tom',    cx: 568, cy: 262, rx:  72, ry: 64 },
  { key: 'floor',  name: 'Floor',   padId: 6,  kind: 'floor',  cx: 738, cy: 376, rx:  82, ry: 74 },
  { key: 'snare',  name: 'Snare',   padId: 1,  kind: 'snare',  cx: 294, cy: 376, rx:  80, ry: 72 },
  { key: 'kick',   name: 'Kick',    padId: 0,  kind: 'kick',   cx: 500, cy: 420, rx: 128, ry: 114 },
];

const STAND_LINES = [
  'M 106 530 L 108 440 L 185 348',
  'M 250 530 L 172 380',
  'M 848 525 L 848 445 L 800 282',
  'M 500 520 L 500 488',
  'M 500 306 L 500 228',
  'M 500 228 L 258 156',
  'M 500 228 L 640 156',
  'M 500 220 L 474 141',
  'M 572 248 L 782 212',
  'M 412 256 L 174 302',
];

function arcPath(cx: number, cy: number, rx: number, ry: number, scale: number, a0: number, a1: number): string {
  const r = Math.PI / 180;
  const x1 = cx + Math.cos(a0 * r) * rx * scale;
  const y1 = cy + Math.sin(a0 * r) * ry * scale;
  const x2 = cx + Math.cos(a1 * r) * rx * scale;
  const y2 = cy + Math.sin(a1 * r) * ry * scale;
  return `M ${x1.toFixed(1)} ${y1.toFixed(1)} A ${(rx * scale).toFixed(1)} ${(ry * scale).toFixed(1)} 0 0 1 ${x2.toFixed(1)} ${y2.toFixed(1)}`;
}

export const DrumView: React.FC<DrumViewProps> = ({
  pads, onSelect, onPlay, osmpBar, onOsmpTrigger,
  osmpZones, padMidiNotes, activePadId,
}) => {
  const [hitKey, setHitKey] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Mirror activePadId flashes onto the drum SVG animation
  useEffect(() => {
    if (activePadId == null) return;
    const piece = PIECES.find(p => p.padId === activePadId);
    if (!piece) return;
    if (timerRef.current) clearTimeout(timerRef.current);
    setHitKey(piece.key);
    timerRef.current = setTimeout(() => setHitKey(null), 440);
  }, [activePadId]);

  const handleHit = useCallback((padId: number, key: string) => {
    onSelect(padId);
    onPlay?.(padId);
    onOsmpTrigger?.(padId);
    if (timerRef.current) clearTimeout(timerRef.current);
    setHitKey(key);
    timerRef.current = setTimeout(() => setHitKey(null), 440);
  }, [onSelect, onPlay, onOsmpTrigger]);

  return (
    <div className="flex-1 flex flex-col min-h-0 bg-[#060909]">
      {osmpBar}
      <div className="flex-1 min-h-0">
      <svg
        className="w-full h-full"
        viewBox="0 0 1000 580"
        preserveAspectRatio="xMidYMid meet"
      >
        <defs>
          {/* Grid patterns */}
          <pattern id="dvGMaj" width="80" height="80" patternUnits="userSpaceOnUse">
            <path d="M 80 0 L 0 0 0 80" fill="none" stroke="#0e1517" strokeWidth="1" />
          </pattern>
          <pattern id="dvGMin" width="16" height="16" patternUnits="userSpaceOnUse">
            <path d="M 16 0 L 0 0 0 16" fill="none" stroke="#0a1113" strokeWidth="0.5" />
          </pattern>

          {/* Active glow filter */}
          <filter id="dvGlow" x="-55%" y="-55%" width="210%" height="210%">
            <feGaussianBlur stdDeviation="5" result="blur" />
            <feColorMatrix in="blur" type="matrix"
              values="0 0 0 0 0.49  0 0 0 0 0.97  0 0 0 0 1  0 0 0 0.75 0"
              result="glow" />
            <feMerge>
              <feMergeNode in="glow" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>

          {/* Radial fills */}
          <radialGradient id="dvFillCymbal" cx="50%" cy="50%" r="50%">
            <stop offset="0%"   stopColor={CYAN} stopOpacity="0.16" />
            <stop offset="100%" stopColor={CYAN} stopOpacity="0.03" />
          </radialGradient>
          <radialGradient id="dvFillDrum" cx="50%" cy="50%" r="50%">
            <stop offset="0%"   stopColor={CYAN} stopOpacity="0.22" />
            <stop offset="100%" stopColor={CYAN} stopOpacity="0.05" />
          </radialGradient>
          <radialGradient id="dvFillKick" cx="50%" cy="50%" r="50%">
            <stop offset="0%"   stopColor={CYAN} stopOpacity="0.28" />
            <stop offset="100%" stopColor={CYAN} stopOpacity="0.06" />
          </radialGradient>
        </defs>

        {/* Background + grid */}
        <rect x="0" y="0" width="1000" height="580" fill="#060909" />
        <rect x="0" y="0" width="1000" height="580" fill="url(#dvGMin)" />
        <rect x="0" y="0" width="1000" height="580" fill="url(#dvGMaj)" />

        {/* Stand / arm lines */}
        <g fill="none" stroke={CYAN_DIM} strokeWidth="1.5" strokeLinecap="round" opacity="0.85">
          {STAND_LINES.map((d, i) => <path key={i} d={d} />)}
        </g>

        {/* Kit label */}
        <text x="500" y="26" textAnchor="middle"
          fontFamily="ui-monospace, Consolas, 'Courier New', monospace"
          fontSize={10} fill={CYAN} opacity={0.22} letterSpacing={5}>
          DRUM KIT
        </text>

        {/* Drum pieces */}
        {PIECES.map((p) => {
          const pad      = pads[p.padId];
          const loaded   = !!pad?.filePath;
          const isActive = !!pad?.isActive;
          const isHit    = hitKey === p.key;
          const isCymbal = p.kind === 'cymbal' || p.kind === 'hihat';

          const stroke   = (loaded || isActive) ? CYAN : CYAN_DIM;
          const strokeW  = isActive ? 2.4 : 1.6;
          const fillId   = p.kind === 'kick' ? 'dvFillKick' : isCymbal ? 'dvFillCymbal' : 'dvFillDrum';
          const baseOp   = loaded ? (isCymbal ? 0.72 : 0.82) : 0.38;

          // Concentric ring scale list
          const rings = isCymbal
            ? [0.18, 0.35, 0.52, 0.70, 0.87]
            : p.kind === 'kick'
              ? [0.20, 0.42, 0.65, 0.84]
              : [0.22, 0.50, 0.78];

          return (
            <g
              key={p.key}
              style={{ cursor: 'pointer' }}
              onMouseDown={() => handleHit(p.padId, p.key)}
              filter={isActive ? 'url(#dvGlow)' : undefined}
            >
              {/* Filled body */}
              <ellipse cx={p.cx} cy={p.cy} rx={p.rx} ry={p.ry}
                fill={loaded ? `url(#${fillId})` : 'transparent'}
                stroke="none" />

              {/* Outer stroke ring */}
              <ellipse cx={p.cx} cy={p.cy} rx={p.rx} ry={p.ry}
                fill="none"
                stroke={stroke}
                strokeWidth={strokeW}
                opacity={baseOp} />

              {/* Concentric inner rings */}
              {rings.map((s, i) => (
                <ellipse key={i} cx={p.cx} cy={p.cy}
                  rx={p.rx * s} ry={p.ry * s}
                  fill="none"
                  stroke={stroke}
                  strokeWidth={isCymbal ? 0.7 : 0.8}
                  opacity={loaded ? (isCymbal ? 0.30 : 0.20) : 0.10} />
              ))}

              {/* Cymbal: arc highlight sweeps (lower-right quadrant) */}
              {isCymbal && [
                { s: 0.93, a0: 195, a1: 345 },
                { s: 0.74, a0: 205, a1: 335 },
                { s: 0.55, a0: 215, a1: 325 },
              ].map((a, i) => (
                <path key={`arc-${i}`}
                  d={arcPath(p.cx, p.cy, p.rx, p.ry, a.s, a.a0, a.a1)}
                  fill="none"
                  stroke={stroke}
                  strokeWidth={0.7}
                  opacity={loaded ? 0.40 : 0.14} />
              ))}

              {/* Snare: cross-wire lines */}
              {p.kind === 'snare' && (
                <line
                  x1={p.cx - p.rx * 0.72} y1={p.cy}
                  x2={p.cx + p.rx * 0.72} y2={p.cy}
                  stroke={stroke} strokeWidth={0.7}
                  strokeDasharray="4 4" opacity={loaded ? 0.35 : 0.12} />
              )}

              {/* Kick: bass-port rectangle */}
              {p.kind === 'kick' && (
                <rect
                  x={p.cx - 18} y={p.cy - 10}
                  width={36} height={20}
                  rx={4}
                  fill="none"
                  stroke={stroke} strokeWidth={1}
                  opacity={loaded ? 0.4 : 0.15} />
              )}

              {/* ── Hit animation ── */}
              {isHit && (
                <g>
                  {/* Outer expanding ring */}
                  <ellipse cx={p.cx} cy={p.cy} rx={p.rx} ry={p.ry}
                    fill="none" stroke={CYAN} strokeWidth="2.5">
                    <animate attributeName="rx" from={p.rx} to={p.rx * 2.0} dur="0.42s" fill="freeze" />
                    <animate attributeName="ry" from={p.ry} to={p.ry * 2.0} dur="0.42s" fill="freeze" />
                    <animate attributeName="opacity" from="0.80" to="0" dur="0.42s" fill="freeze" />
                    <animate attributeName="stroke-width" from="2.5" to="0.4" dur="0.42s" fill="freeze" />
                  </ellipse>

                  {/* Mid expanding ring */}
                  <ellipse cx={p.cx} cy={p.cy} rx={p.rx * 0.55} ry={p.ry * 0.55}
                    fill="none" stroke={CYAN} strokeWidth="2">
                    <animate attributeName="rx" from={p.rx * 0.55} to={p.rx * 1.55} dur="0.32s" fill="freeze" />
                    <animate attributeName="ry" from={p.ry * 0.55} to={p.ry * 1.55} dur="0.32s" fill="freeze" />
                    <animate attributeName="opacity" from="0.65" to="0" dur="0.32s" fill="freeze" />
                  </ellipse>

                  {/* Inner flash fill */}
                  <ellipse cx={p.cx} cy={p.cy} rx={p.rx * 0.90} ry={p.ry * 0.90}
                    fill={CYAN} stroke="none">
                    <animate attributeName="opacity" from="0.16" to="0" dur="0.28s" fill="freeze" />
                  </ellipse>
                </g>
              )}

              {/* Label */}
              {(() => {
                const midiNote = padMidiNotes?.[p.padId] ?? (p.padId + 36);
                const zone = osmpZones?.pad_slots.find(s => s.note === midiNote) ?? null;
                const displayLabel = zone?.label || pad?.label || p.name.toUpperCase();
                const noteLabel = noteName(midiNote);
                const hasZone = !!zone;
                const labelSize = p.kind === 'kick' ? 14 : 11;
                const noteY = p.cy + labelSize * 1.6;
                return (
                  <>
                    <text
                      x={p.cx} y={p.cy + 5}
                      textAnchor="middle"
                      fontFamily="ui-monospace, Consolas, 'Courier New', monospace"
                      fontSize={labelSize}
                      fill={CYAN}
                      opacity={loaded ? 0.72 : 0.28}>
                      {displayLabel}
                    </text>
                    {hasZone && (
                      <text
                        x={p.cx} y={noteY}
                        textAnchor="middle"
                        fontFamily="ui-monospace, Consolas, 'Courier New', monospace"
                        fontSize={8}
                        fill={CYAN}
                        opacity={loaded ? 0.30 : 0.14}>
                        {noteLabel}{zone.zone_count > 1 ? ` ·${zone.zone_count}L` : ''}
                      </text>
                    )}
                  </>
                );
              })()}
            </g>
          );
        })}
      </svg>
      </div>
    </div>
  );
};
