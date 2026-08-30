<template>
  <div class="min-h-full space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-2 sm:gap-3 w-full max-w-full">
      <motion.div v-for="(stat, i) in stats" :key="stat.label" :initial="{ opacity: 0, y: 12 }" :animate="{ opacity: 1, y: 0 }" :transition="{ duration: 0.3, delay: i * 0.08 }" class="stat-card">
        <div class="flex items-center gap-2" :class="stat.color"><component :is="stat.icon" class="w-5 h-5" /></div>
        <span class="stat-value" :class="stat.color">{{ stat.value }}</span>
        <span class="stat-label">{{ stat.label }}</span>
      </motion.div>
    </div>

    <div class="flex items-center justify-between">
      <h3 class="font-gaming text-lg font-bold">État des Consoles</h3>
      <button @click="fetchData" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim transition-colors" :class="{ 'animate-spin': loading }">
        <RefreshCw class="w-4 h-4" />
      </button>
    </div>

    <div v-if="loading && consoles.length === 0" class="card">
      <Loader variant="neon" size="lg" text="Chargement des consoles..." />
    </div>

    <div v-else class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
      <motion.div v-for="(c, i) in consoles" :key="c.id" :initial="{ opacity: 0, y: 12 }" :animate="{ opacity: 1, y: 0 }" :transition="{ duration: 0.25, delay: i * 0.04, ease: 'easeOut' }">
        <ConsoleCard :console="c" @start="openStartModal" @pause="handlePause" @resume="handleResume" @end="openEndModal" />
      </motion.div>
    </div>

    <StartSessionModal :open="showStartModal" :console="selectedConsole" @close="showStartModal = false" @started="onSessionStarted" />
    <EndSessionModal :open="showEndModal" :console="selectedConsole" @close="showEndModal = false" @ended="onSessionEnded" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { motion } from 'motion-v'
import gsap from 'gsap'
import { api } from '@/utils/api'
import { toast } from 'sonner'
import { Monitor, Pause, RefreshCw } from 'lucide-vue-next'
import ConsoleCard from '@/components/ui/ConsoleCard.vue'
import StartSessionModal from '@/components/ui/StartSessionModal.vue'
import EndSessionModal from '@/components/ui/EndSessionModal.vue'
import Loader from '@/components/ui/Loader.vue'

const consoles = ref([])
const loading = ref(false)
const showStartModal = ref(false)
const showEndModal = ref(false)
const selectedConsole = ref(null)
let refreshInterval = null

const occupiedCount = computed(() => consoles.value.filter((c: any) => c.statut === 'occupee' || c.session_statut === 'en_cours').length)
const pausedCount = computed(() => consoles.value.filter((c: any) => c.statut === 'pause' || c.session_statut === 'pause').length)
const freeCount = computed(() => consoles.value.filter((c: any) => c.statut === 'disponible' && !c.session_id).length)

const stats = computed(() => [
  { label: 'Consoles totales', value: consoles.value.length, color: 'text-neon-blue', icon: Monitor },
  { label: 'Occupées', value: occupiedCount.value, color: 'text-neon-red', icon: Monitor },
  { label: 'En pause', value: pausedCount.value, color: 'text-neon-yellow', icon: Pause },
  { label: 'Libres', value: freeCount.value, color: 'text-neon-green', icon: Monitor },
])

async function fetchData() {
  loading.value = true
  try {
    consoles.value = await api.get('/consoles')
  } catch (e) {
    console.error('Erreur chargement consoles:', e)
  } finally {
    loading.value = false
  }
}

function openStartModal(c) {
  selectedConsole.value = c
  showStartModal.value = true
}

function openEndModal(c) {
  selectedConsole.value = c
  showEndModal.value = true
}

async function handlePause(c) {
  try {
    await api.put(`/sessions/${c.session_id}/pause`)
    toast.success('Session mise en pause')
    await fetchData()
  } catch (e) {
    toast.error(e.message)
  }
}

async function handleResume(c) {
  try {
    await api.put(`/sessions/${c.session_id}/reprendre`)
    toast.success('Session reprise')
    await fetchData()
  } catch (e) {
    toast.error(e.message)
  }
}

function onSessionStarted() {
  showStartModal.value = false
  fetchData()
  toast.success('Session démarrée !')
}

function onSessionEnded() {
  showEndModal.value = false
  fetchData()
}

onMounted(() => {
  fetchData()
  refreshInterval = setInterval(fetchData, 10000)
  window.addEventListener('sync-poll', onSyncPoll)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
  window.removeEventListener('sync-poll', onSyncPoll)
})

function onSyncPoll(e: Event) {
  const d = (e as CustomEvent).detail?.changes
  if (d?.sessions_jeu || d?.consoles || d?.factures) fetchData()
}
</script>
