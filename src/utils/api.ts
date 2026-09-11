// @ts-nocheck
// Tauri-only API - plus de fallback navigateur/Node
import { handleRequest } from '@/lib/transport'

async function request(path: string, options: any = {}) {
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
