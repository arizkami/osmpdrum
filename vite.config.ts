// import path from 'path';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { viteSingleFile } from 'vite-plugin-singlefile';
export default defineConfig(() => {
    return {
      plugins: [react(), tailwindcss(), viteSingleFile()],
    };
});
