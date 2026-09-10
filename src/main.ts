// @ts-nocheck
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { isTauri, waitForTauri } from './lib/transport'
import router from './router'
import './assets/main.css'

// === Android / Tauri hardening : jamais de white screen sur erreur non catchée ===
if (typeof window !== 'undefined') {
  window.addEventListener('error', (e) => {
    console.error('[global error]', e.error || e.message)
    // Sur Android WebView, une erreur non catchée = white screen -> on évite le crash
    e.preventDefault?.()
  })
  window.addEventListener('unhandledrejection', (e) => {
    console.error('[unhandled rejection]', e.reason)
    e.preventDefault?.()
  })
  // Fix Android WebView : viewport height sur clavier virtuel
  const setVH = () => {
    const vh = window.innerHeight * 0.01
    document.documentElement.style.setProperty('--vh', `${vh}px`)
  }
  window.addEventListener('resize', setVH)
  window.addEventListener('orientationchange', () => setTimeout(setVH, 300))
  setVH()
}

initClientData()

async function initClientData() {
  // En mode Tauri (APK), la base est gérée par Rust/SQLite - ne pas init la DB navigateur.
  // On attend brièvement l'injection de __TAURI__ pour éviter le race au boot APK
  try {
    const isTauriMode = await waitForTauri(1500)
    if (isTauriMode || isTauri()) {
      console.log('✅ Mode Tauri détecté - DB navigateur ignorée')
      return
    }
  } catch (e) {
    console.warn('[init] waitForTauri failed', e)
  }

  try {
    const { initClientDb, seedClientDb } = await import('@/lib/clientDb')
    await initClientDb()
    await seedClientDb()
    console.log('✅ Base de données navigateur initialisée')
  } catch (e) {
    console.error('❌ Erreur initialisation base de données (non-fatal):', e)
  }
}

const app = createApp(App)
// Handler d'erreur Vue global : évite crash sur render error Android low-end
app.config.errorHandler = (err, instance, info) => {
  console.error('[Vue error]', err, info)
}
app.config.warnHandler = (msg) => {
  console.warn('[Vue warn]', msg)
}
app.use(createPinia())
app.use(router)
app.mount('#app')

// === Android back button : fermer modale/sidebar avant de quitter ===
if (typeof window !== 'undefined') {
  let backPressCount = 0
  window.addEventListener('popstate', () => {
    // Si l'utilisateur appuie sur back à la racine, on évite la fermeture brutale
    // Le router gère déjà, on log seulement
    console.log('[android] back pressed', location.pathname)
  })
  // Alternative Tauri : écoute l'événement Android back si plugin présent
  try {
    const w: any = window as any
    if (w.__TAURI__?.event?.listen) {
      w.__TAURI__.event.listen('android-back', () => {
        // Tente de fermer la sidebar/modal, sinon router.back()
        window.dispatchEvent(new CustomEvent('android-back-pressed'))
      })
    }
  } catch {}
}