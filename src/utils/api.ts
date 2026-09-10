// @ts-nocheck
import { initClientDb, seedClientDb } from '@/lib/clientDb'
import { handleRequest, isTauri } from '@/lib/transport'
import { isHttpServerMode } from '@/lib/httpApi'

let dbReady: Promise<void> | null = null

async function ensureDb() {
  // En mode Tauri (Android/backend Rust) ou HTTP, pas de DB navigateur.
  // On vérifie avec le nouveau isTauri() robuste (qui gère le race au boot APK)
  const { waitForTauri } = await import('@/lib/transport')
  const isTauriMode = await waitForTauri(500).catch(() => false)
  if (isTauriMode || isTauri() || isHttpServerMode()) return
  if (!dbReady) {
    dbReady = (async () => {
      try {
        await initClientDb()
        await seedClientDb()
      } catch (e) {
        console.error('DB init fail (non-fatal)', e)
      }
    })()
  }
  return dbReady
}

async function request(path: string, options: any = {}) {
  // Sur Android cold start, ensureDb peut prendre 200ms ; on ne bloque pas l'UI, on laisse handleRequest gérer le fallback
  try { await ensureDb() } catch (e) { console.warn('[api] ensureDb non-fatal', e) }

  let token: string | null = null
  try { token = localStorage.getItem('gl_token') } catch { token = null }
  let body: any = undefined
  try {
    body = options.body ? (typeof options.body === 'string' ? JSON.parse(options.body) : options.body) : undefined
  } catch {
    body = options.body
  }

  let result: { status: number; body: any }
  try {
    result = await handleRequest(path, options.method || 'GET', body, token)
  } catch (e: any) {
    console.error('[api] handleRequest threw', e)
    // Sur Android WebView, une exception non catchée = crash -> on wrappe
    const err: any = new Error(e?.message || 'Erreur réseau')
    err.status = 500
    err.data = { message: e?.message }
    throw err
  }

  if (!result || typeof result.status !== 'number') {
    const err: any = new Error('Réponse invalide du serveur')
    err.status = 500
    throw err
  }
  if (result.status >= 400) {
    const err: any = new Error(result.body?.message || `Erreur ${result.status}`)
    err.status = result.status
    err.data = result.body
    throw err
  }
  return result.body
}

export function setApiUrl(_url: string) {}

export const api = {
  get: (path: string) => request(path, { method: 'GET' }),
  post: (path: string, body?: any) => request(path, { method: 'POST', body }),
  put: (path: string, body?: any) => request(path, { method: 'PUT', body }),
  delete: (path: string) => request(path, { method: 'DELETE' }),
}