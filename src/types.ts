export interface PadData {
    id: number;
    label: string;
    isMuted: boolean;
    isSolo: boolean;
    isActive: boolean;
    filePath?: string;
    waveformPeaks?: number[];
    duration?: number; // seconds
}

export interface KnobProps {
    value: number;
    label: string;
    onChange: (value: number) => void;
    min?: number;
    max?: number;
    suffix?: string;
}