import React, { useState } from 'react';
import { FolderOpen } from 'lucide-react';
import { FileBrowser } from './FileBrowser';

export interface OsmpInfo {
  name: string;
  samples: number;
  zones: number;
  size_mb: number;
}

export interface OsmpBarProps {
  filePath: string;
  onFilePathChange: (p: string) => void;
  onLoad: () => void;
  loading: boolean;
  info: OsmpInfo | null;
  error: string | null;
  velocity: number;
  onVelocityChange: (v: number) => void;
  warmed: boolean;
  warming: boolean;
  onWarm: () => void;
  onLoadPath?: (path: string) => void;
  ccLabels?: Record<string, string>;
  ccState?: Record<string, number>;
  onCcChange?: (cc: number, value: number) => void;
}

export const OsmpBar: React.FC<OsmpBarProps> = ({
  filePath, onFilePathChange, onLoad, loading,
  info, error, velocity, onVelocityChange,
  warmed, warming, onWarm,
  onLoadPath, ccLabels, ccState, onCcChange,
}) => {
  const ccEntries = Object.entries(ccLabels ?? {}).sort((a, b) => Number(a[0]) - Number(b[0]));
  const [browserOpen, setBrowserOpen] = useState(false);

  return (
    <>
    <FileBrowser
      isOpen={browserOpen}
      onClose={() => setBrowserOpen(false)}
      onSelect={(path) => { onFilePathChange(path); setBrowserOpen(false); onLoadPath?.(path); }}
      filter={['osmp']}
      title="Select OSMP Instrument"
    />
    <div className="shrink-0 border-b border-[#141414] bg-[#070a0a]">
      {/* Main row */}
      <div className="flex items-center gap-2 px-2 py-1">
        <button type="button" onClick={() => setBrowserOpen(true)}
          className="text-[#2a3a3a] hover:text-[#7df9ff] transition-colors shrink-0" title="Browse…">
          <FolderOpen size={11} />
        </button>
        <input
          type="text"
          value={filePath}
          onChange={e => onFilePathChange(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && onLoad()}
          placeholder="Path to .osmp instrument…"
          className="flex-1 min-w-0 bg-transparent border border-[#181818] rounded px-1.5 py-0.5 text-[10px] font-mono text-[#999] placeholder-[#282828] outline-none focus:border-[#7df9ff22] focus:text-white transition-colors"
        />
        <button
          type="button"
          onClick={onLoad}
          disabled={loading || !filePath.trim()}
          className="h-5 px-2 rounded text-[8px] font-bold uppercase tracking-widest border border-[#1a3535] bg-[#0a1e1e] text-[#7df9ff] hover:bg-[#0f2828] disabled:opacity-25 disabled:cursor-not-allowed transition-colors shrink-0"
        >
          {loading ? '…' : 'Load'}
        </button>

        {info ? (
          <>
            <div className="w-px h-3 bg-[#1c1c1c] shrink-0" />
            <span className="text-[9px] font-bold text-[#7df9ff] truncate max-w-[120px]">{info.name}</span>
            <span className="text-[8px] font-mono text-[#333]">{info.zones}z</span>
            <span className="text-[8px] font-mono text-[#2a2a2a]">{info.size_mb}mb</span>
            <div className="w-px h-3 bg-[#1c1c1c] shrink-0" />
            <button
              type="button"
              onClick={onWarm}
              disabled={warmed || warming}
              className={`h-5 px-2 rounded text-[8px] font-bold uppercase tracking-widest border transition-colors shrink-0
                ${warmed   ? 'border-[#1a2e1a] text-[#2a5a2a] cursor-default'
                : warming  ? 'border-[#2a3020] text-[#7df9aa] animate-pulse'
                :            'border-[#1a2a1a] text-[#3a7a3a] hover:text-[#7df9aa] hover:border-[#2a3a2a]'}`}
            >
              {warmed ? '✓ Warm' : warming ? 'Warming…' : 'Warm'}
            </button>
            <div className="w-px h-3 bg-[#1c1c1c] shrink-0" />
            <span className="text-[8px] font-mono text-[#2a2a2a] shrink-0">Vel</span>
            <input
              type="range" min={1} max={127} value={velocity}
              onChange={e => onVelocityChange(Number(e.target.value))}
              className="w-16 h-1 accent-[#7df9ff] cursor-pointer shrink-0"
            />
            <span className="text-[9px] font-mono text-[#7df9ff] w-5 text-right tabular-nums shrink-0">{velocity}</span>
          </>
        ) : (
          <span className="text-[8px] font-mono text-[#1e1e1e] italic">no instrument</span>
        )}
      </div>

      {/* CC sliders row — single scrollable row */}
      {info && ccEntries.length > 0 && (
        <div className="flex items-center gap-2 px-2 pb-1 overflow-x-auto" style={{ scrollbarWidth: 'none' }}>
          <span className="text-[8px] font-mono text-[#1e2e2e] uppercase tracking-widest shrink-0 mr-1">CC</span>
          {ccEntries.map(([ccNum, label]) => {
            const val = ccState?.[ccNum] ?? 0;
            return (
              <div key={ccNum} className="flex items-center gap-1 shrink-0">
                <span className="text-[8px] font-mono text-[#2a4040] w-[52px] truncate" title={label}>{label}</span>
                <input
                  type="range" min={0} max={127} value={val}
                  onChange={e => onCcChange?.(Number(ccNum), Number(e.target.value))}
                  className="w-12 h-1 accent-[#7df9ff] cursor-pointer"
                />
                <span className="text-[8px] font-mono text-[#2a5a5a] w-4 tabular-nums text-right">{val}</span>
              </div>
            );
          })}
        </div>
      )}

      {/* Error row */}
      {error && (
        <div className="px-2 pb-1">
          <span className="text-[9px] font-mono text-[#c05050]">{error}</span>
        </div>
      )}
    </div>
    </>
  );
};
