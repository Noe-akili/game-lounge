// Auth mono-utilisateur admin : PAS de login. L'application considère que la
// personne qui tient l'appareil EST l'admin : la session est bootstrappée
// automatiquement (auth_bootstrap_admin) et restaurée via /auth/me.
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'

function loadUser() {
  try {
    const raw = localStorage.getItem('gl_user')
    if (!raw || raw === 'null') return null
    return JSON.parse(raw)
  } catch { return null }
}
function loadToken() {
  try { return localStorage.getItem('gl_token') } catch { return null }
}

export const useAuthStore = defineStore('auth', () => {
  const user = ref(loadUser())
  const token = ref(loadToken())
  const sessionReady = ref(false)

  const isAuthenticated = computed(() => !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')

  function setSession(data: any) {
    token.value = data.token
    user.value = data.user
    localStorage.setItem('gl_token', data.token)
    localStorage.setItem('gl_user', JSON.stringify(data.user))
    if (data.refresh_token) localStorage.setItem('gl_refresh_token', data.refresh_token)
  }

  function clearSession() {
    token.value = null
    user.value = null
    try {
      localStorage.removeItem('gl_token')
      localStorage.removeItem('gl_user')
      localStorage.removeItem('gl_refresh_token')
    } catch {}
  }

  /// Connexion implicite admin : appelée automatiquement, sans écran de login.
  async function bootstrapAdmin() {
    const data = await api.post('/auth/bootstrap')
    setSession(data)
    return data.user
  }

  async function restoreSession() {
    if (sessionReady.value) return isAuthenticated.value
    try {
      if (token.value && user.value) {
        // Session existante : on la vérifie auprès du backend.
        await fetchMe()
        return true
      }
      // Pas de session (premier lancement ou stockage vidé) : l'admin est
      // connecté d'office, sans écran de connexion.
      await bootstrapAdmin()
      return true
    } catch {
      try { await bootstrapAdmin(); return true } catch { clearSession(); return false }
    }
  }

  async function fetchMe() {
    const data = await api.get('/auth/me')
    user.value = data.user
    localStorage.setItem('gl_user', JSON.stringify(data.user))
  }

  if (typeof window !== 'undefined') {
    window.addEventListener('gl:unauthorized', () => {
      // 401 (token expiré/invalide) : on re-bootstrappe la session admin au
      // prochain passage du guard, au lieu de renvoyer vers un écran de login.
      clearSession()
      sessionReady.value = false
    })
  }

  return { user, token, sessionReady, isAuthenticated, isAdmin, bootstrapAdmin, fetchMe, restoreSession, clearSession }
})
