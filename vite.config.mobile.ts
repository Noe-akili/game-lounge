import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// Build ciblé pour l'application Tauri/Android : le frontend Vue est empaqueté
// dans server-rust/dist puis embarqué par src-tauri (frontendDist "../dist").
export default defineConfig({
  plugins: [vue()],
  base: './',
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  build: {
    outDir: 'server-rust/dist',
    emptyOutDir: true,
    cssCodeSplit: false,
    rollupOptions: {
      input: {
        main: 'index.html'
      },
      output: {
        inlineDynamicImports: true,
        entryFileNames: 'assets/app.js',
        assetFileNames: 'assets/app.[ext]'
      }
    }
  }
})