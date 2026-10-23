import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    // Tauri/Cargo watches Rust source itself. Vite must not watch Rust build outputs
    // because Windows locks compiled .exe files during builds.
    watch: {
      ignored: [
        '**/src-tauri/**',
      ],
    },
  },
});
