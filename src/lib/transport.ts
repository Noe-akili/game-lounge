// @ts-nocheck
// Dispatcher de requêtes API : détecte l'hôte Tauri (application Android légère,
// backend Rust) et route vers les commandes Tauri. Sinon, repli sur la base
// locale sql.js (mode navigateur / démo). Un mode HTTP (backend Rust lancé en
// serveur pour le débogage navigateur) est disponible via VITE_GL_SERVER_URL.

import { handleClientRequest } from './clientApi'
import { handleTauriRequest } from './tauriApi'
import { isHttpServerMode, handleHttpServerRequest } from './httpApi'

export function isTauri() {
  if (typeof window === 'undefined') return false
  const w: any = window as any
  // Détection robuste pour APK Tauri Android :
  // - avecGlobalTauri=true => window.__TAURI__.core.invoke existe après injection
  // - sur Android, l'injection peut être retardée de quelques ms au boot
  // - on vérifie aussi __TAURI_INTERNALS__ et __TAURI_IPC__
  return !!(
    w.__TAURI__?.core?.invoke ||
    w.__TAURI_INTERNALS__?.invoke ||
    w.__TAURI_IPC__ ||
    w.isTauri ||
    navigator.userAgent.includes('Tauri')
  )
}
// Attend que Tauri soit injecté (utile au tout premier chargement de l'APK)
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
  console.log('[TRANSPORT_START]', method, path)
  console.log('[TAURI_DETECTED] isTauri=', isTauri(), 'api path', path)
  // Sur Android, isHttpServerMode est toujours false, mais on garde la branche pour debug
  try {
    if (isHttpServerMode()) {
      console.log('[TRANSPORT] httpServer mode')
      return await handleHttpServerRequest(path, method, body, authToken)
    }
  } catch (e) {
    console.warn('[transport] httpServer failed, fallback', e)
  }
  // En APK, on tente Tauri avec retry (injection WebView peut être lente 0-1500ms)
  if (isTauri()) {
    try {
      return await handleTauriRequest(path, method, body, authToken)
    } catch (e) {
      console.warn('[transport] Tauri invoke failed, retry with wait', e)
      // Retry après 300ms si race au boot
      try {
        const ok = await waitForTauri(800)
        if (ok) return await handleTauriRequest(path, method, body, authToken)
      } catch {}
      console.warn('[transport] Tauri retry failed, fallback to client', e)
    }
  } else {
    // Tentative opportuniste : si window.__TAURI__ est en cours d'injection, on essaie quand même
    const maybeTauri = typeof window !== 'undefined' && !!(window as any).__TAURI__
    if (maybeTauri) {
      try { return await handleTauriRequest(path, method, body, authToken) } catch (e) {
        console.warn('[transport] opportunistic Tauri failed', e)
      }
    }
    // Sur Android sans Tauri (dev), on attend un peu puis on retente (évite white screen au cold start)
    try {
      const ok = await waitForTauri(600)
      if (ok) {
        try { return await handleTauriRequest(path, method, body, authToken) } catch {}
      }
    } catch {}
  }
  // Fallback clientApi (IndexedDB/sql.js) - jamais de crash, retourne toujours un status
  try {
    return await handleClientRequest(path, method, body, authToken)
  } catch (e: any) {
    console.error('[transport] client fallback crashed', e)
    return { status: 500, body: { message: e?.message || 'Erreur interne (fallback)' } }
  }
}