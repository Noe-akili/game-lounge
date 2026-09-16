// @ts-nocheck
import { handleRequest } from '@/lib/transport'
import { SessionStorage } from '@/lib/sessionStorage'

function getToken(): string | null {
  return SessionStorage.getToken()
}

async function request(path: string, options: any = {}) {
  const token = getToken()
  const body = options.body ? (typeof options.body === 'string' ? JSON.parse(options.body) : options.body) : undefined
  const result = await handleRequest(path, options.method || 'GET', body, token)
  if (result.status >= 400) {
    const err: any = new Error(result.body?.message || )
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
