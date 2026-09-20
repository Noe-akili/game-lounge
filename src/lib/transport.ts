// @ts-nocheck
// Tauri-only dispatcher : toute requête passe par Rust/SQLite (pas de fallback navigateur/Node)
// Application dédiée Android Tauri - plus de clientApi/httpApi

import { handleTauriRequest } from './tauriApi'

export function isTauri() {
  if (typeof window === 'undefined') return false
  const w: any = window as any
  return !!(
    w.__TAURI__?.core?.invoke ||
    w.__TAURI_INTERNALS__?.invoke ||
    w.__TAURI_IPC__ ||
    w.isTauri ||
    navigator.userAgent.includes('Tauri')
  )
}
export async function waitForTauri(timeoutMs = 2000): Promise<boolean> {
  if (isTauri()) return true
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    await new Promise(r => setTimeout(r, 50))
    if (isTauri()) return true
  }
  return isTauri()
}

export async function handleRequest(
  path: string,
  method: string,
  body?: any,
  authToken?: string
): Promise<{ status: number; body: any }> {
  // Tauri-only : pas de fallback HTTP/clientApi - si Tauri non détecté, on attend puis on invoque quand même
  if (isTauri()) {
    try {
      return await handleTauriRequest(path, method, body, authToken)
    } catch (e) {
      console.warn('[transport] Tauri invoke failed, retry with wait', e)
      try {
        const ok = await waitForTauri(800)
        if (ok) return await handleTauriRequest(path, method, body, authToken)
      } catch {}
      console.error('[transport] Tauri retry failed', e)
      return { status: 500, body: { message: (e as any)?.message || 'Erreur Tauri' } }
    }
  } else {
    // Race au boot : injection Tauri peut être retardée 0-1500ms sur Android 14
    const maybeTauri = typeof window !== 'undefined' && !!(window as any).__TAURI__
    if (maybeTauri) {
      try { return await handleTauriRequest(path, method, body, authToken) } catch (e) {
        console.warn('[transport] opportunistic Tauri failed', e)
      }
    }
    try {
      const ok = await waitForTauri(1500)
      if (ok) {
        try { return await handleTauriRequest(path, method, body, authToken) } catch (e) {
          console.error('[transport] waitForTauri succeeded but invoke failed', e)
          return { status: 500, body: { message: (e as any)?.message || 'Tauri non disponible' } }
        }
      }
    } catch {}
    console.error('[transport] Tauri non détecté (mode Tauri-only requis)')
    return { status: 503, body: { message: 'Tauri non disponible - application Android requise' } }
  }
}
