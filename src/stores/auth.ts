// @ts-nocheck
// Auth : LOGIN email + mot de passe (obligatoire) — AUCUNE session automatique.
//
// Flux obligatoire de l'app :
//   App start -> écran LOGIN (AUCUN processus métier encore)
//   Login OK -> token/user sauvegardés -> redirection accueil
//             -> processus métier démarrent APRÈS affichage de l'accueil (App.vue)
//   Login ERREUR -> "Identifiants incorrects" -> rester sur login
//   401 (session expirée) -> clearSession -> /login
//   Aucune session locale au boot -> écran login (PAS de restore automatique)
//
// Source UNIQUE de vérité : ce store, hydraté à la session persistée.
// Toutes les écritures de session doivent avoir un token réel.
// JAMAIS de bootstrap admin automatique après un 401.

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'
import { SessionStorage } from '@/lib/sessionStorage'

export type AuthState = 'unauthenticated' | 'authenticated' | 'error'

function log(tag: string, msg: string) {
  console.log(`[${tag}] ${msg}`)
}

// ---- Source UNIQUE de vérité : ce store, hydraté au boot depuis la session
// ---- persistée (miroir localStorage synchrone + store sûr en background).
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
  const token = ref<string | null>(loadToken())
  const user = ref<any | null>(loadUser())
  const state = ref<AuthState>(
    token.value && user.value ? 'authenticated' : 'unauthenticated'
  )
  const lastError = ref<string | null>(null)

  const isAuthenticated = computed(() => state.value === 'authenticated' && !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')

  // Garde-fou interne : refuse toute écriture de session sans token réel.
  // (anti "token missing" : une session sans token n'est JAMAIS persistée)
  async function setSession(data: any) {
    if (!data?.token || !data?.user) {
      log('AUTH', 'setSession refusé: token/user manquant dans la réponse')
      throw new Error('Réponse de connexion invalide')
    }
    token.value = data.token
    user.value = data.user
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

  /// RETRY 503 : "connexion impossible" est souvent une connexion Supabase morte
  /// (fermée par le serveur après idle) — le backend la restaure en ~1-2s. Un
  /// seul retry après un court délai évite les faux échecs au premier essai,
  /// sans masquer une vraie panne réseau (le 2e 503 est remonté tel quel).
  const RETRY_503_DELAY_MS = 1200
  async function withRetry503<T>(fn: () => Promise<T>): Promise<T> {
    try {
      return await fn()
    } catch (e: any) {
      if (e?.status !== 503) throw e
      log('AUTH', `503 reçu -> retry unique dans ${RETRY_503_DELAY_MS}ms`)
      await new Promise(r => setTimeout(r, RETRY_503_DELAY_MS))
      return fn()
    }
  }

  /// CONNEXION (obligatoire) : email + mot de passe -> token + user -> affichage accueil.
  // Le flux est : email+mdp -> backend Rust/Tauri -> Supabase -> verification email+mot de passe.
  // Si correct -> session immediate + sauvegarde token+user -> retour accueil.
  // Si incorrect -> "Identifiants incorrects" immediatement.
  // AUCUN check SQLite, aucune synchronisation,aucun chargement données AVANT cet appel.
  async function login(email: string, password: string) {
    lastError.value = null
    const data = await withRetry503(() => api.post('/auth/login', { email, password }))
    await setSession(data) // lève si token/user absent -> pas de session fantôme
    state.value = 'authenticated'
    log('AUTH', `login OK via ${data.source || 'supabase'} (role=${data.user?.role})`)
    return data.user
  }

  /// BYPASS (secours) : session admin locale créée par auth_bootstrap_admin.
  /// Mêmes garde-fous que setSession (jamais de session sans token/user).
  async function setSessionFromBootstrap(data: any) {
    await setSession(data)
    state.value = 'authenticated'
    log('AUTH', `session de secours active (source=${data?.source || 'kiosk-admin'}, role=${data?.user?.role})`)
  }

  async function logout() {
    try { await api.post('/auth/logout') } catch {}
    // RÈGLE ABSOLUE : suspend les processus métier côté Rust (sync auto,
    // watcher, notifications) — avant même d'effacer la session locale.
    try { await api.post('/auth/business-suspend') } catch {}
    await clearSession()
    log('AUTH', 'logout')
  }

  async function fetchMe() {
    if (!token.value) {
      // Aucun appel API protégé sans token (anti "token missing").
      throw Object.assign(new Error('Aucune session'), { status: 401 })
    }
    const data = await api.get('/auth/me')
    if (data?.user) {
      user.value = data.user
      await SessionStorage.saveSession({ token: token.value, refresh_token: null, user: data.user })
    }
  }

  /**
   * NOUVEAU : au démarrage, PAS de restauration de session locale automatique.
   * L'écran de connexion s'affiche toujours en premier. L'utilisateur doit
   fournir ses identifiants. Pas de bootstrap, pas de check SQLite, pas
   de synchronisation, pas de réseau requis ici.
   */
  function restoreSession(): Promise<boolean> {
    // Aucune session locale automatique au boot : on affiche toujours l'écran login.
    // La session peut être restaurée côté serveur si besoin, mais l'UI montre
    // toujours le formulaire de connexion en premier.
    state.value = 'unauthenticated'
    log('AUTH', 'aucune session locale au boot -> écran login')
    return Promise.resolve(false)
  }

  /// Renouvellement EXPLICITE du token via le refresh token (appelé volontairement).
  async function refreshSession(): Promise<boolean> {
    try {
      const rt = SessionStorage.getSessionSync().refresh_token
      if (!rt || !token.value) return false
      const data = await withRetry503(() => api.post('/auth/refresh', { refresh_token: rt }))
      if (!data?.token) return false
      await setSession({ ...data, user: user.value })
      state.value = 'authenticated'
      log('AUTH', 'token renouvelé')
      return true
    } catch (e) {
      log('AUTH', 'renouvellement échoué')
      return false
    }
  }

  // RÈGLE ABSOLUE : lève le verrou métier côté Rust (sync auto, watcher de
  // sessions, notifications). Appelé par App.vue APRÈS l'affichage de l'écran d'accueil.
  async function businessReady() {
    try { await api.post('/auth/business-ready') } catch {}
  }
  // Règle absolue : abaisse le verrou métier côté Rust (déconnexion/expiration).
  async function businessSuspend() {
    try { await api.post('/auth/business-suspend') } catch {}
  }

  if (typeof window !== 'undefined') {
    // 401 transport (tauriApi.ts) : session expirée/invalide -> retour LOGIN.
    // PAS de bootstrap automatique : c'est l'écran de connexion qui prend le relais.
    window.addEventListener('gl:unauthorized', () => {
      log('AUTH', '401 transport reçu -> session invalidée (retour login)')
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
