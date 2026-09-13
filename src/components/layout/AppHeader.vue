<template>
  <header class="sticky top-0 z-30 bg-bg/80 backdrop-blur-xl border-b border-white/5">
    <div class="flex items-center justify-between px-4 lg:px-6 h-16">
      <div class="flex items-center gap-4">
        <button @click="$emit('toggleSidebar')" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim lg:hidden">
          <Menu v-if="!sidebarOpen" class="w-5 h-5" />
          <X v-else class="w-5 h-5" />
        </button>
        <div>
          <h2 class="font-gaming text-lg font-bold">{{ pageTitle }}</h2>
          <p class="text-xs text-txt-dim">Gestion des sessions en temps réel</p>
        </div>
      </div>

      <div class="flex items-center gap-4">
        <!-- Delta sync discret (mission §12) : le dashboard reste utilisable pendant
             la synchronisation en arrière-plan. visible = admin (statut) ou tous
             (progression émise par Rust pendant un delta/en init). -->
        <div
          v-if="syncBadgeVisible"
          class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-neon-green/10 border border-neon-green/20"
          :title="sync.message || 'Synchronisation cloud'"
        >
          <RefreshCw class="w-3.5 h-3.5 text-neon-green" :class="{ 'animate-spin': sync.isBusy }" />
          <span class="text-[10px] font-medium text-neon-green">{{ syncBadgeText }}</span>
        </div>
        <div v-else-if="syncEnabled" class="hidden sm:flex items-center gap-2 px-3 py-1.5 rounded-full bg-neon-green/10 border border-neon-green/20" title="Sync cloud active">
          <Cloud class="w-3.5 h-3.5 text-neon-green" :class="{ 'animate-pulse': syncActive }" />
          <span class="text-[10px] font-medium text-neon-green">Sync</span>
        </div>
        <div class="text-right hidden sm:block">
          <p class="text-sm font-medium font-gaming">{{ currentTime }}</p>
          <p class="text-[10px] text-txt-dim">{{ currentDate }}</p>
        </div>
        <button class="relative p-2 rounded-xl hover:bg-bg-hover text-txt-dim transition-colors">
          <Bell class="w-5 h-5" />
          <span class="absolute top-1 right-1 w-2 h-2 bg-neon-red rounded-full"></span>
        </button>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { Menu, X, Bell, Cloud, RefreshCw } from 'lucide-vue-next'
import { api } from '@/utils/api'
import { useAuthStore } from '@/stores/auth'
import { useSyncStore, PHASE_LABELS } from '@/stores/sync'

defineProps({ sidebarOpen: Boolean })
defineEmits(['toggleSidebar'])

const route = useRoute()
const auth = useAuthStore()
const sync = useSyncStore()
const now = ref(new Date())
let timer = null
let syncTimer = null
const syncEnabled = ref(false)
const syncActive = ref(false)

async function checkSync() {
  try {
    if (auth.user?.role === 'admin') {
      const s = await api.get('/sync/status')
      syncEnabled.value = s.enabled && s.supabaseConnected
      syncActive.value = s.syncing
    }
  } catch {}
}

onMounted(() => {
  timer = setInterval(() => { now.value = new Date() }, 1000)
  checkSync()
  syncTimer = setInterval(checkSync, 30000)
  sync.bindListeners() // événements Rust sync-progress etc. (delta sync visible)
})
onUnmounted(() => { clearInterval(timer); clearInterval(syncTimer) })

// Badge delta sync : "↻ Sync" pendant un delta, "N changements" restants, "⚠" en erreur.
const syncBadgeVisible = computed(() => sync.isBusy || sync.status === 'error')
const syncBadgeText = computed(() => {
  if (sync.status === 'error') return 'Sync échouée'
  if (sync.phase && PHASE_LABELS[sync.phase]) return PHASE_LABELS[sync.phase]
  if (sync.total > 0) return `Sync ${sync.current}/${sync.total}`
  return 'Synchronisation…'
})

const currentTime = computed(() =>
  now.value.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })
)

const currentDate = computed(() =>
  now.value.toLocaleDateString('fr-FR', { day: '2-digit', month: 'short', year: 'numeric' })
)

const pageTitle = computed(() => {
  const map = {
    '/dashboard': 'Tableau de bord',
    '/sessions': 'Sessions',
    '/joueurs': 'Joueurs',
    '/paiements': 'Paiements',
    '/jetons': 'Jetons',
    '/messages': 'Messages',
    '/parametres': 'Paramètres',
    '/admin': 'Tableau de bord Admin',
    '/admin/consoles': 'Gestion des consoles',
    '/admin/jeux': 'Catalogue des jeux',
    '/admin/tarifs': 'Paramétrage des tarifs',
    '/admin/rapports': 'Rapports & Statistiques',
    '/admin/parametres': 'Paramètres',
    '/admin/utilisateurs': 'Utilisateurs',
  }
  return map[route.path] || 'Game Lounge'
})
</script>
