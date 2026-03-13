import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { viteSingleFile } from 'vite-plugin-singlefile';

export default defineConfig(() => {
    return {
        plugins: [react(), tailwindcss(), viteSingleFile()],
        server: {
            proxy: {
                '/ws': {
                    target: 'ws://127.0.0.1:7878',
                    ws: true,
                    changeOrigin: true,
                },
            },
        },
        build: {
            outDir: 'dist',
        },
    };
});
