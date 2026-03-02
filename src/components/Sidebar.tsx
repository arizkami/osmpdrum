import React, { useState, useEffect, useCallback } from 'react';
import {
    Folder, FolderOpen, FileAudio, ChevronRight,
    Music, BookOpen, HardDrive, RefreshCw,
} from 'lucide-react';
import { audioEngine } from '../lib/audioEngine';

type Tab = 'explorer' | 'preset' | 'library';

interface FsEntry {
    name: string;
    path: string;
    is_dir: boolean;
    size: number | null;
}

interface PresetInfo {
    name: string;
    path: string;
}

interface LibraryEntry {
    name: string;
    path: string;
    size: number;
    ext: string;
}

interface SidebarProps {
    onFileSelect?: (path: string) => void;
}

interface TreeNodeState {
    entries: FsEntry[];
    expanded: boolean;
    loading: boolean;
}

const AUDIO_EXTS = new Set(['wav', 'mp3', 'flac', 'ogg', 'aiff']);

function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}K`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
}

const SectionHeader: React.FC<{ label: string; onRefresh: () => void }> = ({ label, onRefresh }) => (
    <div className="px-2.5 py-1.5 border-b border-[#171717] flex items-center justify-between shrink-0">
        <span className="text-[9px] font-bold text-[#333] uppercase tracking-[0.15em]">{label}</span>
        <button
            onClick={onRefresh}
            className="text-[#2e2e2e] hover:text-[#555] transition-colors"
            title="Refresh"
        >
            <RefreshCw size={9} />
        </button>
    </div>
);

export const Sidebar: React.FC<SidebarProps> = ({ onFileSelect }) => {
    const [activeTab, setActiveTab] = useState<Tab>('explorer');

    // File Explorer — lazy tree
    const [rootPath, setRootPath] = useState<string | null>(null);
    const [treeMap, setTreeMap] = useState<Record<string, TreeNodeState>>({});

    // Preset
    const [presets, setPresets] = useState<PresetInfo[]>([]);
    const [isLoadingPresets, setIsLoadingPresets] = useState(false);

    // Library
    const [libraryEntries, setLibraryEntries] = useState<LibraryEntry[]>([]);
    const [isLoadingLibrary, setIsLoadingLibrary] = useState(false);

    useEffect(() => {
        const handleDirListing = (e: CustomEvent) => {
            const { path, entries: ents } = e.detail as { path: string; entries: FsEntry[] };
            setRootPath(prev => (prev === null ? path : prev));
            setTreeMap(prev => ({
                ...prev,
                [path]: { entries: ents, expanded: true, loading: false },
            }));
        };
        const handlePresets = (e: CustomEvent) => {
            setPresets((e.detail as PresetInfo[]) ?? []);
            setIsLoadingPresets(false);
        };
        const handleLibrary = (e: CustomEvent) => {
            setLibraryEntries((e.detail as LibraryEntry[]) ?? []);
            setIsLoadingLibrary(false);
        };

        window.addEventListener('rust-dir-listing', handleDirListing as EventListener);
        window.addEventListener('rust-presets', handlePresets as EventListener);
        window.addEventListener('rust-library', handleLibrary as EventListener);
        return () => {
            window.removeEventListener('rust-dir-listing', handleDirListing as EventListener);
            window.removeEventListener('rust-presets', handlePresets as EventListener);
            window.removeEventListener('rust-library', handleLibrary as EventListener);
        };
    }, []);

    useEffect(() => { audioEngine.listDirectory(); }, []);

    const toggleDir = useCallback((path: string) => {
        setTreeMap(prev => {
            const node = prev[path];
            if (!node) {
                audioEngine.listDirectory(path);
                return { ...prev, [path]: { entries: [], expanded: true, loading: true } };
            }
            return { ...prev, [path]: { ...node, expanded: !node.expanded } };
        });
    }, []);

    const refreshRoot = useCallback(() => {
        if (rootPath !== null) {
            setTreeMap(prev => ({
                ...prev,
                [rootPath]: { ...(prev[rootPath] ?? { entries: [], expanded: true }), loading: true },
            }));
            audioEngine.listDirectory(rootPath);
        } else {
            audioEngine.listDirectory();
        }
    }, [rootPath]);

    const renderTree = (dirPath: string, depth: number): React.ReactNode => {
        const node = treeMap[dirPath];
        if (!node || !node.expanded) return null;

        if (node.loading) {
            return (
                <div style={{ paddingLeft: `${6 + depth * 12}px` }}
                    className="py-[3px] text-[9px] text-[#252525] italic">
                    Loading…
                </div>
            );
        }

        const filtered = node.entries.filter(e => {
            if (e.is_dir) return true;
            const ext = e.name.split('.').pop()?.toLowerCase() ?? '';
            return AUDIO_EXTS.has(ext);
        });

        if (filtered.length === 0) {
            return (
                <div style={{ paddingLeft: `${6 + depth * 12}px` }}
                    className="py-[3px] text-[9px] text-[#252525] italic">
                    No audio files
                </div>
            );
        }

        return filtered.map(entry => {
            if (entry.is_dir) {
                const child = treeMap[entry.path];
                const isExpanded = child?.expanded ?? false;
                return (
                    <div key={entry.path}>
                        <div
                            style={{ paddingLeft: `${6 + depth * 12}px` }}
                            onClick={() => toggleDir(entry.path)}
                            className="flex items-center gap-1 pr-2 py-[3px] cursor-pointer hover:bg-[#131313] group transition-colors"
                        >
                            <ChevronRight
                                size={9}
                                className={`text-[#333] shrink-0 transition-transform duration-150 ${isExpanded ? 'rotate-90' : ''}`}
                            />
                            {isExpanded
                                ? <FolderOpen size={10} className="text-[#555] shrink-0 group-hover:text-[#777]" />
                                : <Folder     size={10} className="text-[#444] shrink-0 group-hover:text-[#666]" />
                            }
                            <span className="text-[10px] text-[#777] group-hover:text-[#aaa] truncate leading-tight">
                                {entry.name}
                            </span>
                        </div>
                        {renderTree(entry.path, depth + 1)}
                    </div>
                );
            } else {
                return (
                    <div
                        key={entry.path}
                        draggable
                        onDragStart={(e) => {
                            e.dataTransfer.effectAllowed = 'copy';
                            e.dataTransfer.setData('application/x-audio-path', entry.path);
                        }}
                        style={{ paddingLeft: `${6 + depth * 12 + 10}px` }}
                        onClick={() => onFileSelect?.(entry.path)}
                        className="flex items-center gap-1 pr-2 py-[3px] cursor-pointer hover:bg-[#131313] group transition-colors"
                    >
                        <FileAudio size={10} className="text-[#3a7a50] shrink-0" />
                        <span className="text-[10px] text-[#5a8a6a] group-hover:text-[#88bb88] truncate flex-1 leading-tight">
                            {entry.name}
                        </span>
                        {entry.size != null && (
                            <span className="text-[7px] text-[#282828] font-mono shrink-0">{formatSize(entry.size)}</span>
                        )}
                    </div>
                );
            }
        });
    };

    const handleTabChange = (tab: Tab) => {
        setActiveTab(tab);
        if (tab === 'preset') {
            setIsLoadingPresets(true);
            audioEngine.getPresets();
        } else if (tab === 'library') {
            setIsLoadingLibrary(true);
            audioEngine.getLibrary();
        }
    };

    const TABS: { id: Tab; label: string; icon: React.ReactNode }[] = [
        { id: 'explorer', label: 'Files', icon: <HardDrive size={10} /> },
        { id: 'preset',   label: 'Preset', icon: <Music size={10} /> },
        { id: 'library',  label: 'Library', icon: <BookOpen size={10} /> },
    ];

    const rootLabel = rootPath
        ? rootPath.split(/[\\/]/).filter(Boolean).pop() ?? 'Home'
        : 'Home';

    return (
        <aside className="w-56 bg-panel-bg border-r border-[#1e1e1e] flex flex-col shrink-0">

            {/* Tab bar */}
            <div className="flex shrink-0 border-b border-[#171717] bg-[#0c0c0c]">
                {TABS.map(tab => (
                    <button
                        key={tab.id}
                        onClick={() => handleTabChange(tab.id)}
                        className={`flex-1 flex flex-col items-center justify-center gap-0.5 py-2 transition-colors relative ${
                            activeTab === tab.id
                                ? 'text-accent-cyan'
                                : 'text-[#333] hover:text-[#555]'
                        }`}
                    >
                        {tab.icon}
                        <span className="text-[8px] font-bold uppercase tracking-[0.12em]">{tab.label}</span>
                        {activeTab === tab.id && (
                            <div className="absolute bottom-0 left-0 right-0 h-[2px] bg-accent-cyan opacity-70" />
                        )}
                    </button>
                ))}
            </div>

            {/* ── FILE EXPLORER ── */}
            {activeTab === 'explorer' && (
                <div className="flex flex-col flex-1 min-h-0">
                    {/* Header row */}
                    <div className="flex items-center gap-1.5 px-2 py-1.5 border-b border-[#171717] shrink-0">
                        <HardDrive size={9} className="text-[#2a2a2a] shrink-0" />
                        <span className="text-[9px] text-[#3a3a3a] font-mono truncate flex-1">{rootLabel}</span>
                        <button
                            onClick={refreshRoot}
                            className="text-[#2e2e2e] hover:text-[#555] transition-colors"
                            title="Refresh"
                        >
                            <RefreshCw size={9} />
                        </button>
                    </div>

                    {/* Tree view */}
                    <div className="flex-1 overflow-y-auto py-0.5">
                        {rootPath === null ? (
                            <div className="flex items-center justify-center h-12 text-[9px] text-[#252525] uppercase tracking-wider">
                                Loading…
                            </div>
                        ) : (
                            renderTree(rootPath, 0)
                        )}
                    </div>

                    <div className="px-2.5 py-1.5 border-t border-[#141414] shrink-0">
                        <p className="text-[8px] text-[#222] italic leading-relaxed">
                            Click a file to load to selected pad
                        </p>
                    </div>
                </div>
            )}

            {/* ── PRESET ── */}
            {activeTab === 'preset' && (
                <div className="flex flex-col flex-1 min-h-0">
                    <SectionHeader label="Saved Presets" onRefresh={() => { setIsLoadingPresets(true); audioEngine.getPresets(); }} />
                    <div className="flex-1 overflow-y-auto py-0.5">
                        {isLoadingPresets ? (
                            <div className="flex items-center justify-center h-12 text-[9px] text-[#252525] uppercase tracking-wider">
                                Loading…
                            </div>
                        ) : presets.length === 0 ? (
                            <div className="px-3 pt-3 space-y-1.5">
                                <p className="text-[9px] text-[#2a2a2a] italic">No presets found.</p>
                                <p className="text-[8px] text-[#1e1e1e] italic leading-relaxed">
                                    Presets are saved to:<br />
                                    <span className="font-mono not-italic text-[#262626]">%APPDATA%\osmpdrum\presets</span>
                                </p>
                            </div>
                        ) : (
                            presets.map(preset => (
                                <div
                                    key={preset.path}
                                    className="flex items-center gap-1.5 px-2 py-1.5 cursor-pointer hover:bg-[#131313] group transition-colors"
                                >
                                    <Music size={11} className="text-[#444] shrink-0 group-hover:text-[#666]" />
                                    <span className="text-[10px] text-[#666] group-hover:text-[#999] truncate">
                                        {preset.name}
                                    </span>
                                </div>
                            ))
                        )}
                    </div>
                </div>
            )}

            {/* ── LIBRARY ── */}
            {activeTab === 'library' && (
                <div className="flex flex-col flex-1 min-h-0">
                    <SectionHeader label="Sample Library" onRefresh={() => { setIsLoadingLibrary(true); audioEngine.getLibrary(); }} />
                    <div className="flex-1 overflow-y-auto py-0.5">
                        {isLoadingLibrary ? (
                            <div className="flex items-center justify-center h-12 text-[9px] text-[#252525] uppercase tracking-wider">
                                Loading…
                            </div>
                        ) : libraryEntries.length === 0 ? (
                            <div className="px-3 pt-3 space-y-1.5">
                                <p className="text-[9px] text-[#2a2a2a] italic">Library is empty.</p>
                                <p className="text-[8px] text-[#1e1e1e] italic leading-relaxed">
                                    Add audio files to:<br />
                                    <span className="font-mono not-italic text-[#262626]">%APPDATA%\osmpdrum\library</span>
                                </p>
                            </div>
                        ) : (
                            libraryEntries.map(entry => (
                                <div
                                    key={entry.path}
                                    onClick={() => onFileSelect?.(entry.path)}
                                    className="flex items-center gap-1.5 px-2 py-[3px] cursor-pointer hover:bg-[#131313] group transition-colors"
                                >
                                    <FileAudio size={11} className="text-[#3a6a80] shrink-0" />
                                    <span className="text-[10px] text-[#5a8a9a] group-hover:text-[#88bbcc] truncate flex-1 leading-tight">
                                        {entry.name}
                                    </span>
                                    <div className="flex flex-col items-end gap-px shrink-0">
                                        <span className="text-[7px] text-[#282828] font-mono uppercase">
                                            {entry.ext}
                                        </span>
                                        <span className="text-[7px] text-[#222] font-mono">
                                            {formatSize(entry.size)}
                                        </span>
                                    </div>
                                </div>
                            ))
                        )}
                    </div>

                    {libraryEntries.length > 0 && (
                        <div className="px-2.5 py-1.5 border-t border-[#141414] shrink-0">
                            <p className="text-[8px] text-[#222]">
                                {libraryEntries.length} sample{libraryEntries.length !== 1 ? 's' : ''}
                            </p>
                        </div>
                    )}
                </div>
            )}
        </aside>
    );
};
