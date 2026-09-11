// @ts-nocheck
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'

function safeParseUser() {
  try {
    const raw = localStorage.getItem('gl_user')
    if (!raw || raw === 'null' || raw === 'undefined') return null
    return JSON.parse(raw)
  } catch {
    try { localStorage.removeItem('gl_user') } catch {}
    return null
  }
}
function safeGetToken() {
  try { return localStorage.getItem('gl_token') || null } catch { return null }
}
function safeSetItem(k: string, v: string) { try { localStorage.setItem(k, v) } catch {} }
function safeRemoveItem(k: string) { try { localStorage.removeItem(k) } catch {} }

export const useAuthStore = defineStore('auth', () => {
  const user = ref(safeParseUser())
  const token = ref(safeGetToken())

  const isAuthenticated = computed(() => !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')
  const isEmploye = computed(() => user.value?.role === 'employe')

  async function login(email, password) {
    console.log('[AUTH_START] login attempt for', email)
    console.log('[AUTH_STATE_UPDATE] before login, isAuthenticated=', isAuthenticated.value)
    const data = await api.post('/auth/login', { email, password })
    console.log('[AUTH_SUCCESS] login success, user role', data.user?.role)
    token.value = data.token
    user.value = data.user
    safeSetItem('gl_token', data.token)
    console.log('[TOKEN_SAVE] token saved length', data.token?.length)
    safeSetItem('gl_user', JSON.stringify(data.user))
    console.log('[AUTH_STATE_UPDATE] after login, isAuthenticated=', !!token.value && !!user.value)
    return data.user
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    token.value = null
    user.value = null
    safeRemoveItem('gl_token')
    safeRemoveItem('gl_user')
  }

  async function fetchMe() {
    try {
      const data = await api.get('/auth/me')
      user.value = data.user
      safeSetItem('gl_user', JSON.stringify(data.user))
    } catch {
      await logout()
    }
  }

  return { user, token, isAuthenticated, isAdmin, isEmploye, login, logout, fetchMe }
})
