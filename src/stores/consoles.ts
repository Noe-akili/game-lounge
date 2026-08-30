// @ts-nocheck
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '@/utils/api'

export const useConsolesStore = defineStore('consoles', () => {
  const consoles = ref([])
  const loading = ref(false)

  const libres = computed(() => consoles.value.filter(c => c.etat === 'disponible'))
  const occupees = computed(() => consoles.value.filter(c => c.etat === 'occupee'))
  const enPause = computed(() => consoles.value.filter(c => c.etat === 'pause'))
  const horsService = computed(() => consoles.value.filter(c => c.etat === 'hors_service'))
  const totalCount = computed(() => consoles.value.length)

  async function fetchConsoles() {
    loading.value = true
    try {
      consoles.value = await api.get('/consoles')
    } finally {
      loading.value = false
    }
  }

  async function createConsole(data) {
    const result = await api.post('/consoles', data)
    consoles.value.push(result)
    return result
  }

  async function updateConsole(id, data) {
    const result = await api.put(`/consoles/${id}`, data)
    const idx = consoles.value.findIndex(c => c.id === id)
    if (idx !== -1) consoles.value[idx] = result
    return result
  }

  async function deleteConsole(id) {
    await api.delete(`/consoles/${id}`)
    consoles.value = consoles.value.filter(c => c.id !== id)
  }

  return { consoles, loading, libres, occupees, enPause, horsService, totalCount, fetchConsoles, createConsole, updateConsole, deleteConsole }
})
