// @ts-nocheck
import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useSettingsStore = defineStore('settings', () => {
  const fontMode = ref(localStorage.getItem('gl_font') || 'gaming')
  const themeMode = ref(localStorage.getItem('gl_theme') || 'dark')
  const appName = ref(localStorage.getItem('gl_app_name') || 'Game Lounge')

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
  }

  return { fontMode, themeMode, appName, setFont, setTheme, setAppName, init }
})
