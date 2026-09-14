// Auth mono-utilisateur admin (mission §4-§6, §15) : PAS de login.
//
// Trois notions BIEN SÉPARÉES :
//   1. Device Identity  : device_id persistant SQLite côté Rust (device_info)
//   2. AuthSession      : token JWT local + user, géré ICI via SessionStorage
//   3. SyncState        : store sync.ts (aucune dépendance depuis ce store)
//
// Machine à états AuthState (pas de booléens incohérents) :
//   unauthenticated -> initializing -> authenticated | error
//
// bootstrap (première initialisation / récupération contrôlée) :
//   - retry avec backoff exponentiel, JAMAIS de boucle bootstrap->erreur->bootstrap
//   - un 401 ne régénère PAS une session admin (mission §5) : il classe
//     AUTH_EXPIRED / AUTH_INVALID et s'arrête — le token est renouvelé via
//     auth_refresh côté Rust, pas en recréant une identité.
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'
import { SessionStorage } from '@/lib/sessionStorage'

export type AuthState = 'unauthenticated' | 'initializing' | 'authenticated' | 'error'

// Classification des erreurs (mission §15)
export type AuthErrorKind =
  | 'NETWORK_ERROR' | 'AUTH_EXPIRED' | 'AUTH_INVALID' | 'SERVER_ERROR' | 'LOCAL_DB_ERROR'

const MAX_BOOTSTRAP_ATTEMPTS = 3
const BOOTSTRAP_BASE_DELAY_MS = 800
const BOOTSTRAP_MAX_DELAY_MS = 8000

function classifyError(e: any): AuthErrorKind {
  const status = e?.status
  if (status === 401) return 'AUTH_INVALID'
  if (status === 403) return 'AUTH_EXPIRED'
  if (status === 503 || status === 504 || e?.code === 'ECONNREFUSED') return 'NETWORK_ERROR'
  if (status && status >= 500) return 'SERVER_ERROR'
  if (status && status >= 400) return 'AUTH_INVALID'
  // Pas de statut = transport indisponible (Tauri non prêt, IPC timeout...)
  return 'NETWORK_ERROR'
}

function log(tag: string, msg: string) {
  console.log(`[${tag}] ${msg}`)
}

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
  // Démarrage SYNCHRONE depuis le miroir localStorage (cold start rapide, §16) :
  // la session sûre (store Tauri) sera re-vérifiée en background.
  const token = ref(loadToken())
  const user = ref(loadUser())
  const state = ref<AuthState>(token.value && user.value ? 'initializing' : 'unauthenticated')
  const lastError = ref<{ kind: AuthErrorKind; message: string } | null>(null)

  // Mutex logique : une SEULE restauration à la fois (mission §4).
  let restorePromise: Promise<boolean> | null = null
  // Compteur de boucle anti-bootstrap-storm (mission §15).
  let bootstrapFailures = 0

  const isAuthenticated = computed(() => state.value === 'authenticated' && !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')

  function setState(s: AuthState) {
    state.value = s
  }

  async function setSession(data: any) {
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
    setState('unauthenticated')
    await SessionStorage.clearSession()
  }

  /// PREMIÈRE INITIALISATION uniquement (mission §5) : crée la session admin
  /// implicite. JAMAIS appelé automatiquement après un 401.
  async function bootstrapAdmin(): Promise<boolean> {
    if (state.value === 'initializing') return false // re-entrance
    setState('initializing')
    lastError.value = null
    for (let attempt = 1; attempt <= MAX_BOOTSTRAP_ATTEMPTS; attempt++) {
      try {
        log('AUTH', `bootstrap admin (tentative ${attempt}/${MAX_BOOTSTRAP_ATTEMPTS})`)
        const data = await api.post('/auth/bootstrap')
        await setSession(data)
        bootstrapFailures = 0
        setState('authenticated')
        log('AUTH', 'session admin créée')
        return true
      } catch (e: any) {
        const kind = classifyError(e)
        lastError.value = { kind, message: e?.message || 'Erreur de session' }
        log('AUTH', `bootstrap échoué: ${kind}: ${e?.message || e}`)
        // 401/403 du bootstrap = refus serveur explicite : PAS de retry
        // (mission §5 : ne pas transformer un refus en nouvelle identité).
        if (kind === 'AUTH_INVALID' || kind === 'AUTH_EXPIRED') {
          setState('error')
          return false
        }
        // Network/serveur : backoff exponentiel puis retry.
        if (attempt < MAX_BOOTSTRAP_ATTEMPTS) {
          const delay = Math.min(BOOTSTRAP_BASE_DELAY_MS * 2 ** (attempt - 1), BOOTSTRAP_MAX_DELAY_MS)
          await new Promise((r) => setTimeout(r, delay))
        }
      }
    }
    bootstrapFailures++
    setState('error')
    return false
  }

  function classifyAndRemember(e: any) {
    const kind = classifyError(e)
    lastError.value = { kind, message: e?.message || 'Erreur de session' }
    log('AUTH', `fetchMe échoué: ${kind}: ${e?.message || e}`)
    return kind
  }

  async function fetchMe() {
    const data = await api.get('/auth/me')
    user.value = data.user
    await SessionStorage.saveSession({ token: token.value, refresh_token: null, user: data.user })
  }

  /**
   * Restaure la session locale. IDEMPOTENTE + protégée contre les appels
   * concurrents (Promise partagée, mission §4). N'attend JAMAIS le réseau
   * pour déclarer l'app utilisable : si un token+user locaux existent,
   * l'état passe 'authenticated' immédiatement et la vérification serveur
   * (/auth/me) continue en arrière-plan.
   */
  function restoreSession(): Promise<boolean> {
    if (restorePromise) return restorePromise // appel concurrent -> même Promise
    restorePromise = (async () => {
      try {
        if (token.value && user.value) {
          // Session locale présente : ouverture IMMÉDIATE (offline-first, §2/§7).
          setState('authenticated')
          log('AUTH', 'session locale trouvée')
          // Vérification serveur en background — ne bloque pas le rendu.
          fetchMe().catch((e) => {
            const kind = classifyAndRemember(e)
            if (kind === 'AUTH_INVALID') {
              // Token réellement invalide : on s'arrête NET (mission §5).
              // PAS de bootstrap automatique — l'état 'error' est affiché,
              // une action utilisateur (DeveloperView) peut re-bootstrapper.
              log('AUTH', 'token invalide -> état error (pas de re-bootstrap auto)')
              clearSession()
              setState('error')
            }
            // AUTH_EXPIRED : tente UN renouvellement explicite via refresh token.
            if (kind === 'AUTH_EXPIRED') {
              log('AUTH', 'token expiré -> tentative de renouvellement')
              refreshSession().catch(() => {})
            }
          })
          return true
        }
        // Pas de session locale = PREMIER LANCEMENT (ou stockage vidé) :
        // c'est le SEUL cas où bootstrap est légitime (mission §5).
        log('AUTH', 'aucune session locale -> première initialisation')
        return await bootstrapAdmin()
      } finally {
        restorePromise = null // prochain appel pourra re-restaurer
      }
    })()
    return restorePromise
  }

  /// Renouvellement EXPLICITE du token via le refresh token (mission §5).
  async function refreshSession(): Promise<boolean> {
    try {
      const rt = SessionStorage.getSessionSync().refresh_token
      if (!rt) {
        log('AUTH', 'pas de refresh token disponible')
        return false
      }
      const data = await api.post('/auth/refresh', { refresh_token: rt })
      await setSession({ ...data, user: user.value })
      setState('authenticated')
      log('AUTH', 'session renouvelée')
      return true
    } catch (e) {
      log('AUTH', 'renouvellement échoué')
      return false
    }
  }

  if (typeof window !== 'undefined') {
    // 401 transport (tauriApi.ts) : classe l'erreur, NE re-bootstrap PAS.
    window.addEventListener('gl:unauthorized', () => {
      log('AUTH', '401 transport reçu -> classification (pas de re-bootstrap auto)')
      lastError.value = { kind: 'AUTH_EXPIRED', message: 'Session expirée' }
      setState('unauthenticated')
      token.value = null
    })
  }

  return {
    user, token, state, lastError,
    isAuthenticated, isAdmin,
    bootstrapAdmin, fetchMe, restoreSession, refreshSession, clearSession,
  }
})
