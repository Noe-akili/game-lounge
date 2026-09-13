// @ts-nocheck
// Auth minimal propre - Tauri + Supabase
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
  const sessionReady = ref(false)

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

  function clearSession() {
    token.value = null
    user.value = null
    try {
      localStorage.removeItem('gl_token')
      localStorage.removeItem('gl_user')
      localStorage.removeItem('gl_refresh_token')
    } catch {}
  }

  async function bootstrapAdmin() {
    const data = await api.post("/auth/bootstrap")
    token.value = data.token
    user.value = data.user
    localStorage.setItem("gl_token", data.token)
    localStorage.setItem("gl_user", JSON.stringify(data.user))
    if (data.refresh_token) localStorage.setItem("gl_refresh_token", data.refresh_token)
    return data.user
  }

  async function restoreSession() {
    if (sessionReady.value) return isAuthenticated.value
    try {
      if (!token.value || !user.value) { await bootstrapAdmin(); return true }
      await fetchMe()
      return true
    } catch {
      clearSession()
      try { await bootstrapAdmin(); return true } catch { return false }
    } finally {
      sessionReady.value = true
    }
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    clearSession()
  }

  async function fetchMe() {
    const data = await api.get('/auth/me')
    user.value = data.user
    localStorage.setItem('gl_user', JSON.stringify(data.user))
  }

  if (typeof window !== 'undefined') {
    window.addEventListener('gl:unauthorized', clearSession)
  }

  return { user, token, sessionReady, isAuthenticated, isAdmin, login, logout, fetchMe, bootstrapAdmin, restoreSession, clearSession }
})
