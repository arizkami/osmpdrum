
// Interface for the IPC messages
interface AudioCommand {
    command: 'Play' | 'Stop' | 'Load';
    payload: any;
}

export class AudioEngine {
    constructor() {
        // No initialization needed for Wry IPC
    }

    private send(command: string, payload: any) {
        if ((window as any).ipc) {
            (window as any).ipc.postMessage(JSON.stringify({ command, payload }));
        } else {
            console.warn('Wry IPC not available');
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

    public confirmExit() {
        this.send('ConfirmExit', {});
    }
}

export const audioEngine = new AudioEngine();
