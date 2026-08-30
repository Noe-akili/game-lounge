<template>
  <div v-if="!isDesktop" class="lg:hidden">
    <motion.nav
      :initial="{ y: 40, opacity: 0 }"
      :animate="{ y: 0, opacity: 1 }"
      :transition="{ duration: 0.3, ease: 'easeOut' }"
      class="fixed bottom-0 left-0 right-0 z-40 bg-bg-card/95 backdrop-blur-xl border-t border-white/5 safe-bottom"
    >
      <div class="flex items-center justify-around h-16 px-2 max-w-lg mx-auto">
        <template v-if="isAdmin">
          <router-link to="/sessions"
            class="flex flex-col items-center justify-center gap-0.5 w-10 py-1 rounded-xl transition-colors"
            :class="isActive('/sessions') ? 'text-neon-violet' : 'text-txt-dim'">
            <PlayCircle class="w-5 h-5" />
            <span class="text-[8px] font-medium">Sessions</span>
          </router-link>
          <router-link to="/admin/consoles"
            class="flex flex-col items-center justify-center gap-0.5 w-10 py-1 rounded-xl transition-colors"
            :class="isActive('/admin/consoles') ? 'text-neon-violet' : 'text-txt-dim'">
            <Monitor class="w-5 h-5" />
            <span class="text-[8px] font-medium">Consoles</span>
          </router-link>
          <motion.button
            :whileTap="{ scale: 0.9 }"
            :whileHover="{ scale: 1.05 }"
            @click="$emit('createSession')"
            class="flex items-center justify-center w-12 h-12 -mt-4 rounded-full bg-neon-violet shadow-neon-violet text-white shrink-0"
          >
            <Plus class="w-6 h-6" />
          </motion.button>
          <router-link to="/admin/joueurs"
            class="flex flex-col items-center justify-center gap-0.5 w-10 py-1 rounded-xl transition-colors"
            :class="isActive('/admin/joueurs') ? 'text-neon-violet' : 'text-txt-dim'">
            <Users class="w-5 h-5" />
            <span class="text-[8px] font-medium">Joueurs</span>
          </router-link>
          <router-link to="/messages"
            class="flex flex-col items-center justify-center gap-0.5 w-10 py-1 rounded-xl transition-colors"
            :class="isActive('/messages') ? 'text-neon-violet' : 'text-txt-dim'">
            <MessageSquare class="w-5 h-5" />
            <span class="text-[8px] font-medium">Messages</span>
          </router-link>
        </template>
        <template v-else>
          <router-link to="/dashboard"
            class="flex flex-col items-center justify-center gap-0.5 w-14 py-1 rounded-xl transition-colors"
            :class="isActive('/dashboard') ? 'text-neon-violet' : 'text-txt-dim'">
            <Home class="w-5 h-5" />
            <span class="text-[9px] font-medium">Accueil</span>
          </router-link>
          <router-link to="/sessions"
            class="flex flex-col items-center justify-center gap-0.5 w-14 py-1 rounded-xl transition-colors"
            :class="isActive('/sessions') ? 'text-neon-violet' : 'text-txt-dim'">
            <PlayCircle class="w-5 h-5" />
            <span class="text-[9px] font-medium">Sessions</span>
          </router-link>
          <motion.button
            :whileTap="{ scale: 0.9 }"
            :whileHover="{ scale: 1.05 }"
            @click="$emit('createSession')"
            class="flex items-center justify-center w-12 h-12 -mt-6 rounded-full bg-neon-violet shadow-neon-violet text-white"
          >
            <Plus class="w-6 h-6" />
          </motion.button>
          <router-link to="/joueurs"
            class="flex flex-col items-center justify-center gap-0.5 w-14 py-1 rounded-xl transition-colors"
            :class="isActive('/joueurs') ? 'text-neon-violet' : 'text-txt-dim'">
            <Users class="w-5 h-5" />
            <span class="text-[9px] font-medium">Joueurs</span>
          </router-link>
          <router-link to="/paiements"
            class="flex flex-col items-center justify-center gap-0.5 w-14 py-1 rounded-xl transition-colors"
            :class="isActive('/paiements') ? 'text-neon-violet' : 'text-txt-dim'">
            <CreditCard class="w-5 h-5" />
            <span class="text-[9px] font-medium">Paiements</span>
          </router-link>
        </template>
      </div>
    </motion.nav>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { motion } from 'motion-v'
import { useRoute } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { Home, Monitor, Users, PlayCircle, CreditCard, Plus, MessageSquare } from 'lucide-vue-next'

defineEmits(['createSession'])

const route = useRoute()
const auth = useAuthStore()
const isAdmin = computed(() => auth.user?.role === 'admin')

const isDesktop = ref(typeof window !== 'undefined' ? window.innerWidth >= 1024 : false)

function onResize() {
  isDesktop.value = window.innerWidth >= 1024
}

onMounted(() => window.addEventListener('resize', onResize))
onUnmounted(() => window.removeEventListener('resize', onResize))

function isActive(path) {
  if (path === '/admin') return route.path === '/admin'
  return route.path === path || route.path.startsWith(path + '/')
}
</script>

<style scoped>
.safe-bottom {
  padding-bottom: env(safe-area-inset-bottom, 0px);
}
</style>
