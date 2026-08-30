<template>
  <div class="flex flex-col h-full w-full">
    <div class="p-5 border-b border-white/5 flex items-center gap-3">
      <div class="w-10 h-10 rounded-xl bg-neon-violet/20 flex items-center justify-center">
        <Gamepad2 class="w-5 h-5 text-neon-violet" />
      </div>
      <div>
        <h1 class="font-gaming text-lg font-bold text-txt">GAME LOUNGE</h1>
        <p class="text-[10px] text-txt-dim uppercase tracking-widest">Gestion</p>
      </div>
    </div>

    <nav class="flex-1 overflow-y-auto p-3 space-y-1">
      <template v-if="isAdmin">
        <span class="px-4 py-2 text-[10px] font-semibold text-neon-violet uppercase tracking-widest">Admin</span>
        <router-link to="/admin" class="sidebar-link" active-class="sidebar-link-active">
          <LayoutDashboard class="w-4 h-4" />
          <span>Tableau de bord</span>
        </router-link>

        <span class="px-4 py-2 mt-4 text-[10px] font-semibold text-txt-dim uppercase tracking-widest">Gestion</span>
        <router-link to="/sessions" class="sidebar-link" active-class="sidebar-link-active">
          <PlayCircle class="w-4 h-4" />
          <span>Sessions</span>
        </router-link>
        <router-link to="/admin/consoles" class="sidebar-link" active-class="sidebar-link-active">
          <Monitor class="w-4 h-4" />
          <span>Consoles</span>
        </router-link>
        <router-link to="/admin/jeux" class="sidebar-link" active-class="sidebar-link-active">
          <Joystick class="w-4 h-4" />
          <span>Jeux</span>
        </router-link>
        <router-link to="/admin/joueurs" class="sidebar-link" active-class="sidebar-link-active">
          <Users class="w-4 h-4" />
          <span>Joueurs</span>
        </router-link>
        <router-link to="/admin/tarifs" class="sidebar-link" active-class="sidebar-link-active">
          <DollarSign class="w-4 h-4" />
          <span>Tarifs</span>
        </router-link>
        <router-link to="/admin/jetons" class="sidebar-link" active-class="sidebar-link-active">
          <Coins class="w-4 h-4" />
          <span>Jetons</span>
        </router-link>
        <router-link to="/messages" class="sidebar-link" active-class="sidebar-link-active">
          <MessageSquare class="w-4 h-4" />
          <span>Messages</span>
        </router-link>

        <span class="px-4 py-2 mt-4 text-[10px] font-semibold text-txt-dim uppercase tracking-widest">Rapports</span>
        <router-link to="/admin/factures" class="sidebar-link" active-class="sidebar-link-active">
          <Receipt class="w-4 h-4" />
          <span>Factures</span>
        </router-link>
        <router-link to="/admin/rapports" class="sidebar-link" active-class="sidebar-link-active">
          <BarChart3 class="w-4 h-4" />
          <span>Rapports</span>
        </router-link>

        <span class="px-4 py-2 mt-4 text-[10px] font-semibold text-txt-dim uppercase tracking-widest">Paramètres</span>
        <router-link to="/admin/parametres" class="sidebar-link" active-class="sidebar-link-active">
          <Settings class="w-4 h-4" />
          <span>Paramètres</span>
        </router-link>
        <router-link to="/admin/utilisateurs" class="sidebar-link" active-class="sidebar-link-active">
          <UserCog class="w-4 h-4" />
          <span>Utilisateurs</span>
        </router-link>
      </template>

      <template v-else>
        <router-link to="/dashboard" class="sidebar-link" active-class="sidebar-link-active">
          <LayoutDashboard class="w-4 h-4" />
          <span>Tableau de bord</span>
        </router-link>
        <router-link to="/sessions" class="sidebar-link" active-class="sidebar-link-active">
          <PlayCircle class="w-4 h-4" />
          <span>Sessions</span>
        </router-link>
        <router-link to="/joueurs" class="sidebar-link" active-class="sidebar-link-active">
          <Users class="w-4 h-4" />
          <span>Joueurs</span>
        </router-link>
        <router-link to="/paiements" class="sidebar-link" active-class="sidebar-link-active">
          <CreditCard class="w-4 h-4" />
          <span>Paiements</span>
        </router-link>
        <span class="px-4 py-2 mt-4 text-[10px] font-semibold text-txt-dim uppercase tracking-widest">Autre</span>
        <router-link to="/jetons" class="sidebar-link" active-class="sidebar-link-active">
          <Coins class="w-4 h-4" />
          <span>Jetons</span>
        </router-link>
        <router-link to="/messages" class="sidebar-link" active-class="sidebar-link-active">
          <MessageSquare class="w-4 h-4" />
          <span>Messages</span>
        </router-link>
        <router-link to="/parametres" class="sidebar-link" active-class="sidebar-link-active">
          <Settings class="w-4 h-4" />
          <span>Paramètres</span>
        </router-link>
      </template>
    </nav>

    <div class="p-3 border-t border-white/5">
      <div class="flex items-center gap-3 p-3 rounded-xl bg-bg-surface">
        <div class="w-9 h-9 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold text-sm">
          {{ userInitials }}
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-medium truncate">{{ auth.user?.nom }}</p>
          <p class="text-[10px] text-txt-dim uppercase">{{ auth.user?.role === 'admin' ? 'Administrateur' : 'Employé' }}</p>
        </div>
        <button @click="handleLogout" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim hover:text-neon-red transition-colors">
          <LogOut class="w-4 h-4" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import {
  Gamepad2, LayoutDashboard, Monitor, Joystick, Users, DollarSign, Coins,
  Receipt, BarChart3, Settings, UserCog, LogOut, PlayCircle, CreditCard, MessageSquare
} from 'lucide-vue-next'

const auth = useAuthStore()
const router = useRouter()

const isAdmin = computed(() => auth.user?.role === 'admin')
const userInitials = computed(() => {
  const nom = auth.user?.nom || ''
  return nom.split(' ').map(w => w[0]).join('').toUpperCase().slice(0, 2)
})

function handleLogout() {
  auth.logout()
  router.push('/login')
}
</script>
