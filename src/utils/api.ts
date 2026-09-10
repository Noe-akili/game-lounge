// @ts-nocheck
import { initClientDb, seedClientDb } from '@/lib/clientDb'
import { handleRequest, isTauri } from '@/lib/transport'

let dbReady: Promise<void> | null = null

async function ensureDb() {
  // En mode Tauri (Android/backend Rust), on n'initialise pas la base
  // navigateur : toutes les requêtes passent par les commandes Rust.
  if (isTauri()) return
  if (!dbReady) {
    dbReady = (async () => {
      await initClientDb()
      await seedClientDb()
    })()
  }
  return dbReady
}

async function request(path: string, options: any = {}) {
  await ensureDb()

  const token = localStorage.getItem('gl_token')
  const body = options.body ? (typeof options.body === 'string' ? JSON.parse(options.body) : options.body) : undefined

  const result = await handleRequest(path, options.method || 'GET', body, token)

  if (!result.status || result.status >= 400) {
    const err = new Error(result.body?.message || `Erreur ${result.status}`)
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