// @ts-nocheck
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '@/utils/api'

export const useFacturesStore = defineStore('factures', () => {
  const factures = ref([])
  const loading = ref(false)

  async function fetchFactures(filters = {}) {
    loading.value = true
    try {
      const params = new URLSearchParams(filters).toString()
      factures.value = await api.get(`/factures${params ? '?' + params : ''}`)
    } finally {
      loading.value = false
    }
  }

  async function getFacture(id) {
    return await api.get(`/factures/${id}`)
  }

  async function cancelFacture(id, motif) {
    return await api.put(`/factures/${id}/annuler`, { motif })
  }

  async function getFacturePdf(id) {
    const res = await fetch(`/api/factures/${id}/pdf`, {
      headers: { Authorization: `Bearer ${localStorage.getItem('gl_token')}` }
    })
    return await res.blob()
  }

  return { factures, loading, fetchFactures, getFacture, cancelFacture, getFacturePdf }
})
