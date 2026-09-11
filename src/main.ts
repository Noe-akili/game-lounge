// @ts-nocheck
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { waitForTauri } from './lib/transport'
import router from './router'
import './assets/main.css'

console.log('[BOOT] main.ts start Tauri-only')
console.log('[BOOT] href=', typeof window !== 'undefined' ? window.location.href : 'no-window')
console.log('[BOOT] userAgent=', typeof navigator !== 'undefined' ? navigator.userAgent : 'no-navigator')

if (typeof window !== 'undefined') {
  window.addEventListener('error', (e) => {
    console.error('[global error]', e.error || e.message)
    e.preventDefault?.()
  })
  window.addEventListener('unhandledrejection', (e) => {
    console.error('[unhandled rejection]', e.reason)
    e.preventDefault?.()
  })
  const setVH = () => {
    const vh = window.innerHeight * 0.01
    document.documentElement.style.setProperty('--vh', `${vh}px`)
  }
  window.addEventListener('resize', setVH)
  window.addEventListener('orientationchange', () => setTimeout(setVH, 300))
  setVH()
}

console.log('[BOOT] waitForTauri check')
waitForTauri(1500).then(ok => console.log('[BOOT] waitForTauri result', ok)).catch(()=>{})

console.log('[BOOT] createApp')
const app = createApp(App)
app.config.errorHandler = (err, instance, info) => {
  console.error('[Vue error]', err, info)
}
app.config.warnHandler = (msg) => {
  console.warn('[Vue warn]', msg)
}
app.use(createPinia())
app.use(router)
console.log('[BOOT] app.mount #app')
app.mount('#app')
console.log('[BOOT] app mounted - Tauri-only mode')

if (typeof window !== 'undefined') {
  window.addEventListener('popstate', () => {
    console.log('[android] back pressed', location.pathname)
  })
  try {
    const w: any = window as any
    if (w.__TAURI__?.event?.listen) {
      w.__TAURI__.event.listen('android-back', () => {
        window.dispatchEvent(new CustomEvent('android-back-pressed'))
      })
    }
  } catch {}
}
