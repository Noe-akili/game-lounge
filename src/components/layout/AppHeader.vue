<template>
  <!-- safe-top : le contenu commence SOUS la barre de statut Android
       (insets injectés par le patch natif, fallback env(safe-area-inset-*)). -->
  <header class="sticky top-0 z-30 bg-bg/80 backdrop-blur-xl border-b border-white/5 safe-top">
    <!-- h-20 (au lieu de h-16) : les longs titres (ex. "Catalogue des jeux",
         "Paramétrage des tarifs") ne débordent plus ; ils sont aussi tronqués
         avec des points de suspension en dernier recours. -->
    <div class="flex items-center justify-between gap-3 px-4 lg:px-6 h-20">
      <div class="flex items-center gap-3 sm:gap-4 min-w-0 flex-1">
        <button @click="$emit('toggleSidebar')" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim lg:hidden shrink-0">
          <Menu v-if="!sidebarOpen" class="w-5 h-5" />
          <X v-else class="w-5 h-5" />
        </button>
        <div class="min-w-0">
          <h2 class="font-gaming text-lg font-bold truncate">{{ pageTitle }}</h2>
          <p class="text-xs text-txt-dim truncate">Gestion des sessions en temps réel</p>
        </div>
      </div>

      <div class="flex items-center gap-3 sm:gap-4 shrink-0">
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
        <!-- Cloche : changements des AUTRES appareils (db-change, émis par Rust
             même hors écran / en arrière-plan). Dropdown avec badge non-lus. -->
        <div class="relative" ref="bellWrap">
          <button @click="toggleBell" class="relative p-2 rounded-xl hover:bg-bg-hover text-txt-dim transition-colors">
            <Bell class="w-5 h-5" />
            <span v-if="notif.unread > 0" class="absolute top-0.5 right-0.5 min-w-[16px] h-4 px-1 rounded-full bg-neon-red text-white text-[9px] font-bold flex items-center justify-center">{{ notif.unread > 9 ? '9+' : notif.unread }}</span>
            <span v-else class="absolute top-1 right-1 w-2 h-2 bg-neon-red/40 rounded-full"></span>
          </button>
          <transition enter-active-class="transition duration-150" enter-from-class="opacity-0 -translate-y-1" leave-active-class="transition duration-100" leave-to-class="opacity-0">
            <div v-if="bellOpen" class="absolute right-0 mt-2 w-80 max-w-[90vw] bg-bg-surface border border-white/10 rounded-xl shadow-xl z-50 overflow-hidden">
              <div class="flex items-center justify-between px-3 py-2 border-b border-white/5">
                <span class="font-gaming text-sm font-bold">Notifications</span>
                <button @click="notif.clear()" class="text-xs text-txt-dim hover:text-txt-base">Tout effacer</button>
              </div>
              <div class="max-h-72 overflow-auto">
                <div v-for="n in notif.items" :key="n.id" class="px-3 py-2 border-b border-white/5 hover:bg-bg-hover" :class="{ 'bg-neon-violet/5': !n.read }">
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-sm font-medium truncate">{{ n.title }}</span>
                    <span class="text-[10px] text-txt-dim shrink-0">{{ notifTime(n.ts) }}</span>
                  </div>
                  <p class="text-xs text-txt-dim truncate">{{ n.body }}</p>
                </div>
                <div v-if="notif.items.length === 0" class="px-3 py-6 text-center text-xs text-txt-dim">Aucune notification</div>
              </div>
            </div>
          </transition>
        </div>
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
import { useNotifStore } from '@/stores/notifications'

defineProps({ sidebarOpen: Boolean })
defineEmits(['toggleSidebar'])

const route = useRoute()
const auth = useAuthStore()
const sync = useSyncStore()
const notif = useNotifStore()
const bellOpen = ref(false)
const bellWrap = ref<HTMLElement | null>(null)

function toggleBell() {
  bellOpen.value = !bellOpen.value
  if (bellOpen.value) notif.markAllRead()
}

function onDocClick(e: MouseEvent) {
  if (bellWrap.value && !bellWrap.value.contains(e.target as Node)) bellOpen.value = false
}

function notifTime(ts: string) {
  try {
    return new Date(ts).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })
  } catch { return '' }
}

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
  notif.bind() // événements Rust db-change (cloche multi-appareils)
  document.addEventListener('click', onDocClick)
})
onUnmounted(() => { clearInterval(timer); clearInterval(syncTimer); document.removeEventListener('click', onDocClick) })

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
