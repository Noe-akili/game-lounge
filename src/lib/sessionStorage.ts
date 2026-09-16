// Abstraction SessionStorage : Session en MÉMOIRE UNIQUEMENT.
// À la fermeture de l'application, la session est automatiquement perdue.
// Nettoie également les anciens tokens résiduels du localStorage.

export type StoredSession = {
  token: string | null
  refresh_token: string | null
  user: any | null
}

// État stocké en mémoire vive pour la durée de vie du processus
let memorySession: StoredSession = {
  token: null,
  refresh_token: null,
  user: null,
}

// Nettoyage immédiat de tout ancien token persisté dans localStorage
function purgeLegacyStorage() {
  try {
    if (typeof window !== 'undefined' && window.localStorage) {
      localStorage.removeItem('gl_token')
      localStorage.removeItem('gl_refresh_token')
      localStorage.removeItem('gl_user')
    }
  } catch {}
}

purgeLegacyStorage()

export const SessionStorage = {
  /** Lit la session depuis la mémoire. */
  getSessionSync(): StoredSession {
    return { ...memorySession }
  },

  /** Lit la session (async, conforme à l'interface). */
  async getSession(): Promise<StoredSession> {
    return { ...memorySession }
  },

  /** Sauvegarde en mémoire uniquement (aucune écriture disque ni localStorage). */
  async saveSession(s: StoredSession): Promise<void> {
    memorySession = {
      token: s.token,
      refresh_token: s.refresh_token || null,
      user: s.user ? JSON.parse(JSON.stringify(s.user)) : null,
    }
    purgeLegacyStorage()
  },

  /** Efface la session mémoire et purge tout résidu. */
  async clearSession(): Promise<void> {
    memorySession = {
      token: null,
      refresh_token: null,
      user: null,
    }
    purgeLegacyStorage()
  },
}
