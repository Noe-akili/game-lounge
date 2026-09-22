// @ts-nocheck
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/utils/api'

export const useSettingsStore = defineStore('settings', () => {
  const fontMode = ref(localStorage.getItem('gl_font') || 'gaming')
  const themeMode = ref(localStorage.getItem('gl_theme') || 'dark')
  const appName = ref(localStorage.getItem('gl_app_name') || 'Game Lounge')
  const bgMode = ref(localStorage.getItem('gl_bg_mode') || 'default') // 'default' | 'stars' | 'custom'
  const bgCustomImage = ref(localStorage.getItem('gl_bg_custom') || '')

  function setFont(mode: string) {
    fontMode.value = mode
    localStorage.setItem('gl_font', mode)
    applyFont(mode)
  }

  function setTheme(mode: string) {
    themeMode.value = mode
    localStorage.setItem('gl_theme', mode)
    applyTheme(mode)
  }

  function setAppName(name: string) {
    const clean = (name || '').trim().replace(/\s+/g, ' ').slice(0, 40) || 'Game Lounge'
    appName.value = clean
    localStorage.setItem('gl_app_name', clean)
    document.title = clean
  }

  async function saveAppName(name: string) {
    const clean = (name || '').trim().replace(/\s+/g, ' ').slice(0, 40) || 'Game Lounge'
    setAppName(clean)
    try {
      await api.post('/parametres/app', { key: 'app_name', value: clean })
    } catch (e) {
      console.warn('[settings] Échec sauvegarde du nom sur le serveur/Supabase:', e)
    }
  }

  async function fetchRemoteAppName() {
    try {
      const res = await api.get('/parametres/app/app_name')
      if (res && res.value && typeof res.value === 'string' && res.value.trim()) {
        const clean = res.value.trim().slice(0, 40)
        appName.value = clean
        localStorage.setItem('gl_app_name', clean)
        document.title = clean
      }
    } catch {
      // Mode hors ligne
    }
  }

  function applyFont(mode: string) {
    const html = document.documentElement
    html.setAttribute('data-font', mode === 'gaming' ? 'gaming' : 'normal')
  }

  function applyTheme(mode: string) {
    const html = document.documentElement
    if (mode === 'light') {
      html.classList.add('light')
      html.classList.remove('dark')
    } else {
      html.classList.add('dark')
      html.classList.remove('light')
    }
  }

  function init() {
    applyFont(fontMode.value)
    applyTheme(themeMode.value)
    document.title = appName.value
    fetchRemoteAppName()

    if (typeof window !== 'undefined') {
      const tauri = (window as any).__TAURI__?.event || (window as any).__TAURI_INTERNALS__?.event
      if (tauri && typeof tauri.listen === 'function') {
        try {
          tauri.listen('app-setting-changed', (event: any) => {
            if (event?.payload?.key === 'app_name' && event?.payload?.value) {
              setAppName(event.payload.value)
            }
          })
          tauri.listen('sync-completed', () => {
            fetchRemoteAppName()
          })
        } catch {}
      }
    }
  }


  function setBgMode(mode: string) {
    bgMode.value = mode
    localStorage.setItem('gl_bg_mode', mode)
  }

  function setCustomBg(dataUrl: string) {
    bgCustomImage.value = dataUrl
    try {
      localStorage.setItem('gl_bg_custom', dataUrl)
    } catch (e) {
      console.warn('[settings] Quota localStorage dépassé pour le fond:', e)
    }
    setBgMode('custom')
  }

  function removeCustomBg() {
    bgCustomImage.value = ''
    localStorage.removeItem('gl_bg_custom')
    if (bgMode.value === 'custom') {
      setBgMode('default')
    }
  }

  return { fontMode, themeMode, appName, setFont, setTheme, setAppName, saveAppName, fetchRemoteAppName, init }
})
