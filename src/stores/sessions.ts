// @ts-nocheck
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'

export const useSessionsStore = defineStore('sessions', () => {
  const sessions = ref([])
  const activeSessions = ref([])
  const loading = ref(false)

  const sessionCount = computed(() => activeSessions.value.length)

  async function fetchSessions() {
    loading.value = true
    try {
      sessions.value = await api.get('/sessions')
      activeSessions.value = sessions.value.filter(s => s.statut === 'en_cours' || s.statut === 'pause')
    } finally {
      loading.value = false
    }
  }

  async function fetchActiveSessions() {
    const data = await api.get('/sessions?statut=en_cours')
    activeSessions.value = data
    return data
  }

  async function startSession(payload) {
    const result = await api.post('/sessions', payload)
    activeSessions.value.push(result)
    return result
  }

  async function pauseSession(id) {
    const result = await api.put(`/sessions/${id}/pause`)
    const idx = activeSessions.value.findIndex(s => s.id === id)
    if (idx !== -1) activeSessions.value[idx] = result
    return result
  }

  async function resumeSession(id) {
    const result = await api.put(`/sessions/${id}/reprendre`)
    const idx = activeSessions.value.findIndex(s => s.id === id)
    if (idx !== -1) activeSessions.value[idx] = result
    return result
  }

  async function endSession(id) {
    const result = await api.put(`/sessions/${id}/terminer`)
    activeSessions.value = activeSessions.value.filter(s => s.id !== id)
    return result
  }

  return { sessions, activeSessions, loading, sessionCount, fetchSessions, fetchActiveSessions, startSession, pauseSession, resumeSession, endSession }
})
