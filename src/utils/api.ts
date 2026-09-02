// @ts-nocheck
function getApiBase() {
  const envUrl = import.meta.env.VITE_API_URL
  if (envUrl) return envUrl

  if (window.location.protocol === 'capacitor:' || window.location.protocol === 'https:') {
    return 'http://localhost:3001/api'
  }

  return '/api'
}

const API_BASE = getApiBase()

async function request(path, options = {}) {
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
      throw new Error('Non autorise')
    }

    if (!res.ok) {
      const error = await res.json().catch(() => ({ message: 'Erreur serveur' }))
      throw new Error(error.message || `Erreur ${res.status}`)
    }

    return res.json()
  } catch (e) {
    if (e.name === 'TypeError' && e.message?.includes('Failed to fetch')) {
      throw new Error('Serveur indisponible — verifiez la connexion')
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
