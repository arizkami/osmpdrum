
const WS_URL = `ws://${window.location.host}/ws`;

export class AudioEngine {
    private ws: WebSocket | null = null;
    private queue: string[] = [];
    private ready = false;

    constructor() {
        this.connect();
    }

    private connect() {
        const ws = new WebSocket(WS_URL);
        this.ws = ws;

        ws.onopen = () => {
            this.ready = true;
            this.queue.forEach(msg => ws.send(msg));
            this.queue = [];
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data as string) as { type: string; detail: unknown };
                window.dispatchEvent(new CustomEvent(msg.type, { detail: msg.detail }));
            } catch (e) {
                console.error('WS message parse error', e);
            }
        };

        ws.onclose = () => {
            this.ready = false;
            // Reconnect after 1 s
            setTimeout(() => this.connect(), 1000);
        };

        ws.onerror = (e) => console.error('WS error', e);
    }

    private send(command: string, payload: any) {
        const msg = JSON.stringify({ command, payload });
        if (this.ready && this.ws?.readyState === WebSocket.OPEN) {
            this.ws.send(msg);
        } else {
            this.queue.push(msg);
        }
    }

    public play(padId: number, filePath: string, volume: number = 1, pan: number = 0) {
        this.send('Play', { pad_id: padId, file_path: filePath, volume, pan });
    }

    public stop(padId: number) {
        this.send('Stop', { pad_id: padId });
    }

    public load(padId: number, filePath: string) {
        this.send('Load', { pad_id: padId, file_path: filePath });
    }

    public setMasterVolume(volume: number) {
        this.send('SetMasterVolume', { volume });
    }

    public setPlaybackLatency(latencyMs: number) {
        this.send('SetPlaybackLatency', { latency_ms: latencyMs });
    }

    public getAudioBackends() {
        this.send('GetAudioBackends', {});
    }

    public getAudioSettings() {
        this.send('GetAudioSettings', {});
    }

    public getAudioDevices(backend: string) {
        this.send('GetAudioDevices', { backend });
    }

    public setPlaybackBackend(backend: string) {
        this.send('SetPlaybackBackend', { backend });
    }

    public setPlaybackDevice(deviceName: string) {
        this.send('SetPlaybackDevice', { device_name: deviceName });
    }

    public setBufferSizeFrames(frames: number) {
        this.send('SetBufferSizeFrames', { frames });
    }

    public confirmExit() {
        this.send('ConfirmExit', {});
    }

    public listDirectory(path?: string, filter?: string[]) {
        this.send('ListDirectory', { path: path ?? null, filter: filter ?? null });
    }

    public getDrives() {
        this.send('GetDrives', {});
    }

    public getOsmpZones() {
        this.send('GetOsmpZones', {});
    }

    public setOsmpCC(cc_num: number, value: number) {
        this.send('SetOsmpCC', { cc_num, value });
    }

    public getPresets() {
        this.send('GetPresets', {});
    }

    public getLibrary() {
        this.send('GetLibrary', {});
    }

    public getMidiInputs() {
        this.send('GetMidiInputs', {});
    }

    public setMidiInput(portName: string | null) {
        this.send('SetMidiInput', { port_name: portName });
    }

    public setWasapiExclusive(exclusive: boolean) {
        this.send('SetWasapiExclusive', { exclusive });
    }

    public setSampleRate(rate: number) {
        this.send('SetSampleRate', { rate });
    }

    public loadOsmp(path: string) {
        this.send('LoadOsmp', { path });
    }

    public noteOn(note: number, vel: number) {
        this.send('NoteOn', { note, vel });
    }

    public warmOsmpCache() {
        this.send('WarmOsmpCache', {});
    }

    public getOsmpInfo() {
        this.send('GetOsmpInfo', {});
    }
}

export const audioEngine = new AudioEngine();
