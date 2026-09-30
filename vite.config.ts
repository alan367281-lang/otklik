import { resolve } from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Порт 5174, а не 5173: 5173 часто занят другим проектом
// на Vite, а запускать их хочется одновременно.
// Две страницы: окно настроек и всплывающий поиск.
export default defineConfig({
  root: 'src',
  plugins: [react()],
  clearScreen: false,
  server: { port: 5174, strictPort: true },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    target: 'esnext',
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'src/index.html'),
        search: resolve(__dirname, 'src/search.html'),
      },
    },
  },
})
