// @ts-nocheck
function detectCapacitor() {
  if (typeof window === 'undefined') return false
  const c = (window as any).Capacitor
  return !!(c && c.isNativePlatform && c.isNativePlatform())
}

const isCapacitor = detectCapacitor()

const getApiBase = () => {
  if (import.meta.env.VITE_API_URL) return import.meta.env.VITE_API_URL
  const stored = localStorage.getItem('gl_api_url')
  if (stored) return stored
  return isCapacitor ? 'http://127.0.0.1:3001/api' : '/api'
}

let API_BASE = getApiBase()

export function setApiUrl(url: string) {
  API_BASE = url
  localStorage.setItem('gl_api_url', url)
}

// Wait for Node.js server to be ready on Capacitor
let serverReadyResolve = null
const serverReady = new Promise(resolve => { serverReadyResolve = resolve })

async function waitForServer() {
  if (!isCapacitor) return

  try {
    const NodeJS = (window as any).Capacitor.Plugins.CapacitorNodeJS
    if (NodeJS?.whenReady) {
      await NodeJS.whenReady()
      console.log('[API] Node.js server ready')
    }
  } catch (e) {
    console.warn('[API] NodeJS.whenReady() failed:', e)
  }

  // Poll until server responds
  for (let i = 0; i < 30; i++) {
    try {
      const res = await fetch(`${API_BASE}/health`, {
        signal: AbortSignal.timeout(3000)
      })
      if (res.ok || res.status === 401 || res.status === 404) {
        console.log(`[API] Server responding after ${i + 1} attempts`)
        serverReadyResolve?.()
        return
      }
    } catch {}
    await new Promise(r => setTimeout(r, 1000))
  }
  console.warn('[API] Server poll timeout, proceeding anyway')
  serverReadyResolve?.()
}

// Start waiting immediately
waitForServer()

async function request(path, options = {}) {
  // Wait for server ready on Capacitor
  if (isCapacitor) {
    await serverReady
  }

  const token = localStorage.getItem('gl_token')
  const headers = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...options.headers,
  }

  try {
    const res = await fetch(`${API_BASE}${path}`, { ...options, headers, signal: AbortSignal.timeout(15000) })

    if (res.status === 401) {
      localStorage.removeItem('gl_token')
      localStorage.removeItem('gl_user')
      window.location.href = '/login'
      throw new Error('Non autorisé')
    }

    if (!res.ok) {
      const error = await res.json().catch(() => ({ message: 'Erreur serveur' }))
      throw new Error(error.message || `Erreur ${res.status}`)
    }

    return res.json()
  } catch (e) {
    if (e.name === 'TypeError' && e.message?.includes('Failed to fetch')) {
      throw new Error('Serveur indisponible — vérifiez la connexion')
    }
    throw e
  }
}

export const api = {
  get: (path) => request(path),
  post: (path, body) => request(path, { method: 'POST', body: JSON.stringify(body) }),
  put: (path, body) => request(path, { method: 'PUT', body: JSON.stringify(body) }),
  delete: (path) => request(path, { method: 'DELETE' }),
}
