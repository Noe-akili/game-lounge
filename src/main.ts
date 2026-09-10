// @ts-nocheck
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { isTauri } from './lib/transport'
import router from './router'
import './assets/main.css'

initClientData()

async function initClientData() {
  // En mode Tauri, la base de données est gérée par le backend Rust (SQLite)
  // : on n'initialise pas la base navigateur.
  if (isTauri()) return

  try {
    const { initClientDb, seedClientDb } = await import('@/lib/clientDb')
    await initClientDb()
    await seedClientDb()
    console.log('✅ Base de données navigateur initialisée')
  } catch (e) {
    console.error('❌ Erreur initialisation base de données:', e)
  }
}

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.mount('#app')