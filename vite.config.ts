import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Порт 5174, а не 5173: 5173 часто занят другим проектом
// на Vite, а запускать их хочется одновременно.
export default defineConfig({
  root: 'src',
  plugins: [react()],
  clearScreen: false,
  server: { port: 5174, strictPort: true },
  build: { outDir: '../dist', emptyOutDir: true, target: 'esnext' },
})
