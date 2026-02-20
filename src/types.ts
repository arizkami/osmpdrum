export interface PadData {
    id: number;
    label: string;
    isMuted: boolean;
    isSolo: boolean;
    isActive: boolean;
    filePath?: string;
    waveformPeaks?: number[];
    duration?: number; // seconds
    audioBuffer?: AudioBuffer; // For browser-based playback
    startPoint?: number; // 0-1 range
    endPoint?: number; // 0-1 range
}

export interface KnobProps {
    value: number;
    label: string;
    onChange: (value: number) => void;
    min?: number;
    max?: number;
    suffix?: string;
}