<template>
  <div class="min-h-screen flex items-center justify-center bg-bg p-4">
    <div class="w-full max-w-md card space-y-5">
      <div class="text-center" @click="onLogoTap">
        <h1 class="font-gaming text-3xl font-bold text-txt">GAME LOUNGE</h1>
        <p class="text-txt-dim mt-2">Connexion</p>
        <p v-if="debugHint" class="text-[10px] text-amber-400 mt-1">Mode développeur activé</p>
      </div>
      <form @submit.prevent="handleLogin" class="space-y-4">
        <div>
          <label class="block text-sm text-txt-muted mb-1">Email</label>
          <input v-model="form.email" type="email" class="input-field w-full" required />
        </div>
        <div>
          <label class="block text-sm text-txt-muted mb-1">Mot de passe</label>
          <input v-model="form.password" type="password" class="input-field w-full" required />
        </div>
        <button type="submit" :disabled="loading" class="btn-neon-violet w-full">
          {{ loading ? 'Connexion...' : 'Se connecter' }}
        </button>
        <p v-if="success" class="text-sm text-neon-green text-center font-medium">Connexion réussie ✓</p>
        <p v-if="error" class="text-sm text-neon-red text-center">{{ error }}</p>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
// Login NON BLOQUANT (mission §2/§17) : l'authentification seule retourne vite
// (le Rust n'y fait AUCUNE synchronisation). C'est APRÈS le succès que l'on
// décide : première installation -> écran d'initialisation (progression réelle),
// base déjà initialisée -> dashboard immédiatement (le delta sync partira en
// arrière-plan tout seul via la boucle auto_sync du Rust).
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useSyncStore } from '@/stores/sync'

const router = useRouter()
const auth = useAuthStore()
const sync = useSyncStore()
const loading = ref(false)
const success = ref(false)
const error = ref('')

const form = reactive({
  email: 'admin@gamelounge.com',
  password: 'admin123',
})

// ACCÈS DÉBOGAGE (réparé) : 5 taps sur le logo -> gl_debug_bypass=1. Le lien
// "Développeur" apparaît alors dans la barre latérale APRÈS connexion (même en
// compte employé), donnant accès aux logs 1.log, diagnostic users et tests cloud.
const debugHint = ref(false)
let logoTaps = 0
let tapTimer: ReturnType<typeof setTimeout> | null = null
function onLogoTap() {
  logoTaps++
  if (tapTimer) clearTimeout(tapTimer)
  tapTimer = setTimeout(() => { logoTaps = 0 }, 1500)
  if (logoTaps >= 5) {
    logoTaps = 0
    try { localStorage.setItem('gl_debug_bypass', '1') } catch {}
    debugHint.value = true
  }
}

function dashboardPath() {
  return auth.user?.role === 'admin' ? '/admin' : '/dashboard'
}

async function handleLogin() {
  if (loading.value) return
  loading.value = true
  error.value = ''
  success.value = false
  try {
    await auth.login(form.email.trim().toLowerCase(), form.password.trim())
    success.value = true // "Connexion réussie ✓" (mission §17)

    // Première installation ? -> écran d'initialisation avec progression réelle.
    // Sinon -> dashboard immédiatement (mission §12).
    sync.bindListeners()
    const completed = await sync.fetchInitialStatus()
    if (completed === false) {
      await router.replace('/initialisation')
    } else {
      await router.replace(dashboardPath())
    }
  } catch (e: any) {
    error.value = e?.message || 'Identifiants incorrects'
  } finally {
    loading.value = false
  }
}
</script>
