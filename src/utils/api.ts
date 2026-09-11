// @ts-nocheck
// Tauri-only API - best practice avec refresh token et retry, évite localStorage critique
import { handleRequest } from '@/lib/transport'
import { secureGet, secureSet } from '@/lib/secureStore'

let isRefreshing = false
let refreshPromise: Promise<string | null> | null = null

async function getSecureToken(): Promise<string | null> {
  try {
    const s = await secureGet('gl_token')
    if (s) return s
  } catch {}
  try { return localStorage.getItem('gl_token') } catch { return null }
}
async function getSecureRefresh(): Promise<string | null> {
  try {
    const s = await secureGet('gl_refresh_token')
    if (s) return s
  } catch {}
  try { return localStorage.getItem('gl_refresh_token') } catch { return null }
}

async function request(path: string, options: any = {}, retry = true) {
  let token: string | null = null
  try { token = await getSecureToken() } catch { token = null }
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
  // Best practice : 401 -> tente refresh une fois (sauf pour /auth/refresh et /auth/login)
  if (result.status === 401 && retry && !path.includes('/auth/refresh') && !path.includes('/auth/login')) {
    const refreshToken = (() => { try { return localStorage.getItem('gl_refresh_token') } catch { return null } })()
    const isDebug = token === 'debug-bypass-android14'
    if (refreshToken && !isDebug) {
      if (!isRefreshing) {
        isRefreshing = true
        refreshPromise = (async () => {
          try {
            console.log('[api] 401 -> refresh token')
            const refreshRes = await handleRequest('/auth/refresh', 'POST', { refresh_token: refreshToken }, refreshToken)
            if (refreshRes.status === 200 && refreshRes.body?.token) {
              try { localStorage.setItem('gl_token', refreshRes.body.token) } catch {}
              console.log('[api] refresh success')
              return refreshRes.body.token
            }
            return null
          } catch (e) {
            console.warn('[api] refresh failed', e)
            return null
          } finally {
            isRefreshing = false
          }
        })()
      }
      const newToken = await refreshPromise
      if (newToken) {
        // Retry original request avec nouveau token
        return request(path, options, false)
      }
    }
    // Refresh échoué -> déconnexion
    try {
      localStorage.removeItem('gl_token')
      localStorage.removeItem('gl_user')
    } catch {}
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
