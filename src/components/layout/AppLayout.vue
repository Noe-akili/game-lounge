<template>
  <div class="flex h-full w-full max-w-full overflow-hidden">
    <!-- Desktop: permanent sidebar -->
    <div v-if="isDesktop" class="hidden lg:flex flex-col w-64 bg-bg-card border-r border-white/5 flex-shrink-0 h-full">
      <AppSidebarDesktop />
    </div>

    <!-- Mobile: modal sidebar -->
    <AppSidebar v-if="!isDesktop" :open="isSidebarOpen" @close="isSidebarOpen = false" />

    <div class="flex-1 flex flex-col min-h-0 min-w-0 w-full max-w-full overflow-hidden">
      <AppHeader :sidebar-open="isSidebarOpen" @toggle-sidebar="isSidebarOpen = !isSidebarOpen" class="flex-shrink-0" />
      <main class="flex-1 overflow-y-auto overflow-x-hidden p-4 lg:p-6 pb-20 lg:pb-6 w-full max-w-full min-w-0 relative">
        <!-- ALERTE SYNCHRONISATION (S3) : si la sync est active mais que des
             changements attendent depuis plus de 24 h, c'est une panne
             silencieuse (réseau, URL cloud, identifiants). Sans ce bandeau,
             l'utilisateur croit que tout est sauvegardé alors que rien ne
             quitte le téléphone. Visible par TOUS les rôles. -->
        <div
          v-if="syncAlert"
          class="mb-4 rounded-xl border border-amber-400/40 bg-amber-400/10 p-3 flex flex-col sm:flex-row sm:items-center gap-3"
        >
          <AlertTriangle class="w-5 h-5 text-amber-400 shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-amber-300">{{ syncAlertTitle }}</p>
            <p class="text-xs text-amber-200/80">
              {{ syncAlertText }}
            </p>
          </div>
          <button
            @click="retrySync"
            :disabled="retryingSync"
            class="btn-neon-outline shrink-0 flex items-center justify-center gap-2 text-xs"
          >
            <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': retryingSync }" />
            {{ retryingSync ? 'Envoi...' : 'Réessayer' }}
          </button>
        </div>
        <router-view v-slot="{ Component }">
          <transition name="slide-inner" mode="default">
            <component :is="Component" :key="$route.path" />
          </transition>
        </router-view>
      </main>
    </div>

    <BottomNav @create-session="showSessionModal = true" />
    <StartSessionModal :open="showSessionModal" @close="showSessionModal = false" @started="onSessionStarted" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import AppSidebarDesktop from '@/components/layout/AppSidebarDesktop.vue'
import AppHeader from '@/components/layout/AppHeader.vue'
import BottomNav from '@/components/layout/BottomNav.vue'
import StartSessionModal from '@/components/ui/StartSessionModal.vue'
import { useSettingsStore } from '@/stores/settings'
import { AlertTriangle, RefreshCw } from 'lucide-vue-next'

const isSidebarOpen = ref(false)
const showSessionModal = ref(false)
const router = useRouter()
const settings = useSettingsStore()

// --- Alerte de synchronisation en panne (v1.1) ---
// `stuck` est calculé par le backend : sync active + changements en attente
// depuis plus de 24 h. On évite ainsi l'écran « tout va bien » alors que les
// ventes ne remontent pas.
const syncAlert = ref<any>(null)
const retryingSync = ref(false)
let syncAlertInterval: ReturnType<typeof setInterval> | null = null

const syncAlertTitle = computed(() =>
  syncAlert.value?.cloudConfigured === false ? 'Adresse cloud non configurée' : 'Synchronisation en attente'
)

const syncAlertText = computed(() => {
  const a = syncAlert.value
  if (!a) return ''
  const n = a.pendingCount ?? 0
  // Cas le plus grave : aucune adresse cloud sur l'appareil -> rien ne peut
  // partir, même avec Internet. Message spécifique pour ne pas envoyer
  // l'utilisateur vérifier son WiFi inutilement.
  if (a.cloudConfigured === false) {
    return `${n} changement(s) ne peuvent pas être envoyés : aucune adresse cloud n'est configurée sur cet appareil. Demandez à l'administrateur de la renseigner (Paramètres → Synchronisation).`
  }
  const h = a.pendingHours ?? 0
  const jours = Math.floor(h / 24)
  const quand = jours >= 1 ? `depuis ${jours} jour(s)` : `depuis ${h} h`
  return `${n} changement(s) ne sont pas encore envoyés vers le cloud ${quand}. Vérifiez la connexion Internet ; si le problème persiste, demandez à l'administrateur de vérifier la configuration cloud.`
})

async function checkSyncState() {
  try {
    const { api } = await import('@/utils/api')
    const token = (typeof sessionStorage !== 'undefined' ? sessionStorage.getItem('gl_token') : null) || localStorage.getItem('gl_token')
    if (!token) { syncAlert.value = null; return }
    const state: any = await api.get('/sync/state')
    syncAlert.value = state?.stuck ? state : null
  } catch {
    // Hors ligne ou non authentifié : on n'affiche pas d'alerte (l'utilisateur
    // n'a pas forcément d'Internet, ce n'est pas une anomalie en soi).
  }
}

async function retrySync() {
  retryingSync.value = true
  try {
    const { api } = await import('@/utils/api')
    const res: any = await api.post('/sync/run')
    if (res?.started === false) {
      const { toast } = await import('vue-sonner')
      toast.info('Synchronisation déjà en cours...')
    }
    // Laisse le temps au moteur de travailler avant de réévaluer l'alerte.
    setTimeout(checkSyncState, 6000)
  } catch (e: any) {
    const { toast } = await import('vue-sonner')
    toast.error('Synchronisation impossible : ' + (e?.message || 'réseau indisponible'))
  } finally {
    retryingSync.value = false
  }
}

const isDesktop = ref(typeof window !== 'undefined' ? window.innerWidth >= 1024 : false)
let pollInterval: ReturnType<typeof setInterval> | null = null

function onSessionStarted(session?: unknown) {
  showSessionModal.value = false
  window.dispatchEvent(new CustomEvent('session-started', { detail: session }))
  router.push('/sessions')
}

function onResize() {
  const desktop = window.innerWidth >= 1024
  isDesktop.value = desktop
  if (desktop && isSidebarOpen.value) {
    isSidebarOpen.value = false
  }
}

async function pollForChanges() {
  try {
    const { api } = await import('@/utils/api')
    const result = await api.get('/sync/poll')
    if (result && result.changes) {
      window.dispatchEvent(new CustomEvent('sync-poll', { detail: result }))
      if (Object.keys(result.changes).length > 0) {
        window.dispatchEvent(new CustomEvent('app-data-refresh', { detail: result.changes }))
      }
    }
  } catch {}
}

onMounted(() => {
  try { settings.init() } catch (e) { console.warn('[AppLayout] settings.init failed', e) }
  window.addEventListener('resize', onResize)
  // Vraie première installation ? -> bascule sur /initialisation (progression
  // réelle de la 1ère sync). Vérification en ARRIÈRE-PLAN, après affichage de
  // l'accueil : un réseau indisponible n'envoie JAMAIS l'utilisateur sur
  // /initialisation (il reste sur l'accueil, données SQLite locales affichées).
  import('@/stores/auth').then(({ useAuthStore }) => {
    const auth = useAuthStore()
    if (!auth.isAuthenticated) return
    import('@/stores/sync').then(({ useSyncStore }) => {
      const sync = useSyncStore()
      sync.fetchInitialStatus().then((completed) => {
        if (completed === false) router.replace('/initialisation')
      })
    })
  })
  // Android back button : fermer sidebar/modal avant de quitter
  const onAndroidBack = () => {
    if (isSidebarOpen.value) { isSidebarOpen.value = false; return }
    if (showSessionModal.value) { showSessionModal.value = false; return }
  }
  window.addEventListener('android-back-pressed', onAndroidBack as any)

  // Écouteurs Tauri pour synchroniser tout le frontend en temps réel
  if (typeof window !== 'undefined') {
    const tauri = (window as any).__TAURI__?.event || (window as any).__TAURI_INTERNALS__?.event
    if (tauri && typeof tauri.listen === 'function') {
      try {
        tauri.listen('sync-completed', (event: any) => {
          window.dispatchEvent(new CustomEvent('sync-completed', { detail: event?.payload }))
          window.dispatchEvent(new CustomEvent('app-data-refresh', { detail: { all: true } }))
        })
        tauri.listen('db-change', (event: any) => {
          window.dispatchEvent(new CustomEvent('db-change', { detail: event?.payload }))
          if (event?.payload?.table) {
            window.dispatchEvent(new CustomEvent('app-data-refresh', { detail: { [event.payload.table]: 1 } }))
          }
        })
        tauri.listen('app-setting-changed', (event: any) => {
          window.dispatchEvent(new CustomEvent('app-setting-changed', { detail: event?.payload }))
        })
      } catch (err) {
        console.warn('[AppLayout] Tauri event listener setup:', err)
      }
    }
  }
  // Poll sync moins agressif sur Android (économise batterie, évite ANR sur low-end)
  const intervalMs = typeof navigator !== 'undefined' && /Android/i.test(navigator.userAgent) ? 15000 : 10000
  pollInterval = setInterval(() => {
    try {
      const token = (typeof sessionStorage !== 'undefined' ? sessionStorage.getItem('gl_token') : null) || localStorage.getItem('gl_token')
      if (token) pollForChanges()
    } catch {}
  }, intervalMs)
  // Nettoyage sur unmount
  ;(window as any).__gl_cleanup_back = () => window.removeEventListener('android-back-pressed', onAndroidBack as any)

  // Surveillance de la synchronisation : première vérification après
  // l'affichage (laisse le temps au pool cloud de se connecter), puis toutes
  // les 5 minutes. Vérifie aussi après chaque cycle de sync terminé.
  setTimeout(checkSyncState, 8000)
  syncAlertInterval = setInterval(checkSyncState, 5 * 60 * 1000)
  window.addEventListener('sync-completed', checkSyncState)
})
onUnmounted(() => {
  window.removeEventListener('resize', onResize)
  window.removeEventListener('sync-completed', checkSyncState)
  if (pollInterval) clearInterval(pollInterval)
  if (syncAlertInterval) clearInterval(syncAlertInterval)
  try { (window as any).__gl_cleanup_back?.() } catch {}
})
</script>
