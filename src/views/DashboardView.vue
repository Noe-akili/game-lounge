<template>
  <div class="min-h-full space-y-5 w-full max-w-full min-w-0 overflow-hidden">
    <!-- En-tête de poste : qui travaille, quand, et fraîcheur des données -->
    <div class="card w-full max-w-full min-w-0 overflow-hidden">
      <div class="flex items-start justify-between gap-3 min-w-0">
        <div class="min-w-0">
          <h2 class="font-gaming text-lg font-bold truncate">{{ salutation }}{{ prenom ? ', ' + prenom : '' }}</h2>
          <p class="text-xs text-txt-dim mt-1 truncate">{{ dateLongue }} — {{ heureCourante }}</p>
        </div>
        <div class="flex flex-col items-end gap-1 shrink-0">
          <button
            @click="fetchData"
            class="p-2 rounded-xl bg-bg-surface hover:bg-bg-hover text-txt-dim transition-colors"
            :aria-label="'Rafraîchir'"
          >
            <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': loading }" />
          </button>
          <span class="text-[10px] text-txt-muted whitespace-nowrap">{{ fraicheur }}</span>
        </div>
      </div>
    </div>

    <!-- Compteurs cliquables : un appui filtre la liste des consoles -->
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-2 sm:gap-3 w-full max-w-full">
      <motion.div
        v-for="(stat, i) in stats"
        :key="stat.key"
        :initial="{ opacity: 0, y: 12 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.3, delay: i * 0.06 }"
        class="stat-card cursor-pointer select-none"
        :class="filtre === stat.key ? 'border-neon-violet/50' : ''"
        role="button"
        @click="filtre = filtre === stat.key ? 'toutes' : stat.key"
      >
        <div class="flex items-center gap-2" :class="stat.color"><component :is="stat.icon" class="w-5 h-5" /></div>
        <span class="stat-value" :class="stat.color">{{ stat.value }}</span>
        <span class="stat-label">{{ stat.label }}</span>
      </motion.div>
    </div>

    <!-- Alertes : ce qui demande une action immédiate de l'employé -->
    <div v-if="alertes.length" class="card border-neon-yellow/30 w-full max-w-full min-w-0 overflow-hidden">
      <h3 class="font-gaming font-bold mb-3 flex items-center gap-2 min-w-0">
        <AlertTriangle class="w-4 h-4 text-neon-yellow shrink-0" />
        <span class="truncate">À surveiller</span>
        <span class="badge badge-yellow ml-auto shrink-0">{{ alertes.length }}</span>
      </h3>
      <div class="space-y-2">
        <!-- Deux lignes : le nom du poste garde toute la largeur, l'action reste
             à portée de pouce même sur un petit écran. -->
        <div
          v-for="c in alertes"
          :key="c.id"
          class="p-2 rounded-xl bg-bg-surface min-w-0"
        >
          <div class="flex items-center gap-2 min-w-0">
            <div class="w-2 h-2 rounded-full shrink-0" :class="estDepasse(c) ? 'bg-neon-red animate-pulse' : 'bg-neon-yellow'"></div>
            <p class="text-sm font-medium truncate flex-1 min-w-0">{{ c.nom }}</p>
            <!-- Temps restant, ou mention explicite quand le crédit est épuisé -->
            <span v-if="estDepasse(c)" class="badge badge-red shrink-0">Dépassé</span>
            <LiveSessionTimer
              v-else
              :session-debut="c.session_debut"
              :duree-allouee="c.duree_allouee || 0"
              :accum-sec="c.duree_secondes ?? -1"
              :accumulee-min="c.duree_minutes || 0"
              :statut="c.session_statut || 'en_cours'"
              mode="remaining"
              class="text-sm shrink-0"
            />
          </div>
          <div class="flex items-center gap-2 mt-1 min-w-0">
            <p class="text-xs text-txt-dim truncate flex-1 min-w-0">{{ c.joueur_nom || 'Joueur' }} — {{ c.jeu_nom || 'Jeu' }}</p>
            <button
              @click="openEndModal(c)"
              class="btn-neon-red px-3 py-1.5 text-xs shrink-0 flex items-center gap-1"
            >
              <Square class="w-4 h-4" />
              <span>Terminer</span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Bilan de la journée : visible sans passer par les écrans admin -->
    <div class="grid grid-cols-3 gap-2 sm:gap-3 w-full max-w-full">
      <div class="card py-3 px-2 min-w-0">
        <p class="text-[10px] leading-tight text-txt-muted uppercase tracking-wider">Sessions</p>
        <p class="font-gaming text-lg font-bold text-neon-blue leading-tight">{{ sessionsDuJour.length }}</p>
      </div>
      <div class="card py-3 px-2 min-w-0">
        <p class="text-[10px] leading-tight text-txt-muted uppercase tracking-wider">Encaissé</p>
        <p class="font-gaming text-lg font-bold text-neon-green leading-tight break-words">{{ formatCurrency(encaisseDuJour) }}</p>
      </div>
      <div class="card py-3 px-2 min-w-0">
        <p class="text-[10px] leading-tight text-txt-muted uppercase tracking-wider">Temps joué</p>
        <p class="font-gaming text-lg font-bold text-neon-violet leading-tight break-words">{{ tempsJoueAffiche }}</p>
      </div>
    </div>

    <!-- Recherche : utile dès que le salon compte beaucoup de postes -->
    <div class="flex items-center gap-2 min-w-0">
      <div class="relative flex-1 min-w-0">
        <Search class="w-4 h-4 text-txt-dim absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
        <input
          v-model="recherche"
          type="search"
          placeholder="Chercher une console, un joueur, un jeu…"
          class="input-field pl-9 w-full"
        />
      </div>
      <button
        v-if="filtre !== 'toutes' || recherche"
        @click="filtre = 'toutes'; recherche = ''"
        class="btn-neon-outline px-3 py-2 text-xs shrink-0"
      >
        Réinitialiser
      </button>
    </div>

    <div class="flex items-center justify-between gap-2 min-w-0">
      <h3 class="font-gaming text-lg font-bold truncate">État des consoles</h3>
      <span class="text-xs text-txt-dim shrink-0">{{ consolesFiltrees.length }}/{{ consoles.length }}</span>
    </div>

    <div v-if="loading && consoles.length === 0" class="card">
      <Loader variant="neon" size="lg" text="Chargement des consoles..." />
    </div>

    <div v-else-if="consolesFiltrees.length === 0" class="card text-center py-8">
      <Monitor class="w-8 h-8 text-txt-dim mx-auto mb-2" />
      <p class="text-sm text-txt-dim">
        {{ consoles.length === 0 ? 'Aucune console enregistrée' : 'Aucune console ne correspond à ce filtre' }}
      </p>
    </div>

    <div v-else class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
      <motion.div
        v-for="(c, i) in consolesFiltrees"
        :key="c.id"
        :initial="{ opacity: 0, y: 12 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.25, delay: Math.min(i, 8) * 0.04, ease: 'easeOut' }"
      >
        <ConsoleCard :console="c" @start="openStartModal" @pause="handlePause" @resume="handleResume" @end="openEndModal" />
      </motion.div>
    </div>

    <StartSessionModal :open="showStartModal" :console="selectedConsole" @close="showStartModal = false" @started="onSessionStarted" />
    <EndSessionModal :open="showEndModal" :console="selectedConsole" @close="showEndModal = false" @ended="onSessionEnded" />
  </div>
</template>

<script setup lang="ts">
// @ts-nocheck
// Tableau de bord EMPLOYÉ : tout ce qui sert pendant un poste de travail, sans
// accès aux écrans d'administration — état des consoles, alertes de fin de
// session, bilan de la journée, recherche.
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { motion } from 'motion-v'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'
import { formatCurrency, formatDuration } from '@/utils/helpers'
import { useAuthStore } from '@/stores/auth'
import { AlertTriangle, Monitor, Pause, RefreshCw, Search, Square } from 'lucide-vue-next'
import ConsoleCard from '@/components/ui/ConsoleCard.vue'
import StartSessionModal from '@/components/ui/StartSessionModal.vue'
import EndSessionModal from '@/components/ui/EndSessionModal.vue'
import LiveSessionTimer from '@/components/ui/LiveSessionTimer.vue'
import Loader from '@/components/ui/Loader.vue'

const auth = useAuthStore()

const consoles = ref([])
const sessions = ref([])
const loading = ref(false)
const showStartModal = ref(false)
const showEndModal = ref(false)
const selectedConsole = ref(null)
const recherche = ref('')
const filtre = ref('toutes')
const derniereMaj = ref(null)
const maintenant = ref(Date.now())

let refreshInterval = null
let horloge = null

// ===== Identité et horloge =====

const prenom = computed(() => (auth.user?.nom || '').trim().split(' ')[0] || '')

const salutation = computed(() => {
  const h = new Date(maintenant.value).getHours()
  if (h < 12) return 'Bonjour'
  if (h < 18) return 'Bon après-midi'
  return 'Bonsoir'
})

const dateLongue = computed(() =>
  new Intl.DateTimeFormat('fr-FR', { weekday: 'long', day: 'numeric', month: 'long' }).format(new Date(maintenant.value))
)

const heureCourante = computed(() =>
  new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' }).format(new Date(maintenant.value))
)

const fraicheur = computed(() => {
  if (!derniereMaj.value) return ''
  const sec = Math.max(0, Math.floor((maintenant.value - derniereMaj.value) / 1000))
  if (sec < 10) return 'à jour'
  if (sec < 60) return `il y a ${sec} s`
  return `il y a ${Math.floor(sec / 60)} min`
})

// ===== Compteurs =====

const aUneSession = (c) => !!c.session_id && (c.session_statut === 'en_cours' || c.session_statut === 'pause')
const estOccupee = (c) => aUneSession(c) && c.session_statut === 'en_cours'
const estEnPause = (c) => aUneSession(c) && c.session_statut === 'pause'
const estLibre = (c) => !aUneSession(c) && c.etat !== 'maintenance' && c.etat !== 'hors_service'

const stats = computed(() => [
  { key: 'toutes', label: 'Consoles', value: consoles.value.length, color: 'text-neon-blue', icon: Monitor },
  { key: 'occupees', label: 'Occupées', value: consoles.value.filter(estOccupee).length, color: 'text-neon-red', icon: Monitor },
  { key: 'pause', label: 'En pause', value: consoles.value.filter(estEnPause).length, color: 'text-neon-yellow', icon: Pause },
  { key: 'libres', label: 'Libres', value: consoles.value.filter(estLibre).length, color: 'text-neon-green', icon: Monitor },
])

// ===== Alertes de fin de session (moins de 5 min, ou temps dépassé) =====

// Secondes déjà consommées : même règle que LiveSessionTimer (cumul + temps
// écoulé depuis la reprise, gelé en pause).
function secondesEcoulees(c) {
  const base = (c.duree_secondes ?? -1) >= 0 ? c.duree_secondes : (c.duree_minutes || 0) * 60
  if (c.session_statut === 'pause' || !c.session_debut) return base
  const debut = new Date(c.session_debut).getTime()
  if (Number.isNaN(debut)) return base
  return base + Math.max(0, Math.floor((maintenant.value - debut) / 1000))
}

function secondesRestantes(c) {
  const allouee = c.duree_allouee || 0
  if (allouee <= 0) return null
  return allouee * 60 - secondesEcoulees(c)
}

const estDepasse = (c) => {
  const r = secondesRestantes(c)
  return r !== null && r < 0
}

const alertes = computed(() =>
  consoles.value
    .filter((c) => estOccupee(c))
    .filter((c) => {
      const r = secondesRestantes(c)
      return r !== null && r < 5 * 60
    })
    .sort((a, b) => (secondesRestantes(a) ?? 0) - (secondesRestantes(b) ?? 0))
)

// ===== Bilan de la journée =====

function memeJour(valeur) {
  if (!valeur) return false
  const d = new Date(valeur)
  if (Number.isNaN(d.getTime())) return false
  const n = new Date(maintenant.value)
  return d.getFullYear() === n.getFullYear() && d.getMonth() === n.getMonth() && d.getDate() === n.getDate()
}

const sessionsDuJour = computed(() => sessions.value.filter((s) => memeJour(s.debut || s.created_at)))

const encaisseDuJour = computed(() =>
  sessionsDuJour.value
    .filter((s) => s.statut === 'terminee')
    .reduce((total, s) => total + (Number(s.montant) || 0), 0)
)

const tempsJoueAffiche = computed(() => {
  const sec = sessionsDuJour.value.reduce(
    (total, s) => total + ((s.duree_secondes ?? -1) >= 0 ? s.duree_secondes : (s.duree_minutes || 0) * 60),
    0
  )
  return formatDuration(sec)
})

// ===== Filtrage de la grille =====

const consolesFiltrees = computed(() => {
  const q = recherche.value.trim().toLowerCase()
  return consoles.value.filter((c) => {
    if (filtre.value === 'occupees' && !estOccupee(c)) return false
    if (filtre.value === 'pause' && !estEnPause(c)) return false
    if (filtre.value === 'libres' && !estLibre(c)) return false
    if (!q) return true
    return [c.nom, c.type, c.joueur_nom, c.jeu_nom, c.poste_numero]
      .some((v) => String(v ?? '').toLowerCase().includes(q))
  })
})

// ===== Chargement =====

async function fetchData() {
  loading.value = true
  try {
    const [c, s] = await Promise.all([
      api.get('/consoles'),
      api.get('/sessions').catch(() => []),
    ])
    consoles.value = Array.isArray(c) ? c : []
    sessions.value = Array.isArray(s) ? s : []
    derniereMaj.value = Date.now()
  } catch (e) {
    toast.error(e?.message || 'Impossible de charger les consoles')
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
  if (!c?.session_id) {
    toast.error('Aucune session active sur cette console')
    return
  }
  try {
    await api.put(`/sessions/${c.session_id}/pause`)
    toast.success('Session mise en pause')
    await fetchData()
  } catch (e) {
    toast.error(e.message)
  }
}

async function handleResume(c) {
  if (!c?.session_id) {
    toast.error('Aucune session active sur cette console')
    return
  }
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
  toast.success('Session démarrée')
}

function onSessionEnded() {
  showEndModal.value = false
  fetchData()
}

function onSyncPoll(e: Event) {
  const d = (e as CustomEvent).detail?.changes
  if (d?.sessions_jeu || d?.consoles || d?.factures) fetchData()
}

// Rafraîchissement suspendu quand l'application passe en arrière-plan :
// inutile de solliciter la base et la batterie de l'appareil Android.
function onVisibilityChange() {
  if (document.hidden) {
    if (refreshInterval) { clearInterval(refreshInterval); refreshInterval = null }
  } else if (!refreshInterval) {
    fetchData()
    refreshInterval = setInterval(fetchData, 10000)
  }
}

onMounted(() => {
  fetchData()
  refreshInterval = setInterval(fetchData, 10000)
  horloge = setInterval(() => { maintenant.value = Date.now() }, 1000)
  window.addEventListener('sync-poll', onSyncPoll)
  document.addEventListener('visibilitychange', onVisibilityChange)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
  if (horloge) clearInterval(horloge)
  window.removeEventListener('sync-poll', onSyncPoll)
  document.removeEventListener('visibilitychange', onVisibilityChange)
})
</script>
