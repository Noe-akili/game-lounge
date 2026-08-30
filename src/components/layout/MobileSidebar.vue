<template>
  <Transition name="slide">
    <div v-if="open" class="fixed inset-0 z-50 lg:hidden">
      <div class="absolute inset-0 bg-black/60" @click="$emit('close')"></div>
      <aside class="absolute left-0 top-0 bottom-0 w-64 bg-bg-card border-r border-white/5 flex flex-col">
        <div class="p-5 border-b border-white/5 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-xl bg-neon-violet/20 flex items-center justify-center">
              <Gamepad2 class="w-5 h-5 text-neon-violet" />
            </div>
            <div>
              <h1 class="font-gaming text-lg font-bold">GAME LOUNGE</h1>
              <p class="text-[10px] text-txt-dim uppercase tracking-widest">Gestion</p>
            </div>
          </div>
          <button @click="$emit('close')" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim">
            <X class="w-5 h-5" />
          </button>
        </div>

        <nav class="flex-1 overflow-y-auto p-3 space-y-1">
          <router-link v-for="item in menuItems" :key="item.to" :to="item.to"
            class="sidebar-link" active-class="sidebar-link-active" @click="$emit('close')">
            <component :is="item.icon" class="w-4 h-4" />
            <span>{{ item.label }}</span>
          </router-link>
        </nav>

        <div class="p-3 border-t border-white/5 space-y-2">
          <button @click="toggleTheme" class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-bg-hover text-txt-dim transition-colors">
            <Moon v-if="settings.themeMode === 'dark'" class="w-4 h-4 text-neon-blue" />
            <Sun v-else class="w-4 h-4 text-neon-yellow" />
            <span class="text-sm">{{ settings.themeMode === 'dark' ? 'Mode clair' : 'Mode sombre' }}</span>
          </button>
          <div class="flex items-center gap-3 p-3 rounded-xl bg-bg-surface">
            <div class="w-9 h-9 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold text-sm">
              {{ userInitials }}
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium truncate">{{ auth.user?.nom }}</p>
              <p class="text-[10px] text-txt-dim uppercase">{{ auth.user?.role === 'admin' ? 'Admin' : 'Employé' }}</p>
            </div>
            <button @click="handleLogout" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim hover:text-neon-red transition-colors">
              <LogOut class="w-4 h-4" />
            </button>
          </div>
        </div>
      </aside>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSettingsStore } from '@/stores/settings'
import {
  Gamepad2, X, LayoutDashboard, PlayCircle, Users, CreditCard, Coins,
  MessageSquare, Settings, LogOut, Monitor, Joystick, Receipt, BarChart3, DollarSign, UserCog, Moon, Sun
} from 'lucide-vue-next'

const props = defineProps({ open: Boolean })
defineEmits(['close'])

const auth = useAuthStore()
const router = useRouter()
const settings = useSettingsStore()

const menuItems = computed(() => {
  if (auth.user?.role === 'admin') {
    return [
      { to: '/admin', label: 'Tableau de bord', icon: LayoutDashboard },
      { to: '/admin/consoles', label: 'Consoles', icon: Monitor },
      { to: '/admin/jeux', label: 'Jeux', icon: Joystick },
      { to: '/joueurs', label: 'Joueurs', icon: Users },
      { to: '/admin/tarifs', label: 'Tarifs', icon: DollarSign },
      { to: '/paiements', label: 'Factures', icon: Receipt },
      { to: '/admin/rapports', label: 'Rapports', icon: BarChart3 },
      { to: '/admin/parametres', label: 'Paramètres', icon: Settings },
    ]
  }
  return [
    { to: '/dashboard', label: 'Tableau de bord', icon: LayoutDashboard },
    { to: '/sessions', label: 'Sessions', icon: PlayCircle },
    { to: '/joueurs', label: 'Joueurs', icon: Users },
    { to: '/paiements', label: 'Paiements', icon: CreditCard },
    { to: '/jetons', label: 'Jetons', icon: Coins },
    { to: '/messages', label: 'Messages', icon: MessageSquare },
  ]
})

const userInitials = computed(() => {
  const nom = auth.user?.nom || ''
  return nom.split(' ').map(w => w[0]).join('').toUpperCase().slice(0, 2)
})

function handleLogout() {
  auth.logout()
  router.push('/login')
}

function toggleTheme() {
  settings.setTheme(settings.themeMode === 'dark' ? 'light' : 'dark')
}
</script>

<style scoped>
.slide-enter-active, .slide-leave-active { transition: opacity 0.2s ease; }
.slide-enter-from, .slide-leave-to { opacity: 0; }
</style>
