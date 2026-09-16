// @ts-nocheck
// Auth : Session en MÉMOIRE UNIQUEMENT — AUCUNE persistance entre démarrages.
//
// Comportement :
//   App start -> écran LOGIN systématique (token/user en mémoire à null)
//   Login OK -> session conservée en mémoire pendant l'utilisation
//   Navigation -> session active
//   Logout -> session effacée
//   Fermeture de l'app -> mémoire libérée, l'utilisateur doit se reconnecter
//   401 -> session effacée -> retour écran login

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'
import { SessionStorage } from '@/lib/sessionStorage'

export type AuthState = 'unauthenticated' | 'authenticated' | 'error'

function log(tag: string, msg: string) {
  console.log(`[${tag}] ${msg}`)
}

export const useAuthStore = defineStore('auth', () => {
  // Session en mémoire uniquement : initialisée à null à chaque démarrage
  const token = ref<string | null>(null)
  const user = ref<any | null>(null)
  const state = ref<AuthState>('unauthenticated')
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
    log('AUTH', `login OK via ${data.source || 'local'} (role=${data.user?.role})`)
    return data.user
  }

  async function setSessionFromBootstrap(data: any) {
    await setSession(data)
    log('AUTH', `session de secours active (source=${data?.source || 'kiosk-admin'}, role=${data?.user?.role})`)
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    try { await api.post('/auth/business-suspend') } catch {}
    await clearSession()
    log('AUTH', 'logout')
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

  // Au boot, aucune restauration automatique : l'utilisateur doit se reconnecter
  function restoreSession(): Promise<boolean> {
    if (token.value && user.value) {
      state.value = 'authenticated'
      return Promise.resolve(true)
    }
    state.value = 'unauthenticated'
    log('AUTH', 'boot sans session persistée -> écran login')
    return Promise.resolve(false)
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
    login, logout, fetchMe, restoreSession, refreshSession, clearSession, setSessionFromBootstrap,
    businessReady, businessSuspend,
  }
})
