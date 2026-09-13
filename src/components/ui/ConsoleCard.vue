<template>
  <motion.div
    :initial="{ opacity: 0, y: 12 }"
    :animate="{ opacity: 1, y: 0 }"
    :whileHover="{ y: -4, scale: 1.01 }"
    :transition="{ duration: 0.25, ease: 'easeOut' }"
    class="card-hover relative overflow-hidden group cursor-pointer w-full max-w-full min-w-0 box-border"
  >
    <motion.div class="absolute top-0 left-0 w-full h-1" :class="statusBarColor" :animate="isOccupied ? { opacity: [1, 0.6, 1] } : {}" :transition="{ duration: 1.5, repeat: Infinity }" />

    <!-- Image de couverture (item 6) : visuel principal en arrière-plan, texte
         lisible par-dessus via un dégradé ; fallback icône si absente/invalide. -->
    <div v-if="console.image_url" class="absolute inset-0 z-0">
      <img :src="console.image_url" alt="" class="w-full h-full object-cover" @error="imgFailed = true" v-show="!imgFailed" />
      <div class="absolute inset-0 bg-gradient-to-t from-bg via-bg/80 to-bg/30"></div>
    </div>

    <div class="relative z-10 flex items-center justify-between gap-2 mb-3 min-w-0">
      <div class="flex items-center gap-2 min-w-0 flex-1">
        <Monitor v-if="imgFailed || !console.image_url" class="w-5 h-5 shrink-0" :class="statusIconColor" />
        <h3 class="font-gaming font-bold text-txt truncate min-w-0">{{ console.nom }}</h3>
      </div>
      <span class="badge shrink-0" :class="statusBadgeClass">{{ statusLabel }}</span>
    </div>

    <p class="relative z-10 text-xs text-txt-dim mb-3 truncate">Poste {{ console.poste_numero }} — {{ console.type }}</p>

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

      <!-- Temps RÉEL + progression vers la fin de la session : la barre avance
           chaque seconde et se remplit à mesure que le temps alloué s'écoule.
           Temps dépassé = barre et chrono en rouge (+X pulsé) : la session sera
           terminée automatiquement par le backend (notification Android envoyée). -->
      <div class="bg-bg-surface rounded-xl p-3 min-w-0">
        <div class="flex items-center gap-2">
          <Timer class="w-5 h-5 shrink-0" :class="depasse ? 'text-neon-red' : 'text-neon-blue'" />
          <LiveSessionTimer
            ref="liveTimer"
            :session-debut="console.session_debut"
            :duree-allouee="console.duree_allouee || 0"
            :accum-sec="console.duree_secondes ?? -1"
            :accumulee-min="console.duree_minutes || 0"
            :statut="console.session_statut || 'en_cours'"
            class="text-lg sm:text-xl tabular-nums"
          />
          <span v-if="restantAffiche" class="ml-auto text-xs shrink-0 text-right" :class="depasse ? 'text-neon-red' : 'text-txt-dim'">
            {{ depasse ? 'Terminaison auto…' : `Reste ${restantAffiche}` }}
          </span>
        </div>
        <div v-if="hasAllocation" class="mt-2 h-1.5 rounded-full bg-white/5 overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-1000"
            :class="depasse ? 'bg-neon-red' : restant < 5 * 60 ? 'bg-neon-yellow' : 'bg-neon-blue'"
            :style="{ width: progressPct + '%' }"
          />
        </div>
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
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { motion } from 'motion-v'
import { Monitor, User, Timer, TrendingUp, Play, Pause, Square } from 'lucide-vue-next'
import { formatCurrency, formatDuration } from '@/utils/helpers'
import LiveSessionTimer from './LiveSessionTimer.vue'

const props = defineProps({
  console: { type: Object, required: true },
})

defineEmits(['start', 'pause', 'resume', 'end'])

// Chrono temps réel partagé (LiveSessionTimer expose elapsedSeconds/restant).
const liveTimer = ref(null)
// Image de couverture : si l'URL échoue à charger, on retire l'img du DOM
// (fallback propre -> carte classique avec icône).
const imgFailed = ref(false)
const elapsed = computed(() => liveTimer.value?.elapsedSeconds ?? 0)
const restant = computed(() => liveTimer.value?.restant ?? 0)
const depasse = computed(() => liveTimer.value?.depasse ?? false)
const hasAllocation = computed(() => (props.console.duree_allouee || 0) > 0)
const progressPct = computed(() => {
  if (!hasAllocation.value) return 0
  return Math.min(100, Math.max(0, (elapsed.value / (props.console.duree_allouee * 60)) * 100))
})
const restantAffiche = computed(() =>
  hasAllocation.value ? formatDuration(Math.max(0, restant.value)) : ''
)

const isActive = computed(() => props.console.session_statut === 'en_cours' || props.console.session_statut === 'pause' || props.console.etat === 'occupee' || props.console.etat === 'pause')
const isOccupied = computed(() => props.console.session_statut === 'en_cours' || props.console.etat === 'occupee')
const isPaused = computed(() => props.console.session_statut === 'pause' || props.console.etat === 'pause')
const isFree = computed(() => props.console.etat === 'disponible' && !props.console.session_id)

const montantActuel = computed(() => {
  // MÊME règle que le backend (compute_montant) : forfait choisi + dépassement
  // prorata sur le taux du forfait. Avant : montant "horaire" qui démarrait à
  // 0 FC et grimpait -> contre-sens pour un forfait déjà payé d'avance.
  const tarif = props.console.tarif_prix || 2000
  const allouee = props.console.duree_allouee || 0
  if (allouee <= 0) return Math.ceil(elapsed.value / 3600) * tarif
  const joueeMin = Math.ceil(elapsed.value / 60)
  if (joueeMin <= allouee) return tarif
  const depassement = joueeMin - allouee
  return tarif + Math.ceil((depassement * tarif) / allouee)
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
