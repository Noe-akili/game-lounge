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
let apiChecked = false

export function setApiUrl(url: string) {
  API_BASE = url
  localStorage.setItem('gl_api_url', url)
}

// ===== Port / server discovery =====
const CANDIDATE_BASES = [
  'http://127.0.0.1:3001/api',
  'http://localhost:3001/api',
]

async function probeUrl(url, timeoutMs = 1500) {
  try {
    const ctrl = new AbortController()
    const t = setTimeout(() => ctrl.abort(), timeoutMs)
    const res = await fetch(url + '/health', { signal: ctrl.signal })
    clearTimeout(t)
    return res.ok || res.status === 401 || res.status === 404
  } catch {
    return false
  }
}

async function checkServerAvailable() {
  if (!isCapacitor) return true

  for (let attempt = 0; attempt < 20; attempt++) {
    for (const candidate of CANDIDATE_BASES) {
      const ok = await probeUrl(candidate)
      if (ok) {
        if (candidate !== API_BASE) {
          API_BASE = candidate
          localStorage.setItem('gl_api_url', candidate)
        }
        console.log('[API] Server found at', API_BASE)
        return true
      }
    }
    await new Promise(r => setTimeout(r, 1000))
  }
  console.warn('[API] Server not found after 20s, using default:', API_BASE)
  return false
}

// Check once at module load (non-blocking)
checkServerAvailable()

async function request(path, options = {}) {
  const token = localStorage.getItem('gl_token')
  const headers = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...options.headers,
  }

  try {
    const res = await fetch(`${API_BASE}${path}`, {
      ...options,
      headers,
      signal: AbortSignal.timeout(15000),
    })

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
    if (e.name === 'TimeoutError' || e.name === 'AbortError') {
      throw new Error('Serveur indisponible — délai dépassé')
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
