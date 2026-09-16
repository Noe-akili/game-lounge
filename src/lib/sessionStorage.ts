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

// Pont Tauri direct (sans import afin de rester utilisable partout).
function tauriInvoke(cmd: string, args: Record<string, unknown>): Promise<any> {
  const w: any = window as any
  const fn = w.__TAURI__?.core?.invoke || w.__TAURI_INTERNALS__?.invoke
  if (!fn) return Promise.reject(new Error('Tauri indisponible'))
  return Promise.resolve(fn.call(w.__TAURI__?.core || w.__TAURI_INTERNALS__, cmd, args))
}

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
      // Copie durable cote Rust/SQLite : le localStorage du WebView Android
      // n'est pas garanti persistant apres redemarrage de l'application.
      void this.saveDurable(s).catch(() => {})
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
      void this.clearDurable().catch(() => {})
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
  },

  /** Sauvegarde durable cote Rust (SQLite settings). */
  async saveDurable(s: StoredSession): Promise<void> {
    await tauriInvoke('auth_session_save', {
      payload: JSON.stringify({ token: s.token, refresh_token: s.refresh_token, user: s.user }),
    })
  },

  /** Charge la session depuis la sauvegarde durable Rust (SQLite). */
  async loadDurable(): Promise<StoredSession | null> {
    try {
      const res = await tauriInvoke('auth_session_load', {})
      const raw = res?.session
      if (!raw) return null
      const parsed = typeof raw === 'string' ? JSON.parse(raw) : raw
      if (parsed?.token && parsed?.user) return parsed as StoredSession
      return null
    } catch {
      return null
    }
  },

  /** Efface la sauvegarde durable cote Rust (Logout). */
  async clearDurable(): Promise<void> {
    try { await tauriInvoke('auth_session_clear', {}) } catch {}
  }
}
