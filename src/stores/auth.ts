// @ts-nocheck
// Best-practice auth store : offline-first + Neon sync + JWT pair + secure storage
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'
import { secureSet, secureGet, secureRemove, migrateFromLocalStorage } from '@/lib/secureStore'

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
  const loginSource = ref(localStorage.getItem('gl_login_source') || null)
  const lastSync = ref(localStorage.getItem('gl_last_sync') || null)

  const isAuthenticated = computed(() => !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')
  const isEmploye = computed(() => user.value?.role === 'employe')
  const isDebug = computed(() => token.value === 'debug-bypass-android14')

  // Migration secure storage au boot (évite localStorage critique)
  ;(async () => {
    try {
      await migrateFromLocalStorage()
      const sToken = await secureGet('gl_token')
      const sRefresh = await secureGet('gl_refresh_token')
      const sUser = await secureGet('gl_user')
      if (sToken && sToken !== 'null') token.value = sToken
      if (sRefresh) refreshToken.value = sRefresh
      if (sUser) {
        try { user.value = JSON.parse(sUser) } catch {}
      }
    } catch {}
  })()

  async function login(email, password) {
    console.log('[AUTH_START] login attempt for', email)
    const data = await api.post('/auth/login', { email, password })
    console.log('[AUTH_SUCCESS] via', data.source, 'role', data.user?.role)
    token.value = data.token
    user.value = data.user
    if (data.refresh_token) {
      refreshToken.value = data.refresh_token
      await secureSet('gl_refresh_token', data.refresh_token)
      safeSetItem('gl_refresh_token', data.refresh_token) // compat
    }
    if (data.source) {
      loginSource.value = data.source
      safeSetItem('gl_login_source', data.source)
    }
    await secureSet('gl_token', data.token)
    await secureSet('gl_user', JSON.stringify(data.user))
    safeSetItem('gl_token', data.token) // compat pour api.ts qui lit localStorage
    safeSetItem('gl_user', JSON.stringify(data.user))
    console.log('[TOKEN_SAVE] secure store saved')
    try {
      const sync = await api.get('/sync/status')
      console.log('[NEON_SYNC] status', sync)
      if (sync.neonAvailable) {
        api.post('/sync/run').then(r => {
          console.log('[NEON_SYNC] run', r)
          lastSync.value = new Date().toISOString()
          safeSetItem('gl_last_sync', lastSync.value)
        }).catch(e => console.warn('[NEON_SYNC] run failed', e.message))
      }
    } catch (e) {
      console.warn('[NEON_SYNC] check failed', e.message)
    }
    return data.user
  }

  async function refresh() {
    if (!refreshToken.value) {
      const s = await secureGet('gl_refresh_token')
      if (s) refreshToken.value = s
      else throw new Error('No refresh token')
    }
    console.log('[AUTH_REFRESH] attempting')
    const data = await api.post('/auth/refresh', { refresh_token: refreshToken.value })
    token.value = data.token
    await secureSet('gl_token', data.token)
    safeSetItem('gl_token', data.token)
    console.log('[AUTH_REFRESH] new token')
    return data.token
  }

  function debugBypassLogin() {
    console.log('[DEBUG_BYPASS] activation')
    const debugUser = { id: 1, email: 'debug@gamelounge.com', role: 'admin', nom: 'Debug Android14' }
    const debugToken = 'debug-bypass-android14'
    token.value = debugToken
    user.value = debugUser
    refreshToken.value = null
    loginSource.value = 'debug'
    // Stockage non critique en localStorage pour debug, mais token critique aussi en secure
    secureSet('gl_token', debugToken)
    secureSet('gl_user', JSON.stringify(debugUser))
    safeSetItem('gl_token', debugToken)
    safeSetItem('gl_user', JSON.stringify(debugUser))
    safeSetItem('gl_login_source', 'debug')
    try { localStorage.setItem('gl_debug_bypass', '1') } catch {}
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
    await secureRemove('gl_token')
    await secureRemove('gl_refresh_token')
    await secureRemove('gl_user')
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
      await secureSet('gl_user', JSON.stringify(data.user))
      safeSetItem('gl_user', JSON.stringify(data.user))
    } catch (e: any) {
      if (e.status === 401 && refreshToken.value) {
        try {
          await refresh()
          const data2 = await api.get('/auth/me')
          user.value = data2.user
          await secureSet('gl_user', JSON.stringify(data2.user))
          safeSetItem('gl_user', JSON.stringify(data2.user))
          return
        } catch {}
      }
      await logout()
      throw e
    }
  }

  async function checkAuth() {
    if (!isAuthenticated.value) return false
    try { await fetchMe(); return true } catch { return false }
  }

  return { user, token, refreshToken, loginSource, lastSync, isAuthenticated, isAdmin, isEmploye, isDebug, login, logout, fetchMe, refresh, checkAuth, debugBypassLogin, isDebugBypass }
})
