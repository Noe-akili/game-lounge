// @ts-nocheck
const isCapacitor = typeof window !== 'undefined' && (window as any).Capacitor
const API_BASE = isCapacitor ? 'http://127.0.0.1:3001/api' : '/api'

async function request(path, options = {}) {
  const token = localStorage.getItem('gl_token')
  const headers = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...options.headers,
  }

  const res = await fetch(`${API_BASE}${path}`, { ...options, headers })

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
}

export const api = {
  get: (path) => request(path),
  post: (path, body) => request(path, { method: 'POST', body: JSON.stringify(body) }),
  put: (path, body) => request(path, { method: 'PUT', body: JSON.stringify(body) }),
  delete: (path) => request(path, { method: 'DELETE' }),
}
