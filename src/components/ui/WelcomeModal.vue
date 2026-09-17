<template>
  <Transition name="welcome-fade">
    <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-bg-dark/85 backdrop-blur-md">
      <div 
        class="relative w-full max-w-sm rounded-2xl bg-gradient-to-b from-bg-card to-bg-dark border border-neon-violet/40 p-6 shadow-2xl shadow-neon-violet/20 text-center overflow-hidden transform transition-all animate-welcome-scale"
      >
        <!-- Effet de halo néon arrière-plan -->
        <div class="absolute -top-16 -left-16 w-36 h-36 bg-neon-violet/30 rounded-full blur-3xl pointer-events-none"></div>
        <div class="absolute -bottom-16 -right-16 w-36 h-36 bg-neon-blue/25 rounded-full blur-3xl pointer-events-none"></div>

        <!-- Avatar / Icône gaming animée -->
        <div class="relative mx-auto mb-4 w-20 h-20 flex items-center justify-center">
          <div class="absolute inset-0 rounded-full bg-gradient-to-tr from-neon-violet to-neon-blue animate-pulse blur-md opacity-70"></div>
          <div class="relative w-18 h-18 rounded-full bg-bg-dark border-2 border-neon-violet flex items-center justify-center text-3xl font-gaming font-extrabold text-neon-blue shadow-inner">
            {{ userInitial }}
          </div>
        </div>

        <!-- Titre & Sous-titre -->
        <h2 class="font-gaming text-2xl font-bold tracking-wide text-txt mb-1 animate-welcome-slide">
          Bienvenue, <span class="text-transparent bg-clip-text bg-gradient-to-r from-neon-violet to-neon-blue">{{ userName }}</span> !
        </h2>
        <p class="text-sm text-txt-dim mb-4">
          Ravi de vous revoir sur <span class="font-semibold text-txt">Game Lounge</span>
        </p>

        <!-- Badge Rôle -->
        <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-semibold mb-6"
          :class="userRole === 'admin' ? 'bg-neon-violet/15 text-neon-violet border border-neon-violet/30' : 'bg-neon-blue/15 text-neon-blue border border-neon-blue/30'">
          <span class="w-2 h-2 rounded-full animate-ping" :class="userRole === 'admin' ? 'bg-neon-violet' : 'bg-neon-blue'"></span>
          {{ userRole === 'admin' ? 'Administrateur' : 'Employé' }}
        </div>

        <!-- Barre de chargement fluide -->
        <div class="w-full bg-bg-hover h-1.5 rounded-full overflow-hidden mb-5">
          <div class="h-full bg-gradient-to-r from-neon-violet via-neon-blue to-neon-violet animate-welcome-progress rounded-full"></div>
        </div>

        <!-- Bouton continuer direct -->
        <button 
          @click="$emit('continue')" 
          class="btn-neon-violet w-full py-2.5 text-sm font-semibold rounded-xl flex items-center justify-center gap-2 group transition-transform active:scale-95"
        >
          <span>Accéder à l'espace</span>
          <span class="transform transition-transform group-hover:translate-x-1">→</span>
        </button>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  open: boolean
  user?: { nom?: string; email?: string; role?: string } | null
}>()

defineEmits<{
  (e: 'continue'): void
}>()

const userName = computed(() => {
  if (props.user?.nom) return props.user.nom
  if (props.user?.email) return props.user.email.split('@')[0]
  return 'Gamer'
})

const userInitial = computed(() => {
  return userName.value.charAt(0).toUpperCase() || 'G'
})

const userRole = computed(() => {
  return props.user?.role || 'employe'
})
</script>

<style scoped>
@keyframes welcomeScale {
  0% {
    opacity: 0;
    transform: scale(0.92) translateY(12px);
  }
  100% {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

@keyframes welcomeSlide {
  0% {
    opacity: 0;
    transform: translateY(8px);
  }
  100% {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes welcomeProgress {
  0% {
    width: 0%;
  }
  100% {
    width: 100%;
  }
}

.animate-welcome-scale {
  animation: welcomeScale 0.35s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.animate-welcome-slide {
  animation: welcomeSlide 0.4s ease-out 0.1s both;
}

.animate-welcome-progress {
  animation: welcomeProgress 1.6s ease-in-out forwards;
}

.welcome-fade-enter-active,
.welcome-fade-leave-active {
  transition: opacity 0.3s ease;
}

.welcome-fade-enter-from,
.welcome-fade-leave-to {
  opacity: 0;
}
</style>
