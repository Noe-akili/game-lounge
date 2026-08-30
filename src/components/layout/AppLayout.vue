<template>
  <div v-if="serverReady || !isCapacitor" class="flex h-full w-full max-w-full overflow-hidden">
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
  <div v-else class="flex items-center justify-center h-full bg-bg">
    <div class="text-center space-y-4">
      <div class="w-16 h-16 mx-auto rounded-2xl bg-neon-violet/20 flex items-center justify-center animate-pulse">
        <svg class="w-8 h-8 text-neon-violet" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/></svg>
      </div>
      <h2 class="font-gaming text-xl font-bold text-neon-violet">Game Lounge</h2>
      <p class="text-txt-dim text-sm">Démarrage du serveur...</p>
      <div class="w-48 h-1 bg-bg-surface rounded-full mx-auto overflow-hidden">
        <div class="h-full bg-gradient-to-r from-neon-violet to-neon-blue rounded-full animate-shimmer" style="background-size:200% 100%"></div>
      </div>
    </div>
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
import { api } from '@/utils/api'

const isSidebarOpen = ref(false)
const showSessionModal = ref(false)
const router = useRouter()
const settings = useSettingsStore()

const isDesktop = ref(typeof window !== 'undefined' ? window.innerWidth >= 1024 : false)
const isCapacitor = typeof window !== 'undefined' && !!(window as any).Capacitor
const serverReady = ref(!isCapacitor)
let pollInterval: ReturnType<typeof setInterval> | null = null

function onSessionStarted() {
  showSessionModal.value = false
  router.push('/sessions')
}

function onResize() {
  const desktop = window.innerWidth >= 1024
  isDesktop.value = desktop
  if (desktop && isSidebarOpen.value) {
    isSidebarOpen.value = false
  }
}

async function startEmbeddedServer() {
  if (!isCapacitor) { serverReady.value = true; return }
  try {
    const { Capacitor } = await import('@capacitor/core')
    const NodeJs = (Capacitor as any).Plugins?.NodeJs
    if (NodeJs) {
      const result = await NodeJs.start()
      serverReady.value = true
      console.log('Serveur démarré:', result)
    } else {
      serverReady.value = true
    }
  } catch (e) {
    console.error('Échec démarrage serveur:', e)
    // Réessayer après 3 secondes
    setTimeout(() => startEmbeddedServer(), 3000)
  }
}

// Real-time polling: fetch changes from Neon every 10s
async function pollForChanges() {
  try {
    const result = await api.get('/sync/poll')
    if (result && result.changes) {
      // Dispatch custom event so views can refresh
      window.dispatchEvent(new CustomEvent('sync-poll', { detail: result }))
    }
  } catch {}
}

onMounted(() => {
  settings.init()
  window.addEventListener('resize', onResize)
  startEmbeddedServer()
  // Start polling every 10 seconds when logged in
  pollInterval = setInterval(() => {
    const token = localStorage.getItem('gl_token')
    if (token) pollForChanges()
  }, 10000)
})
onUnmounted(() => {
  window.removeEventListener('resize', onResize)
  if (pollInterval) clearInterval(pollInterval)
})
</script>
