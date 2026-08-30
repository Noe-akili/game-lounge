<template>
  <header class="sticky top-0 z-30 bg-bg/80 backdrop-blur-xl border-b border-white/5">
    <div class="flex items-center justify-between px-4 lg:px-6 h-16">
      <div class="flex items-center gap-4">
        <button @click="$emit('toggleSidebar')" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim lg:hidden">
          <Menu v-if="!sidebarOpen" class="w-5 h-5" />
          <X v-else class="w-5 h-5" />
        </button>
        <div>
          <h2 class="font-gaming text-lg font-bold">{{ pageTitle }}</h2>
          <p class="text-xs text-txt-dim">Gestion des sessions en temps réel</p>
        </div>
      </div>

      <div class="flex items-center gap-4">
        <div class="text-right hidden sm:block">
          <p class="text-sm font-medium font-gaming">{{ currentTime }}</p>
          <p class="text-[10px] text-txt-dim">{{ currentDate }}</p>
        </div>
        <button class="relative p-2 rounded-xl hover:bg-bg-hover text-txt-dim transition-colors">
          <Bell class="w-5 h-5" />
          <span class="absolute top-1 right-1 w-2 h-2 bg-neon-red rounded-full"></span>
        </button>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import { Menu, X, Bell } from 'lucide-vue-next'

defineProps({ sidebarOpen: Boolean })
defineEmits(['toggleSidebar'])

const route = useRoute()
const now = ref(new Date())
let timer = null

onMounted(() => { timer = setInterval(() => { now.value = new Date() }, 1000) })
onUnmounted(() => clearInterval(timer))

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
    '/admin/joueurs': 'Gestion des joueurs',
    '/admin/tarifs': 'Paramétrage des tarifs',
    '/admin/jetons': 'Gestion des jetons',
    '/admin/factures': 'Factures',
    '/admin/rapports': 'Rapports & Statistiques',
    '/admin/parametres': 'Paramètres',
    '/admin/utilisateurs': 'Utilisateurs',
  }
  return map[route.path] || 'Game Lounge'
})
</script>
