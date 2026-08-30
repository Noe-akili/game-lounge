// @ts-nocheck
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/utils/api'

export const useJoueursStore = defineStore('joueurs', () => {
  const joueurs = ref([])
  const loading = ref(false)

  async function fetchJoueurs(search = '') {
    loading.value = true
    try {
      const params = search ? `?search=${encodeURIComponent(search)}` : ''
      joueurs.value = await api.get(`/joueurs${params}`)
    } finally {
      loading.value = false
    }
  }

  async function createJoueur(data) {
    const result = await api.post('/joueurs', data)
    joueurs.value.push(result)
    return result
  }

  async function updateJoueur(id, data) {
    const result = await api.put(`/joueurs/${id}`, data)
    const idx = joueurs.value.findIndex(j => j.id === id)
    if (idx !== -1) joueurs.value[idx] = result
    return result
  }

  async function getJoueurHistorique(id) {
    return await api.get(`/joueurs/${id}/historique`)
  }

  return { joueurs, loading, fetchJoueurs, createJoueur, updateJoueur, getJoueurHistorique }
})
