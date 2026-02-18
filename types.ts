export interface PadData {
    id: number;
    label: string;
    isMuted: boolean;
    isSolo: boolean;
    isActive: boolean;
    audioBuffer?: AudioBuffer;
    fileName?: string;
}

export interface KnobProps {
    value: number;
    label: string;
    onChange: (value: number) => void;
    min?: number;
    max?: number;
    suffix?: string;
}

export interface AudioContextState {
    context: AudioContext | null;
    analyser: AnalyserNode | null;
}