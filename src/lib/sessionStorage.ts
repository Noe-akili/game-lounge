// Abstraction SessionStorage : Session persistante dans localStorage.
// La session reste enregistrée dans le téléphone après fermeture/réouverture de l'application.
// Elle n'est supprimée que lors d'une déconnexion explicite (Logout) ou d'un 401.

export type StoredSession = {
  token: string | null
  refresh_token: string | null
  user: any | null
}

const K_TOKEN = 'gl_token'
const K_REFRESH = 'gl_refresh_token'
const K_USER = 'gl_user'

export const SessionStorage = {
  /** Lit la session persistée (localStorage prioritaire, fallback sessionStorage). */
  getSessionSync(): StoredSession {
    try {
      const storage = typeof localStorage !== 'undefined' ? localStorage : (typeof sessionStorage !== 'undefined' ? sessionStorage : null)
      if (!storage) return { token: null, refresh_token: null, user: null }

      const token = storage.getItem(K_TOKEN)
      const refresh_token = storage.getItem(K_REFRESH)
      const rawUser = storage.getItem(K_USER)
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

  /** Sauvegarde durablement la session dans le stockage du téléphone. */
  async saveSession(s: StoredSession): Promise<void> {
    try {
      if (typeof localStorage !== 'undefined') {
        if (s.token) localStorage.setItem(K_TOKEN, s.token)
        if (s.refresh_token) localStorage.setItem(K_REFRESH, s.refresh_token)
        if (s.user) localStorage.setItem(K_USER, JSON.stringify(s.user))
      }
      if (typeof sessionStorage !== 'undefined') {
        if (s.token) sessionStorage.setItem(K_TOKEN, s.token)
        if (s.refresh_token) sessionStorage.setItem(K_REFRESH, s.refresh_token)
        if (s.user) sessionStorage.setItem(K_USER, JSON.stringify(s.user))
      }
    } catch (e) {
      console.warn('[SessionStorage] Erreur sauvegarde session:', e)
    }
  },

  /** Efface complètement la session sur demande (Logout). */
  async clearSession(): Promise<void> {
    try {
      if (typeof localStorage !== 'undefined') {
        localStorage.removeItem(K_TOKEN)
        localStorage.removeItem(K_REFRESH)
        localStorage.removeItem(K_USER)
      }
      if (typeof sessionStorage !== 'undefined') {
        sessionStorage.removeItem(K_TOKEN)
        sessionStorage.removeItem(K_REFRESH)
        sessionStorage.removeItem(K_USER)
      }
    } catch (e) {
      console.warn('[SessionStorage] Erreur nettoyage session:', e)
    }
  },

  /** Récupère le token actif persisté. */
  getToken(): string | null {
    try {
      if (typeof localStorage !== 'undefined') {
        const t = localStorage.getItem(K_TOKEN)
        if (t) return t
      }
      if (typeof sessionStorage !== 'undefined') {
        return sessionStorage.getItem(K_TOKEN)
      }
      return null
    } catch {
      return null
    }
  }
}
