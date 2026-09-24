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
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import AppSidebarDesktop from '@/components/layout/AppSidebarDesktop.vue'
import AppHeader from '@/components/layout/AppHeader.vue'
import BottomNav from '@/components/layout/BottomNav.vue'
import StartSessionModal from '@/components/ui/StartSessionModal.vue'
import { useSettingsStore } from '@/stores/settings'

const isSidebarOpen = ref(false)
const showSessionModal = ref(false)
const router = useRouter()
const settings = useSettingsStore()

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
})
onUnmounted(() => {
  window.removeEventListener('resize', onResize)
  if (pollInterval) clearInterval(pollInterval)
  try { (window as any).__gl_cleanup_back?.() } catch {}
})
</script>
