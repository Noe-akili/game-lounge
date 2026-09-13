<template>
  <div class="min-h-screen flex items-center justify-center bg-bg p-4">
    <div class="w-full max-w-md card space-y-6">
      <div class="text-center">
        <h1 class="font-gaming text-2xl font-bold text-txt">Initialisation de</h1>
        <h2 class="font-gaming text-3xl font-bold text-neon-violet">GAME LOUNGE</h2>
        <p class="text-txt-dim mt-2 text-sm">{{ subtitle }}</p>
      </div>

      <!-- Barre de progression RÉELLE (données du SyncEngine Rust via store sync) -->
      <div v-if="sync.status !== 'error'" class="space-y-3">
        <div class="w-full h-3 rounded-full bg-bg-hover overflow-hidden">
          <motion.div
            class="h-full rounded-full bg-neon-violet shadow-neon-violet"
            :initial="{ width: '0%' }"
            :animate="{ width: progressPct + '%' }"
            :transition="{ duration: 0.3, ease: 'easeOut' }"
          />
        </div>
        <div class="flex items-center justify-between text-sm">
          <span class="font-gaming font-bold text-txt">{{ progressPct }}%</span>
          <span class="text-txt-dim">{{ phaseLabel }}</span>
        </div>

        <!-- Liste des phases (état réel : fait / en cours / à venir) -->
        <div class="space-y-1.5 max-h-56 overflow-y-auto pr-1">
          <div v-for="t in SYNC_PHASES" :key="t" class="flex items-center gap-2 text-sm">
            <CheckCircle2 v-if="isPhaseDone(t)" class="w-4 h-4 text-neon-green shrink-0" />
            <Loader2 v-else-if="isPhaseActive(t)" class="w-4 h-4 text-neon-violet shrink-0 animate-spin" />
            <Circle v-else class="w-4 h-4 text-txt-dim shrink-0" />
            <span :class="isPhaseDone(t) ? 'text-txt' : isPhaseActive(t) ? 'text-neon-violet' : 'text-txt-dim'">
              {{ PHASE_LABELS[t] || t }}
            </span>
          </div>
        </div>

        <div class="text-center text-sm text-txt-dim">
          <p v-if="sync.processed > 0">{{ sync.processed.toLocaleString('fr-FR') }} changements traités</p>
          <p v-else class="animate-pulse">{{ sync.message || 'Préparation…' }}</p>
        </div>
      </div>

      <!-- ERREUR : connexion perdue / Supabase indisponible (mission §10) -->
      <div v-else class="space-y-4 text-center">
        <CloudOff class="w-12 h-12 text-neon-red mx-auto" />
        <p class="text-neon-red font-medium">Impossible de récupérer les données.</p>
        <p class="text-xs text-txt-dim">{{ sync.error || 'Vérifiez votre connexion internet.' }}</p>
        <div class="flex gap-3">
          <button @click="logout" class="btn-neon-outline flex-1">Plus tard</button>
          <button @click="retry" :disabled="retrying" class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
            <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': retrying }" />
            Réessayer
          </button>
        </div>
        <p class="text-[11px] text-txt-dim">La synchronisation reprendra où elle s'est arrêtée — aucune donnée n'est perdue.</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// Écran de PREMIÈRE synchronisation (mission §6) : affiché APRÈS le login réussi,
// uniquement quand initial_sync_completed = false côté SQLite. La progression est
// 100% réelle : événements Rust -> store Pinia sync (mission §7-8). En cas de
// coupure : bouton Réessayer (reprise aux tables restantes, mission §11).
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { motion } from 'motion-v'
import { useRouter } from 'vue-router'
import { CheckCircle2, Circle, Loader2, CloudOff, RefreshCw } from 'lucide-vue-next'
import { useSyncStore, SYNC_PHASES, PHASE_LABELS } from '@/stores/sync'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const sync = useSyncStore()
const auth = useAuthStore()
const retrying = ref(false)
let pollTimer: ReturnType<typeof setInterval> | null = null

const progressPct = computed(() => {
  if (sync.status === 'completed') return 100
  // Progression par PHASE (fiable même si un count réseau manque) croisée avec
  // le pourcentage détaillé envoyé par Rust quand il est disponible.
  const phaseBase = Math.round((sync.tablesDone.length / SYNC_PHASES.length) * 100)
  return Math.max(phaseBase, sync.percent || 0)
})

const subtitle = computed(() => {
  if (sync.status === 'completed') return 'Synchronisation terminée ✓'
  if (sync.status === 'initializing') return 'Synchronisation des données…'
  return 'Préparation de votre espace…'
})

const phaseLabel = computed(() => {
  if (!sync.phase) return sync.message || '…'
  return sync.message || PHASE_LABELS[sync.phase] || sync.phase
})

function isPhaseDone(t: string) {
  return sync.tablesDone.includes(t) || (sync.status === 'completed')
}
function isPhaseActive(t: string) {
  return sync.status !== 'completed' && sync.phase === t
}

async function retry() {
  retrying.value = true
  try { await sync.startInitialSync() } finally { retrying.value = false }
}

async function logout() {
  await auth.logout()
  sync.reset()
  router.replace('/login')
}

function goNext() {
  // Mission §12 : une fois l'init terminée, jamais revu — les connexions
  // suivantes vont directement au dashboard.
  router.replace(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
}

onMounted(async () => {
  sync.bindListeners()
  await sync.fetchInitialStatus()
  // Démarre la sync initiale si elle n'est pas déjà en cours (retour sur l'écran
  // après fermeture de l'app = reprise, mission §11).
  if (sync.initialSyncCompleted === false && sync.status !== 'completed') {
    if (sync.status === 'idle' || sync.status === 'error') await sync.startInitialSync()
  }
  // Filet si les événements Tauri ne passent pas : statut du SyncEngine via /sync/poll
  pollTimer = setInterval(async () => {
    try {
      const { api } = await import('@/utils/api')
      const r = await api.get('/sync/poll')
      sync.onPollDetail(r)
      if (r?.last_sync?.running === false && r?.last_sync?.success === false && sync.status !== 'completed') {
        sync.status = 'error'
        sync.error = r.last_sync.message || 'Synchronisation interrompue'
      }
      // Re-vérifie le flag de complétion (écrit par Rust en fin d'init)
      if (sync.status === 'syncing' || sync.status === 'initializing') {
        const done = sync.tablesDone.length >= SYNC_PHASES.length
        if (done && r?.last_sync?.running === false) {
          await sync.fetchInitialStatus()
          if (sync.initialSyncCompleted) {
            sync.status = 'completed'
            setTimeout(goNext, 800)
          }
        }
      }
    } catch {}
  }, 2500)
  // Filet de fin : si le flag passe à true pendant qu'on regarde, on continue
  const finishWatcher = setInterval(async () => {
    if (sync.status === 'completed') { clearInterval(finishWatcher); return }
    await sync.fetchInitialStatus()
    if (sync.initialSyncCompleted && sync.status !== 'error') {
      sync.status = 'completed'
      setTimeout(goNext, 800)
    }
  }, 4000)
  setTimeout(() => clearInterval(finishWatcher), 15 * 60 * 1000)
})

onUnmounted(() => { if (pollTimer) clearInterval(pollTimer) })
</script>
