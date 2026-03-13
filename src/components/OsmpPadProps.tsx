import React from 'react';
import { Music, Layers, Zap, Activity } from 'lucide-react';
import type { OsmpZoneSlot } from '../types/osmp';

const NOTE_NAMES = ['C','C#','D','D#','E','F','F#','G','G#','A','A#','B'];
const noteName = (n: number) => `${NOTE_NAMES[n % 12]}${Math.floor(n / 12) - 1}`;

interface RowProps { label: string; value: React.ReactNode }
const Row: React.FC<RowProps> = ({ label, value }) => (
  <div className="flex justify-between items-center">
    <span className="text-[9px] uppercase tracking-wider text-[#333]">{label}</span>
    <span className="text-[10px] font-mono text-[#7df9ff] bg-black/40 px-1.5 py-0.5 rounded min-w-[44px] text-right">{value}</span>
  </div>
);

function EnvBar({ label, pct }: { label: string; pct: number }) {
  return (
    <div className="flex flex-col items-center gap-0.5">
      <div className="w-4 bg-[#0d0d0d] rounded-sm overflow-hidden" style={{ height: 36 }}>
        <div className="w-full bg-[#7df9ff22] rounded-sm transition-all duration-300"
          style={{ height: `${Math.min(100, pct * 100)}%`, marginTop: `${(1 - Math.min(1, pct)) * 36}px` }} />
      </div>
      <span className="text-[8px] font-mono text-[#2a2a2a]">{label}</span>
    </div>
  );
}

interface OsmpPadPropsProps {
  zone: OsmpZoneSlot | null;
  padLabel?: string;
}

export const OsmpPadProps: React.FC<OsmpPadPropsProps> = ({ zone, padLabel }) => {
  if (!zone) {
    return (
      <div className="p-3 flex flex-col gap-2">
        {[
          { label: 'Root Note', value: '—' },
          { label: 'Key Range', value: '—' },
          { label: 'Vel Range', value: '—' },
          { label: 'Pan', value: '—' },
        ].map(r => <Row key={r.label} {...r} />)}
      </div>
    );
  }

  const rootName   = noteName(zone.root_key);
  const loName     = noteName(zone.lo_key);
  const hiName     = noteName(zone.hi_key);
  const keyRange   = zone.lo_key === zone.hi_key ? rootName : `${loName}–${hiName}`;
  const velRange   = zone.lo_vel === 0 && zone.hi_vel === 127
    ? 'Full' : `${zone.lo_vel}–${zone.hi_vel}`;
  const panStr     = zone.pan === 0 ? 'C'
    : zone.pan > 0 ? `R${Math.round(zone.pan)}` : `L${Math.round(-zone.pan)}`;

  const maxEnv = Math.max(zone.ampeg_attack, zone.ampeg_hold, zone.ampeg_decay, zone.ampeg_release, 0.001);
  const normEnv = (v: number) => v / maxEnv;

  return (
    <div className="flex flex-col gap-2 px-2 py-2 text-xs select-none">
      {/* Zone label header */}
      <div className="flex items-center gap-1.5 border-b border-[#141414] pb-1.5 mb-0.5">
        <Music size={9} className="text-[#7df9ff44]" />
        <span className="text-[10px] font-bold text-[#7df9ff] truncate">{padLabel ?? zone.label}</span>
        <span className="ml-auto text-[9px] font-mono text-[#2a2a2a]">#{zone.note}</span>
      </div>

      {/* Key / vel info */}
      <div className="flex flex-col gap-1.5">
        <Row label="Root"      value={rootName} />
        <Row label="Key Range" value={keyRange} />
        <Row label="Vel Range" value={velRange} />
        <Row label="Pan"       value={panStr} />
        {zone.volume_db !== 0 && <Row label="Volume" value={`${zone.volume_db > 0 ? '+' : ''}${zone.volume_db.toFixed(1)} dB`} />}
      </div>

      {/* Zone metadata */}
      <div className="flex flex-col gap-1.5 border-t border-[#141414] pt-1.5">
        <div className="flex items-center gap-1 mb-0.5">
          <Layers size={8} className="text-[#2a4444]" />
          <span className="text-[8px] uppercase tracking-widest text-[#222]">Zones</span>
        </div>
        <Row label="Layers"  value={zone.zone_count} />
        {zone.seq_length > 1 && <Row label="RR Steps" value={zone.seq_length} />}
        {zone.group_id > 0 && <Row label="Group"   value={zone.group_id} />}
      </div>

      {/* AHDSR envelope visualizer */}
      <div className="border-t border-[#141414] pt-1.5">
        <div className="flex items-center gap-1 mb-2">
          <Activity size={8} className="text-[#2a4444]" />
          <span className="text-[8px] uppercase tracking-widest text-[#222]">Envelope</span>
        </div>
        <div className="flex gap-1.5 justify-around px-1">
          <EnvBar label="A" pct={normEnv(zone.ampeg_attack)} />
          <EnvBar label="H" pct={normEnv(zone.ampeg_hold)} />
          <EnvBar label="D" pct={normEnv(zone.ampeg_decay)} />
          <EnvBar label="S" pct={zone.ampeg_sustain / 100} />
          <EnvBar label="R" pct={normEnv(zone.ampeg_release)} />
        </div>
        <div className="flex justify-around px-1 mt-1">
          {(['attack','hold','decay','sustain','release'] as const).map((k) => {
            const v = zone[`ampeg_${k}` as keyof OsmpZoneSlot] as number;
            const display = k === 'sustain' ? `${v.toFixed(0)}%` : v < 1 ? `${(v*1000).toFixed(0)}ms` : `${v.toFixed(2)}s`;
            return <span key={k} className="text-[7px] font-mono text-[#222] w-5 text-center">{display}</span>;
          })}
        </div>
      </div>

      {/* Sample name */}
      <div className="border-t border-[#141414] pt-1.5">
        <div className="flex items-center gap-1 mb-1">
          <Zap size={8} className="text-[#2a4444]" />
          <span className="text-[8px] uppercase tracking-widest text-[#222]">Sample</span>
        </div>
        <span className="text-[8px] font-mono text-[#2a3a3a] break-all leading-relaxed">{zone.sample}</span>
      </div>
    </div>
  );
};
