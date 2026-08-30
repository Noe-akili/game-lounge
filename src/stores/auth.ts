// @ts-nocheck
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'

export const useAuthStore = defineStore('auth', () => {
  const user = ref(JSON.parse(localStorage.getItem('gl_user') || 'null'))
  const token = ref(localStorage.getItem('gl_token') || null)

  const isAuthenticated = computed(() => !!token.value && !!user.value)
  const isAdmin = computed(() => user.value?.role === 'admin')
  const isEmploye = computed(() => user.value?.role === 'employe')

  async function login(email, password) {
    const data = await api.post('/auth/login', { email, password })
    token.value = data.token
    user.value = data.user
    localStorage.setItem('gl_token', data.token)
    localStorage.setItem('gl_user', JSON.stringify(data.user))
    return data.user
  }

  function logout() {
    token.value = null
    user.value = null
    localStorage.removeItem('gl_token')
    localStorage.removeItem('gl_user')
  }

  async function fetchMe() {
    try {
      const data = await api.get('/auth/me')
      user.value = data.user
      localStorage.setItem('gl_user', JSON.stringify(data.user))
    } catch {
      logout()
    }
  }

  return { user, token, isAuthenticated, isAdmin, isEmploye, login, logout, fetchMe }
})
