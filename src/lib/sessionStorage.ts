// @ts-nocheck
// Abstraction SessionStorage (mission §6) : TOUTE la persistance de session
// passe par ici — plus aucun localStorage.setItem dispersé dans auth.ts.
//
// Backend de stockage :
//  - Tauri Android : tauri-plugin-store (fichier app_data, isolé par app,
//    pas accessible aux autres apps) via src/lib/secureStore.ts.
//  - Fallback : localStorage (web/dev).
// Le backend peut être remplacé (Keystore natif, etc.) sans toucher auth.ts.

import { secureGet, secureSet, secureRemove } from './secureStore'

const K_TOKEN = 'gl_token'
const K_REFRESH = 'gl_refresh_token'
const K_USER = 'gl_user'

export type StoredSession = {
  token: string | null
  refresh_token: string | null
  user: any | null
}

function loadUserFallback(): any | null {
  try {
    const raw = localStorage.getItem(K_USER)
    if (!raw || raw === 'null') return null
    return JSON.parse(raw)
  } catch { return null }
}

export const SessionStorage = {
  /** Lit la session persistée (synchrone au démarrage pour un rendu immédiat). */
  getSessionSync(): StoredSession {
    return {
      token: (() => { try { return localStorage.getItem(K_TOKEN) } catch { return null } })(),
      refresh_token: (() => { try { return localStorage.getItem(K_REFRESH) } catch { return null } })(),
      user: loadUserFallback(),
    }
  },

  /** Lit la session depuis le backend de stockage sûr (async). */
  async getSession(): Promise<StoredSession> {
    const [token, refresh_token, rawUser] = await Promise.all([
      secureGet(K_TOKEN),
      secureGet(K_REFRESH),
      secureGet(K_USER),
    ])
    let user: any = null
    if (rawUser) { try { user = JSON.parse(rawUser) } catch { user = null } }
    // Filet : si le store sûr est vide mais localStorage a une session (ancienne
    // install), on la considère quand même — migrateSession la déplacera.
    const fb = this.getSessionSync()
    return {
      token: token || fb.token,
      refresh_token: refresh_token || fb.refresh_token,
      user: user || fb.user,
    }
  },

  /** Sauvegarde atomique de la session (store sûr + miroir localStorage). */
  async saveSession(s: StoredSession): Promise<void> {
    if (s.token) await secureSet(K_TOKEN, s.token)
    if (s.refresh_token) await secureSet(K_REFRESH, s.refresh_token)
    if (s.user) await secureSet(K_USER, JSON.stringify(s.user))
    // Miroir localStorage : Lecture SYNCHRONE au cold start (avant que le
    // plugin store async soit prêt) — c'est un cache, pas la source de vérité.
    try {
      if (s.token) localStorage.setItem(K_TOKEN, s.token)
      if (s.refresh_token) localStorage.setItem(K_REFRESH, s.refresh_token)
      if (s.user) localStorage.setItem(K_USER, JSON.stringify(s.user))
    } catch {}
  },

  /** Efface toute trace de session des deux backends. */
  async clearSession(): Promise<void> {
    await Promise.all([
      secureRemove(K_TOKEN), secureRemove(K_REFRESH), secureRemove(K_USER),
    ])
    try {
      localStorage.removeItem(K_TOKEN)
      localStorage.removeItem(K_REFRESH)
      localStorage.removeItem(K_USER)
    } catch {}
  },
}
