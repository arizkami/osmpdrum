import React, { useState, useEffect, useCallback, useRef } from 'react';
import { X, Folder, FolderOpen, FileAudio, ChevronRight, HardDrive, ArrowUp } from 'lucide-react';
import { audioEngine } from '../lib/audioEngine';

interface FsEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size?: number;
}

interface DirListing {
  path: string;
  parent?: string;
  entries: FsEntry[];
}

export interface FileBrowserProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (path: string) => void;
  filter?: string[];
  title?: string;
}

function fileIcon(entry: FsEntry) {
  if (entry.is_dir) return <Folder size={13} className="text-[#7df9ff55] shrink-0" />;
  const ext = entry.name.split('.').pop()?.toLowerCase() ?? '';
  if (ext === 'osmp') return <FileAudio size={13} className="text-[#7df9ff88] shrink-0" />;
  if (ext === 'osmpd') return <FileAudio size={13} className="text-[#4ade8088] shrink-0" />;
  return <FileAudio size={13} className="text-[#55555588] shrink-0" />;
}

function formatSize(bytes?: number): string {
  if (bytes == null) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

export const FileBrowser: React.FC<FileBrowserProps> = ({
  isOpen, onClose, onSelect, filter, title = 'Open File',
}) => {
  const [listing, setListing] = useState<DirListing | null>(null);
  const [drives, setDrives] = useState<string[]>([]);
  const [selected, setSelected] = useState<FsEntry | null>(null);
  const [manualPath, setManualPath] = useState('');
  const [loading, setLoading] = useState(false);
  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const navigate = useCallback((path?: string) => {
    setLoading(true);
    setSelected(null);
    audioEngine.listDirectory(path, filter);
  }, [filter]);

  useEffect(() => {
    if (!isOpen) return;
    audioEngine.getDrives();
    navigate();
  }, [isOpen, navigate]);

  useEffect(() => {
    if (!isOpen) return;

    const onDir = (e: CustomEvent) => {
      const d = e.detail as DirListing;
      setListing(d);
      setManualPath(d.path);
      setLoading(false);
    };
    const onDriveList = (e: CustomEvent) => {
      setDrives(e.detail as string[]);
    };

    window.addEventListener('rust-dir-listing', onDir as any);
    window.addEventListener('rust-drives', onDriveList as any);
    return () => {
      window.removeEventListener('rust-dir-listing', onDir as any);
      window.removeEventListener('rust-drives', onDriveList as any);
    };
  }, [isOpen]);

  const handleEntryClick = useCallback((entry: FsEntry) => {
    if (entry.is_dir) {
      if (clickTimer.current) clearTimeout(clickTimer.current);
      clickTimer.current = setTimeout(() => {
        setSelected(entry);
      }, 200);
    } else {
      setSelected(entry);
      setManualPath(entry.path);
    }
  }, []);

  const handleEntryDblClick = useCallback((entry: FsEntry) => {
    if (clickTimer.current) clearTimeout(clickTimer.current);
    if (entry.is_dir) {
      navigate(entry.path);
    } else {
      onSelect(entry.path);
      onClose();
    }
  }, [navigate, onSelect, onClose]);

  const handleSelect = () => {
    const path = manualPath.trim() || selected?.path;
    if (path) { onSelect(path); onClose(); }
  };

  const handleManualGo = () => {
    if (manualPath.trim()) navigate(manualPath.trim());
  };

  const breadcrumbs = (() => {
    if (!listing) return [];
    const sep = listing.path.includes('\\') ? '\\' : '/';
    const parts = listing.path.split(sep).filter(Boolean);
    const crumbs: { label: string; path: string }[] = [];
    parts.forEach((p, i) => {
      const path = (listing.path.includes('\\') ? '' : '/') + parts.slice(0, i + 1).join(sep);
      crumbs.push({ label: p, path: path + (path.endsWith(sep) ? '' : sep) });
    });
    return crumbs;
  })();

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-[2px]"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="w-[680px] h-[480px] flex flex-col bg-[#0d0d0d] border border-[#1e1e1e] rounded-sm shadow-2xl"
        onMouseDown={e => e.stopPropagation()}>

        {/* Title bar */}
        <div className="h-9 flex items-center px-3 border-b border-[#1a1a1a] shrink-0 bg-[#0a0a0a]">
          <FolderOpen size={13} className="text-[#7df9ff66] mr-2" />
          <span className="text-[10px] font-bold uppercase tracking-widest text-[#7df9ff99]">{title}</span>
          <div className="flex-1" />
          <button onClick={onClose}
            className="w-5 h-5 flex items-center justify-center rounded text-[#333] hover:text-[#aaa] hover:bg-[#1a1a1a] transition-colors">
            <X size={12} />
          </button>
        </div>

        {/* Breadcrumb bar */}
        <div className="h-7 flex items-center px-2 gap-0.5 border-b border-[#161616] shrink-0 bg-[#080808] overflow-x-auto">
          <button onClick={() => navigate(listing?.path.includes('\\') ? undefined : '/')}
            className="text-[9px] font-mono text-[#2a2a2a] hover:text-[#7df9ff] px-1 shrink-0 transition-colors">
            Root
          </button>
          {breadcrumbs.map((c, i) => (
            <React.Fragment key={i}>
              <ChevronRight size={9} className="text-[#1e1e1e] shrink-0" />
              <button onClick={() => navigate(c.path)}
                className="text-[9px] font-mono text-[#3a3a3a] hover:text-[#7df9ff] px-1 shrink-0 whitespace-nowrap transition-colors">
                {c.label}
              </button>
            </React.Fragment>
          ))}
        </div>

        {/* Body */}
        <div className="flex flex-1 min-h-0">
          {/* Drive sidebar */}
          {drives.length > 0 && (
            <div className="w-24 border-r border-[#161616] flex flex-col shrink-0 overflow-y-auto bg-[#090909]">
              <div className="px-2 py-1.5 text-[8px] font-bold uppercase tracking-widest text-[#222]">Drives</div>
              {drives.map(d => (
                <button key={d} onClick={() => navigate(d)}
                  className={`flex items-center gap-1.5 px-2 py-1.5 text-[10px] font-mono text-left transition-colors w-full
                    ${listing?.path.startsWith(d.slice(0, 2)) ? 'bg-[#0f1a1a] text-[#7df9ff]' : 'text-[#3a3a3a] hover:text-[#7df9ff] hover:bg-[#0a1010]'}`}>
                  <HardDrive size={11} className="shrink-0" />
                  {d}
                </button>
              ))}
            </div>
          )}

          {/* File list */}
          <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
            {/* Column headers */}
            <div className="flex items-center h-6 px-2 border-b border-[#141414] shrink-0 bg-[#080808]">
              <span className="flex-1 text-[8px] font-bold uppercase tracking-wider text-[#222]">Name</span>
              <span className="w-20 text-right text-[8px] font-bold uppercase tracking-wider text-[#222]">Size</span>
            </div>

            <div className="flex-1 overflow-y-auto">
              {/* Up directory */}
              {listing?.parent && (
                <button
                  onClick={() => navigate(listing.parent)}
                  className="w-full flex items-center gap-2 px-2 py-1 text-left hover:bg-[#0e0e0e] transition-colors group">
                  <ArrowUp size={12} className="text-[#2a2a2a] shrink-0" />
                  <span className="text-[10px] font-mono text-[#2a2a2a] group-hover:text-[#555]">..</span>
                </button>
              )}

              {loading && (
                <div className="flex items-center justify-center py-8">
                  <span className="text-[9px] font-mono text-[#1e1e1e] animate-pulse">Loading…</span>
                </div>
              )}

              {!loading && listing?.entries.map(entry => {
                const isSel = selected?.path === entry.path;
                return (
                  <button
                    key={entry.path}
                    onClick={() => handleEntryClick(entry)}
                    onDoubleClick={() => handleEntryDblClick(entry)}
                    className={`w-full flex items-center gap-2 px-2 py-[3px] text-left transition-colors
                      ${isSel ? 'bg-[#0c1e1e] text-[#7df9ff]' : 'hover:bg-[#0e0e0e] text-[#5a5a5a]'}`}>
                    {fileIcon(entry)}
                    <span className={`flex-1 min-w-0 text-[10px] font-mono truncate ${entry.is_dir ? 'text-[#aaa]' : ''} ${isSel ? 'text-[#7df9ff]' : ''}`}>
                      {entry.name}
                    </span>
                    {!entry.is_dir && (
                      <span className="w-20 text-right text-[8px] font-mono text-[#2a2a2a] shrink-0">
                        {formatSize(entry.size)}
                      </span>
                    )}
                  </button>
                );
              })}

              {!loading && listing && listing.entries.length === 0 && (
                <div className="flex items-center justify-center py-8">
                  <span className="text-[9px] font-mono text-[#1a1a1a]">empty</span>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Bottom bar */}
        <div className="h-10 flex items-center gap-2 px-3 border-t border-[#161616] shrink-0 bg-[#080808]">
          <input
            type="text"
            value={manualPath}
            onChange={e => setManualPath(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter') handleManualGo();
            }}
            placeholder="Path…"
            className="flex-1 bg-[#0c0c0c] border border-[#1c1c1c] rounded px-2 py-0.5 text-[10px] font-mono text-[#aaa] placeholder-[#252525] outline-none focus:border-[#7df9ff22] transition-colors"
          />
          <button onClick={handleManualGo}
            className="h-6 px-2 rounded text-[9px] font-bold uppercase tracking-widest border border-[#1e1e1e] text-[#3a3a3a] hover:text-[#7df9ff] hover:border-[#2a2a2a] transition-colors">
            Go
          </button>
          <div className="w-px h-4 bg-[#1a1a1a]" />
          <button onClick={onClose}
            className="h-6 px-3 rounded text-[9px] font-bold uppercase tracking-widest border border-[#1e1e1e] text-[#3a3a3a] hover:text-[#aaa] transition-colors">
            Cancel
          </button>
          <button onClick={handleSelect}
            disabled={!manualPath.trim() && !selected}
            className="h-6 px-3 rounded text-[9px] font-bold uppercase tracking-widest border border-[#1a3535] bg-[#0a1e1e] text-[#7df9ff] hover:bg-[#0f2828] disabled:opacity-25 disabled:cursor-not-allowed transition-colors">
            Select
          </button>
        </div>
      </div>
    </div>
  );
};
