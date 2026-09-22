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
    // Un seul fichier CSS : Tailwind est déjà purgé, le découper n'apporte rien.
    cssCodeSplit: false,
    // Cible moderne : la WebView Android 8+ comprend ES2020, donc pas de
    // transpilation inutile (moins de code = APK et démarrage plus légers).
    target: 'es2020',
    // Les gros graphiques ne sont plus inlinés dans le JS.
    assetsInlineLimit: 4096,
    chunkSizeWarningLimit: 300,
    rollupOptions: {
      input: {
        main: 'index.html'
      },
      output: {
        // DÉCOUPAGE DU CODE (remplace inlineDynamicImports) : au démarrage, la
        // WebView ne charge plus que le noyau + l'écran affiché. Les écrans
        // d'administration, les graphiques et la documentation arrivent
        // seulement quand on les ouvre -> premier affichage bien plus rapide
        // et bundle initial divisé.
        entryFileNames: 'assets/app.js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/app.[ext]',
        manualChunks(id) {
          if (!id.includes('node_modules')) return
          // Chart.js n'est utilisé que par les rapports : chargé à la demande.
          if (id.includes('chart.js') || id.includes('vue-chartjs')) return 'graphiques'
          // Animations : utilisées par quelques écrans seulement.
          if (id.includes('motion')) return 'animations'
          // Noyau (vue, router, pinia) : nécessaire tout de suite.
          if (id.includes('/vue/') || id.includes('@vue/') || id.includes('vue-router') || id.includes('pinia')) return 'noyau'
          return 'librairies'
        }
      }
    }
  }
})