<template>
  <div id="game-lounge-app" class="h-full bg-bg">
    <motion.div
      v-if="routeLoading"
      :initial="{ scaleX: 0 }"
      :animate="{ scaleX: 1 }"
      :transition="{ duration: 0.8, ease: 'easeInOut' }"
      class="fixed top-0 left-0 right-0 h-0.5 bg-neon-violet z-[60] origin-left shadow-neon-violet"
      style="transform-origin: left"
    />
    <motion.div
      :initial="{ opacity: 0, y: 24 }"
      :animate="{ opacity: 1, y: 0 }"
      :transition="{ duration: 0.5, ease: [0.22, 1, 0.36, 1] }"
      class="w-full min-h-0 flex-1 flex flex-col"
    >
      <router-view />
    </motion.div>
    <Toaster
      :position="isMobile ? 'top-center' : 'bottom-right'"
      :toast-options="{
        style: {
          background: '#12121a',
          border: '1px solid rgba(168, 85, 247, 0.3)',
          color: '#e2e8f0',
          borderRadius: '12px',
          padding: '12px 16px',
        },
        success: {
          iconTheme: { primary: '#22c55e', secondary: '#0a0a0f' }
        },
        error: {
          iconTheme: { primary: '#ef4444', secondary: '#0a0a0f' }
        }
      }"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { motion } from 'motion-v'
import { Toaster } from 'sonner'

const router = useRouter()
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
onMounted(() => {
  // hide initial loader after mount
  setTimeout(() => (routeLoading.value = false), 400)
})
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
