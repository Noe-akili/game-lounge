// @ts-nocheck
// Auth : Session persistante sur l'appareil — Reconnexion automatique au démarrage.
//
// Comportement :
//   App start -> vérifie la session dans localStorage.
//   Si session présente -> utilisateur connecté, redirection directe vers l'écran d'accueil.
//   Si session absente -> redirection vers l'écran de login.
//   Logout -> session supprimée du téléphone et retour sur l'écran de login.
//   401 -> session expirée et supprimée.

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'
import { SessionStorage } from '@/lib/sessionStorage'

export type AuthState = 'unauthenticated' | 'authenticated' | 'error'

function log(tag: string, msg: string) {
  // Trace lisible dans l'écran Développeur (capture de console.log).
  console.log(`[${tag}] ${msg}`)
}

export const useAuthStore = defineStore('auth', () => {
  // Initialisation immédiate depuis le stockage persistant
  const initial = SessionStorage.getSessionSync()
  const token = ref<string | null>(initial.token || null)
  const user = ref<any | null>(initial.user || null)
  const state = ref<AuthState>(initial.token && initial.user ? 'authenticated' : 'unauthenticated')
  const lastError = ref<string | null>(null)

  const isAuthenticated = computed(() => state.value === 'authenticated' && !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')

  async function setSession(data: any) {
    if (!data?.token || !data?.user) {
      log('AUTH', 'setSession refusé: token/user manquant dans la réponse')
      throw new Error('Réponse de connexion invalide')
    }
    token.value = data.token
    user.value = data.user
    state.value = 'authenticated'
    await SessionStorage.saveSession({
      token: data.token,
      refresh_token: data.refresh_token || null,
      user: data.user,
    })
  }

  async function clearSession() {
    token.value = null
    user.value = null
    state.value = 'unauthenticated'
    await SessionStorage.clearSession()
  }

  async function login(email: string, password: string) {
    lastError.value = null
    const data = await api.post('/auth/login', { email, password })
    await setSession(data)
    log('AUTH', 'connexion réussie, session enregistrée sur l\'appareil')
    return data.user
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    try { await api.post('/auth/business-suspend') } catch {}
    await clearSession()
    log('AUTH', 'logout effectué, session supprimée du téléphone')
  }

  async function fetchMe() {
    if (!token.value) {
      throw Object.assign(new Error('Aucune session'), { status: 401 })
    }
    const data = await api.get('/auth/me')
    if (data?.user) {
      user.value = data.user
      await SessionStorage.saveSession({ token: token.value, refresh_token: null, user: data.user })
    }
  }

  async function restoreSession(): Promise<boolean> {
    const s = SessionStorage.getSessionSync()
    if (s.token && s.user) {
      token.value = s.token
      user.value = s.user
      state.value = 'authenticated'
      log('AUTH', 'session restauree depuis le stockage local')
      return true
    }
    // Stockage WebView vide (cas Android frequent) : tenter la sauvegarde
    // durable cote Rust/SQLite avant de renvoyer l'utilisateur au login.
    try {
      const durable = await SessionStorage.loadDurable()
      if (durable?.token && durable?.user) {
        token.value = durable.token
        user.value = durable.user
        state.value = 'authenticated'
        await SessionStorage.saveSession({
          token: durable.token,
          refresh_token: durable.refresh_token || null,
          user: durable.user,
        })
        log('AUTH', 'session restauree depuis la sauvegarde durable SQLite')
        return true
      }
    } catch {}
    state.value = 'unauthenticated'
    return false
  }

  async function refreshSession(): Promise<boolean> {
    try {
      const rt = SessionStorage.getSessionSync().refresh_token
      if (!rt || !token.value) return false
      const data = await api.post('/auth/refresh', { refresh_token: rt })
      if (!data?.token) return false
      await setSession({ ...data, user: user.value })
      return true
    } catch (e) {
      log('AUTH', 'renouvellement échoué')
      return false
    }
  }

  async function businessReady() {
    try { await api.post('/auth/business-ready') } catch {}
  }

  async function businessSuspend() {
    try { await api.post('/auth/business-suspend') } catch {}
  }

  if (typeof window !== 'undefined') {
    window.addEventListener('gl:unauthorized', () => {
      log('AUTH', '401 reçu -> déconnexion et retour login')
      lastError.value = 'Session expirée, veuillez vous reconnecter'
      businessSuspend()
      clearSession()
    })
  }

  return {
    user, token, state, lastError,
    isAuthenticated, isAdmin,
    login, logout, fetchMe, restoreSession, refreshSession, clearSession,
    businessReady, businessSuspend,
  }
})
