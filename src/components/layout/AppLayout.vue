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

async function pollForChanges() {
  try {
    const { api } = await import('@/utils/api')
    const result = await api.get('/sync/poll')
    if (result && result.changes) {
      window.dispatchEvent(new CustomEvent('sync-poll', { detail: result }))
    }
  } catch {}
}

onMounted(() => {
  settings.init()
  window.addEventListener('resize', onResize)
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
