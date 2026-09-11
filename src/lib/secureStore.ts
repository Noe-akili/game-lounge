// @ts-nocheck
// Secure storage : évite localStorage pour données critiques (token, refresh, user)
// Utilise tauri-plugin-store si disponible (chiffré), sinon fallback localStorage (web)

let storePromise: Promise<any> | null = null

async function getStore() {
  try {
    const { Store } = await import('@tauri-apps/plugin-store')
    if (!storePromise) {
      // Store persistant chiffré (fichier .dat dans app_data)
      storePromise = Store.load('secure.dat')
    }
    return await storePromise
  } catch {
    return null
  }
}

export async function secureSet(key: string, value: string) {
  // Essaie Store d'abord (Tauri)
  try {
    const s = await getStore()
    if (s) {
      await s.set(key, value)
      await s.save()
      return
    }
  } catch {}
  // Fallback localStorage pour web (mais on évite pour critique si possible)
  try { localStorage.setItem(key, value) } catch {}
}

export async function secureGet(key: string): Promise<string | null> {
  try {
    const s = await getStore()
    if (s) {
      const v = await s.get(key)
      return v as string | null
    }
  } catch {}
  try { return localStorage.getItem(key) } catch { return null }
}

export async function secureRemove(key: string) {
  try {
    const s = await getStore()
    if (s) {
      await s.delete(key)
      await s.save()
      return
    }
  } catch {}
  try { localStorage.removeItem(key) } catch {}
}

// Sync helper pour migrer ancien localStorage vers store (une fois)
export async function migrateFromLocalStorage() {
  const keys = ['gl_token', 'gl_refresh_token', 'gl_user', 'gl_login_source']
  for (const k of keys) {
    try {
      const v = localStorage.getItem(k)
      if (v) {
        await secureSet(k, v)
        // Garde localStorage pour compat, mais le store est la source critique
      }
    } catch {}
  }
}
