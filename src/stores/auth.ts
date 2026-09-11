// @ts-nocheck
// Best-practice auth store : offline-first + Neon sync + JWT pair (access 24h + refresh 7j)
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
function safeGetRefresh() {
  try { return localStorage.getItem('gl_refresh_token') || null } catch { return null }
}
function safeSetItem(k: string, v: string) { try { localStorage.setItem(k, v) } catch {} }
function safeRemoveItem(k: string) { try { localStorage.removeItem(k) } catch {} }

export const useAuthStore = defineStore('auth', () => {
  const user = ref(safeParseUser())
  const token = ref(safeGetToken())
  const refreshToken = ref(safeGetRefresh())
  const loginSource = ref(localStorage.getItem('gl_login_source') || null) // 'local' | 'neon' | 'debug'
  const lastSync = ref(localStorage.getItem('gl_last_sync') || null)

  const isAuthenticated = computed(() => !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')
  const isEmploye = computed(() => user.value?.role === 'employe')
  const isDebug = computed(() => token.value === 'debug-bypass-android14')

  // Best-practice login : offline-first, Neon sync, Argon2, token pair
  async function login(email, password) {
    console.log('[AUTH_START] login attempt for', email)
    console.log('[AUTH_STATE_UPDATE] before login, isAuthenticated=', isAuthenticated.value)
    const data = await api.post('/auth/login', { email, password })
    console.log('[AUTH_SUCCESS] login success via', data.source, 'role', data.user?.role)
    token.value = data.token
    user.value = data.user
    if (data.refresh_token) {
      refreshToken.value = data.refresh_token
      safeSetItem('gl_refresh_token', data.refresh_token)
    }
    if (data.source) {
      loginSource.value = data.source
      safeSetItem('gl_login_source', data.source)
    }
    safeSetItem('gl_token', data.token)
    console.log('[TOKEN_SAVE] token len', data.token?.length, 'refresh len', data.refresh_token?.length)
    safeSetItem('gl_user', JSON.stringify(data.user))
    console.log('[AUTH_STATE_UPDATE] after login, isAuthenticated=', !!token.value && !!user.value)
    // Background sync après login (best practice)
    try {
      const sync = await api.get('/sync/status')
      console.log('[NEON_SYNC] status', sync)
      if (sync.neonAvailable) {
        // Pull en arrière-plan non bloquant
        api.post('/sync/run').then(r => {
          console.log('[NEON_SYNC] run result', r)
          lastSync.value = new Date().toISOString()
          safeSetItem('gl_last_sync', lastSync.value)
        }).catch(e => console.warn('[NEON_SYNC] run failed (offline?)', e.message))
      }
    } catch (e) {
      console.warn('[NEON_SYNC] status check failed (offline)', e.message)
    }
    return data.user
  }

  // Refresh token (best practice)
  async function refresh() {
    if (!refreshToken.value) throw new Error('No refresh token')
    console.log('[AUTH_REFRESH] attempting refresh')
    const data = await api.post('/auth/refresh', { refresh_token: refreshToken.value })
    token.value = data.token
    safeSetItem('gl_token', data.token)
    console.log('[AUTH_REFRESH] new token len', data.token?.length)
    return data.token
  }

  // Mode diagnostic Android 14 : bypass sécurité pour tester autres fonctionnalités
  function debugBypassLogin() {
    console.log('[DEBUG_BYPASS] activation mode diagnostic Android 14')
    const debugUser = { id: 1, email: 'debug@gamelounge.com', role: 'admin', nom: 'Debug Android14' }
    const debugToken = 'debug-bypass-android14'
    token.value = debugToken
    user.value = debugUser
    refreshToken.value = null
    loginSource.value = 'debug'
    safeSetItem('gl_token', debugToken)
    safeSetItem('gl_user', JSON.stringify(debugUser))
    safeSetItem('gl_login_source', 'debug')
    try { localStorage.setItem('gl_debug_bypass', '1') } catch {}
    console.log('[DEBUG_BYPASS] token saved, isAuthenticated=', isAuthenticated.value)
    return debugUser
  }

  function isDebugBypass() {
    try { return localStorage.getItem('gl_token') === 'debug-bypass-android14' || localStorage.getItem('gl_debug_bypass') === '1' } catch { return false }
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    token.value = null
    refreshToken.value = null
    user.value = null
    loginSource.value = null
    safeRemoveItem('gl_token')
    safeRemoveItem('gl_refresh_token')
    safeRemoveItem('gl_user')
    safeRemoveItem('gl_login_source')
    try { localStorage.removeItem('gl_debug_bypass') } catch {}
    try { localStorage.removeItem('gl_last_sync') } catch {}
  }

  async function fetchMe() {
    try {
      const data = await api.get('/auth/me')
      user.value = data.user
      safeSetItem('gl_user', JSON.stringify(data.user))
    } catch (e: any) {
      // Best practice : tente refresh sur 401
      if (e.status === 401 && refreshToken.value) {
        try {
          await refresh()
          const data2 = await api.get('/auth/me')
          user.value = data2.user
          safeSetItem('gl_user', JSON.stringify(data2.user))
          return
        } catch {}
      }
      await logout()
      throw e
    }
  }

  // Vérifie auth au boot (best practice)
  async function checkAuth() {
    if (!isAuthenticated.value) return false
    try {
      await fetchMe()
      return true
    } catch {
      return false
    }
  }

  return { user, token, refreshToken, loginSource, lastSync, isAuthenticated, isAdmin, isEmploye, isDebug, login, logout, fetchMe, refresh, checkAuth, debugBypassLogin, isDebugBypass }
})
