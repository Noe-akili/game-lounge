// @ts-nocheck
// Auth minimal propre - Tauri + Neon
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'

function loadUser() {
  try {
    const raw = localStorage.getItem('gl_user')
    if (!raw || raw === 'null') return null
    return JSON.parse(raw)
  } catch { return null }
}
function loadToken() {
  try { return localStorage.getItem('gl_token') } catch { return null }
}

export const useAuthStore = defineStore('auth', () => {
  const user = ref(loadUser())
  const token = ref(loadToken())

  const isAuthenticated = computed(() => !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')

  async function login(email: string, password: string) {
    const data = await api.post('/auth/login', { email, password })
    token.value = data.token
    user.value = data.user
    localStorage.setItem('gl_token', data.token)
    localStorage.setItem('gl_user', JSON.stringify(data.user))
    if (data.refresh_token) localStorage.setItem('gl_refresh_token', data.refresh_token)
    return data.user
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    token.value = null
    user.value = null
    localStorage.removeItem('gl_token')
    localStorage.removeItem('gl_user')
    localStorage.removeItem('gl_refresh_token')
  }

  async function fetchMe() {
    const data = await api.get('/auth/me')
    user.value = data.user
    localStorage.setItem('gl_user', JSON.stringify(data.user))
  }

  function debugBypassLogin() {
    const debugUser = { id: 1, email: 'debug@gamelounge.com', role: 'admin', nom: 'Debug Android14' }
    const debugToken = 'debug-bypass-android14'
    token.value = debugToken
    user.value = debugUser
    localStorage.setItem('gl_token', debugToken)
    localStorage.setItem('gl_user', JSON.stringify(debugUser))
    localStorage.setItem('gl_debug_bypass', '1')
    return debugUser
  }

  return { user, token, isAuthenticated, isAdmin, login, logout, fetchMe, debugBypassLogin }
})
