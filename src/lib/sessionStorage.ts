// Abstraction SessionStorage : Session en SESSIONSTORAGE (volatilité garantie à la fermeture de l'app).
// À la fermeture ou au kill de l'application, sessionStorage est automatiquement vidé par le système.
// Aucune donnée n'est conservée dans localStorage (disque permanent).

export type StoredSession = {
  token: string | null
  refresh_token: string | null
  user: any | null
}

const K_TOKEN = 'gl_token'
const K_REFRESH = 'gl_refresh_token'
const K_USER = 'gl_user'

// Purge systématique du localStorage (disque permanent) pour éviter toute persistance
function purgePersistentStorage() {
  try {
    if (typeof localStorage !== 'undefined') {
      localStorage.removeItem(K_TOKEN)
      localStorage.removeItem(K_REFRESH)
      localStorage.removeItem(K_USER)
    }
  } catch {}
}

purgePersistentStorage()

export const SessionStorage = {
  /** Lit la session depuis sessionStorage (actif tant que l'app tourne). */
  getSessionSync(): StoredSession {
    try {
      const token = typeof sessionStorage !== 'undefined' ? sessionStorage.getItem(K_TOKEN) : null
      const refresh_token = typeof sessionStorage !== 'undefined' ? sessionStorage.getItem(K_REFRESH) : null
      const rawUser = typeof sessionStorage !== 'undefined' ? sessionStorage.getItem(K_USER) : null
      let user: any = null
      if (rawUser) {
        try { user = JSON.parse(rawUser) } catch {}
      }
      return { token, refresh_token, user }
    } catch {
      return { token: null, refresh_token: null, user: null }
    }
  },

  /** Lit la session (async). */
  async getSession(): Promise<StoredSession> {
    return this.getSessionSync()
  },

  /** Sauvegarde dans sessionStorage (en mémoire pendant l'exécution, détruit à la fermeture). */
  async saveSession(s: StoredSession): Promise<void> {
    try {
      if (typeof sessionStorage !== 'undefined') {
        if (s.token) sessionStorage.setItem(K_TOKEN, s.token)
        if (s.refresh_token) sessionStorage.setItem(K_REFRESH, s.refresh_token)
        if (s.user) sessionStorage.setItem(K_USER, JSON.stringify(s.user))
      }
    } catch {}
    // Garantir qu'aucun token ne persiste dans localStorage sur disque
    purgePersistentStorage()
  },

  /** Efface complètement la session active. */
  async clearSession(): Promise<void> {
    try {
      if (typeof sessionStorage !== 'undefined') {
        sessionStorage.removeItem(K_TOKEN)
        sessionStorage.removeItem(K_REFRESH)
        sessionStorage.removeItem(K_USER)
      }
    } catch {}
    purgePersistentStorage()
  },

  /** Récupère le token actif. */
  getToken(): string | null {
    try {
      return typeof sessionStorage !== 'undefined' ? sessionStorage.getItem(K_TOKEN) : null
    } catch {
      return null
    }
  }
}
