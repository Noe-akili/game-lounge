// @ts-nocheck
import { handleRequest } from '@/lib/transport'

async function request(path: string, options: any = {}) {
  const token = (() => { try { return localStorage.getItem('gl_token') } catch { return null } })()
  const body = options.body ? (typeof options.body === 'string' ? JSON.parse(options.body) : options.body) : undefined
  const result = await handleRequest(path, options.method || 'GET', body, token)
  if (result.status >= 400) {
    const err: any = new Error(result.body?.message || `Erreur ${result.status}`)
    err.status = result.status
    throw err
  }
  return result.body
}

export const api = {
  get: (p: string) => request(p, { method: 'GET' }),
  post: (p: string, b?: any) => request(p, { method: 'POST', body: b }),
  put: (p: string, b?: any) => request(p, { method: 'PUT', body: b }),
  delete: (p: string) => request(p, { method: 'DELETE' }),
}
