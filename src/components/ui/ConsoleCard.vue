<template>
  <motion.div
    :initial="{ opacity: 0, y: 12 }"
    :animate="{ opacity: 1, y: 0 }"
    :whileHover="{ y: -4, scale: 1.01 }"
    :transition="{ duration: 0.25, ease: 'easeOut' }"
    class="card-hover relative overflow-hidden group cursor-pointer w-full max-w-full min-w-0 box-border"
  >
    <motion.div class="absolute top-0 left-0 w-full h-1" :class="statusBarColor" :animate="isOccupied ? { opacity: [1, 0.6, 1] } : {}" :transition="{ duration: 1.5, repeat: Infinity }" />

    <div class="flex items-center justify-between gap-2 mb-3 min-w-0">
      <div class="flex items-center gap-2 min-w-0 flex-1">
        <Monitor class="w-5 h-5 shrink-0" :class="statusIconColor" />
        <h3 class="font-gaming font-bold text-txt truncate min-w-0">{{ console.nom }}</h3>
      </div>
      <span class="badge shrink-0" :class="statusBadgeClass">{{ statusLabel }}</span>
    </div>

    <p class="text-xs text-txt-dim mb-3 truncate">Poste {{ console.poste_numero }} — {{ console.type }}</p>

    <div v-if="isActive" class="mb-4 space-y-2 min-w-0">
      <div class="flex items-center gap-2 min-w-0">
        <div class="w-8 h-8 rounded-full bg-neon-violet/20 flex items-center justify-center shrink-0">
          <User class="w-4 h-4 text-neon-violet" />
        </div>
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium truncate">{{ console.joueur_nom || 'Joueur' }}</p>
          <p class="text-xs text-txt-dim truncate">{{ console.jeu_nom || 'Jeu' }}</p>
        </div>
      </div>

      <div class="flex items-center gap-2 bg-bg-surface rounded-xl p-3 min-w-0">
        <Timer class="w-5 h-5 text-neon-blue shrink-0" />
        <span class="font-gaming text-lg sm:text-xl font-bold text-neon-blue truncate">{{ timerDisplay }}</span>
      </div>

      <div v-if="isOccupied" class="flex items-center gap-2 text-sm text-txt-dim min-w-0">
        <TrendingUp class="w-4 h-4 shrink-0" />
        <span class="truncate">Tarif : {{ formatCurrency(console.tarif_prix || 2000) }}/h</span>
        <span class="ml-auto font-semibold text-neon-green shrink-0">{{ formatCurrency(montantActuel) }}</span>
      </div>
    </div>

    <div class="flex gap-2 flex-wrap sm:flex-nowrap min-w-0">
      <button v-if="isFree" @click="$emit('start', console)"
        class="btn-neon-green flex-1 flex items-center justify-center gap-2">
        <Play class="w-5 h-5" />
        <span>Démarrer</span>
      </button>

      <template v-if="isOccupied">
        <button @click="$emit('pause', console)"
          class="btn-neon flex-1 bg-neon-yellow/15 text-neon-yellow border border-neon-yellow/30 hover:bg-neon-yellow/25 flex items-center justify-center gap-2">
          <Pause class="w-5 h-5" />
          <span>Pause</span>
        </button>
        <button @click="$emit('end', console)"
          class="btn-neon-red flex-1 flex items-center justify-center gap-2">
          <Square class="w-5 h-5" />
          <span>Terminer</span>
        </button>
      </template>

      <button v-if="isPaused" @click="$emit('resume', console)"
        class="btn-neon-blue flex-1 flex items-center justify-center gap-2">
        <Play class="w-5 h-5" />
        <span>Reprendre</span>
      </button>
    </div>
  </motion.div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { motion } from 'motion-v'
import { Monitor, User, Timer, TrendingUp, Play, Pause, Square } from 'lucide-vue-next'
import { formatCurrency, formatDuration } from '@/utils/helpers'

const props = defineProps({
  console: { type: Object, required: true },
})

defineEmits(['start', 'pause', 'resume', 'end'])

const elapsed = ref(0)
let timer = null

const isActive = computed(() => props.console.session_statut === 'en_cours' || props.console.session_statut === 'pause' || props.console.etat === 'occupee' || props.console.etat === 'pause')
const isOccupied = computed(() => props.console.session_statut === 'en_cours' || props.console.etat === 'occupee')
const isPaused = computed(() => props.console.session_statut === 'pause' || props.console.etat === 'pause')
const isFree = computed(() => props.console.etat === 'disponible' && !props.console.session_id)

function startTimer() {
  if (timer) clearInterval(timer)
  if (!props.console.session_debut) { elapsed.value = 0; return }

  const computeElapsed = () => {
    if (props.console.session_statut === 'pause') {
      return (props.console.duree_minutes || 0) * 60
    }
    const debut = new Date(props.console.session_debut).getTime()
    return Math.floor((Date.now() - debut) / 1000)
  }

  elapsed.value = computeElapsed()
  if (isOccupied.value) {
    timer = setInterval(() => { elapsed.value++ }, 1000)
  }
}

onMounted(startTimer)
onUnmounted(() => { if (timer) clearInterval(timer) })

const timerDisplay = computed(() => formatDuration(elapsed.value))
const montantActuel = computed(() => {
  const minutes = elapsed.value / 60
  const tarif = props.console.tarif_prix || 2000
  return Math.ceil(minutes / 60) * tarif
})

const statusLabel = computed(() => {
  if (props.console.session_statut === 'en_cours' || props.console.etat === 'occupee') return 'Occupé'
  if (props.console.session_statut === 'pause' || props.console.etat === 'pause') return 'En pause'
  if (props.console.etat === 'maintenance') return 'Maintenance'
  if (props.console.etat === 'hors_service') return 'Hors service'
  return 'Libre'
})

const statusBadgeClass = computed(() => {
  if (isOccupied.value) return 'badge-red'
  if (isPaused.value) return 'badge-yellow'
  return 'badge-green'
})

const statusBarColor = computed(() => {
  if (isOccupied.value) return 'bg-neon-red'
  if (isPaused.value) return 'bg-neon-yellow'
  return 'bg-neon-green'
})

const statusIconColor = computed(() => {
  if (isOccupied.value) return 'text-neon-red'
  if (isPaused.value) return 'text-neon-yellow'
  return 'text-neon-green'
})
</script>
