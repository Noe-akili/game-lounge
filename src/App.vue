<template>
  <div id="game-lounge-app" class="h-full overflow-hidden bg-bg">
    <!-- Barre de progression de chargement d'onglet : SOUS le header (safe-top
         = barre de statut Android + h-20 du header), et après la sidebar
         desktop (lg:left-64) -->
    <motion.div
      v-if="routeLoading"
      :initial="{ scaleX: 0 }"
      :animate="{ scaleX: 1 }"
      :transition="{ duration: 0.8, ease: 'easeInOut' }"
      class="fixed left-0 right-0 lg:left-64 h-0.5 bg-neon-violet z-[60] origin-left shadow-neon-violet safe-top header-offset"
      style="transform-origin: left"
    />
    <motion.div
      :initial="{ opacity: 0, y: 24 }"
      :animate="{ opacity: 1, y: 0 }"
      :transition="{ duration: 0.5, ease: [0.22, 1, 0.36, 1] }"
      class="w-full h-full min-h-0 flex-1 flex flex-col overflow-hidden"
    >
      <router-view />
    </motion.div>
    <Toaster
      :position="isMobile ? 'top-center' : 'bottom-right'"
      theme="dark"
      :toast-options="{
        style: {
          background: '#12121a',
          border: '1px solid rgba(168, 85, 247, 0.3)',
          color: '#e2e8f0',
          borderRadius: '12px',
          padding: '12px 16px',
        },
        className: 'toast-gaming'
      }"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { motion } from 'motion-v'
import { Toaster } from 'vue-sonner'
import { useAuthStore } from '@/stores/auth'
import { useSyncStore } from '@/stores/sync'
import { useNotifStore } from '@/stores/notifications'

const router = useRouter()
const auth = useAuthStore()
const routeLoading = ref(false)
const isMobile = typeof window !== 'undefined' && window.innerWidth < 640

let timeout: ReturnType<typeof setTimeout> | null = null
router.beforeEach(() => {
  routeLoading.value = true
  if (timeout) clearTimeout(timeout)
})
router.afterEach(() => {
  timeout = setTimeout(() => (routeLoading.value = false), 300)
})
onMounted(async () => {
  setTimeout(() => (routeLoading.value = false), 400)
})

// ============================================================
// SÉQUENCE DE DÉMARRAGE STRICTE (règle absolue) :
//   1. Démarrage app (main.ts) — rien d'autre
//   2. AUTH uniquement (store auth + guard routeur, lecture locale)
//   3. Pas de session -> LoginView affichée, AUCUN processus métier
//   4. Session valide -> authentifié
//   5. SEULEMENT APRÈS authentification (écran d'accueil affiché) :
//      listeners sync/notifications + tout processus métier.
//
// LoginView est donc TOTALEMENT indépendant du métier : tant que
// isAuthenticated est false, rien ci-dessous ne s'exécute.
// ============================================================
let businessStarted = false
function startBusinessProcesses() {
  if (businessStarted) return
  businessStarted = true
  // RÈGLE ABSOLUE : lève d'abord le verrou métier côté Rust — la sync
  // automatique, le watcher de sessions et les notifications natives ne
  // démarrent QU'À PARTIR DE CET INSTANT (accueil affiché).
  auth.businessReady()
  // Événements Rust (sync-progress, db-change pour la cloche) : posés au
  // niveau App pour rester actifs sur TOUTES les vues — mais APRÈS auth.
  useSyncStore().bindListeners()
  useNotifStore().bind()
}

watch(
  () => auth.isAuthenticated,
  (ok) => {
    if (ok) {
      startBusinessProcesses()
    } else {
      // Déconnexion / session expirée : on arrête et purge le métier.
      auth.businessSuspend()
      businessStarted = false
      useSyncStore().reset()
      useNotifStore().clear()
    }
  },
  { immediate: true }
)
</script>

<style>
.slide-inner-enter-active, .slide-inner-leave-active {
  transition: transform 0.36s cubic-bezier(0.22, 1, 0.36, 1), opacity 0.3s ease;
  width: 100%;
  will-change: transform, opacity;
}
.slide-inner-enter-active {
  position: relative;
  z-index: 1;
}
.slide-inner-leave-active {
  position: absolute;
  top: 1rem;
  left: 1rem;
  right: 1rem;
  z-index: 0;
}
@media (min-width: 1024px) {
  .slide-inner-leave-active {
    top: 1.5rem;
    left: 1.5rem;
    right: 1.5rem;
  }
}
.slide-inner-enter-from {
  transform: translateX(32px);
  opacity: 0;
}
.slide-inner-leave-to {
  transform: translateX(-16px);
  opacity: 0;
}
@media (max-width: 768px) {
  .slide-inner-enter-from {
    transform: translateX(100%);
  }
  .slide-inner-leave-to {
    transform: translateX(-24%);
  }
}
</style>
