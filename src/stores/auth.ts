// @ts-nocheck
// Auth : LOGIN email + mot de passe (obligatoire) — AUCUNE session automatique.
//
// Flux obligatoire de l'app :
//   App start -> session locale valide -> Dashboard
//             -> aucune session        -> /login
//   Login OK  -> token/user sauvegardés (source UNIQUE de vérité = ce store)
//             -> SQLite chargée (les vues lisent l'API = SQLite locale)
//             -> sync Supabase en arrière-plan (boucle auto_sync Rust)
//
// 401 (session expirée/invalide) -> clearSession + état 'unauthenticated'
// -> le routeur renvoie à /login. JAMAIS de bootstrap admin automatique après
// un 401, JAMAIS de boucle login -> token missing -> login (token absent =
// on affiche le login, point ; aucun appel API protégé n'est tenté sans token).
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'
import { SessionStorage } from '@/lib/sessionStorage'

export type AuthState = 'unauthenticated' | 'restoring' | 'authenticated' | 'error'

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
    token.value && user.value ? 'restoring' : 'unauthenticated'
  )
  const lastError = ref<string | null>(null)
  const restoreAttempted = ref(false)

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

  /// CONNEXION (obligatoire) : email + mot de passe -> token + user.
  async function login(email: string, password: string) {
    lastError.value = null
    const data = await api.post('/auth/login', { email, password })
    await setSession(data) // lève si token/user absent -> pas de session fantôme
    state.value = 'authenticated'
    restoreAttempted.value = false
    log('AUTH', `login OK via ${data.source || 'local'} (role=${data.user?.role})`)
    return data.user
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
   * Restaure la session locale au démarrage. IDEMPOTENTE + protégée contre
   * les appels concurrents (Promise partagée). Côté réseau : la vérification
   * serveur (/auth/me) tourne en ARRIÈRE-PLAN — le guard routeur n'attend
   * jamais le réseau pour décider Login vs Dashboard.
   */
  function restoreSession(): Promise<boolean> {
    if (restoreAttempted.value) {
      return Promise.resolve(isAuthenticated.value)
    }
    restoreAttempted.value = true
    if (token.value && user.value) {
      // Session locale présente : ouverture IMMÉDIATE (offline-first).
      state.value = 'authenticated'
      log('AUTH', 'session locale trouvée')
      // Vérification serveur en arrière-plan — ne bloque pas le rendu.
      fetchMe().catch((e) => {
        if (e?.status === 401) {
          // Session expirée/invalide : retour au LOGIN (jamais de re-bootstrap,
          // jamais de boucle : clearSession une seule fois).
          log('AUTH', 'session expirée (401) -> retour login')
          clearSession()
        } else {
          log('AUTH', `vérification session en échec (hors ligne ?): ${e?.message || e}`)
        }
      })
      return Promise.resolve(true)
    }
    // Aucune session locale : PAS de bootstrap. L'écran de connexion s'affiche.
    state.value = 'unauthenticated'
    log('AUTH', 'aucune session locale -> écran login')
    return Promise.resolve(false)
  }

  /// Renouvellement EXPLICITE du token via le refresh token (appelé volontairement).
  async function refreshSession(): Promise<boolean> {
    try {
      const rt = SessionStorage.getSessionSync().refresh_token
      if (!rt || !token.value) return false
      const data = await api.post('/auth/refresh', { refresh_token: rt })
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
  // sessions, notifications). Appelé par App.vue APRÈS l'affichage de l'accueil.
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
    login, logout, fetchMe, restoreSession, refreshSession, clearSession,
    businessReady, businessSuspend,
  }
})
