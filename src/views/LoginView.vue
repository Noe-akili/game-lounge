<template>
  <div class="min-h-screen flex items-center justify-center bg-bg p-4">
    <div class="w-full max-w-md card space-y-5">
      <div class="text-center">
        <h1 class="font-gaming text-3xl font-bold text-txt">{{ settings.appName }}</h1>
        <p class="text-txt-dim mt-2">Connexion</p>
      </div>
      <form @submit.prevent="handleLogin" class="space-y-4">
        <div>
          <label class="block text-sm text-txt-muted mb-1">Email</label>
          <input v-model.trim="form.email" type="email" inputmode="email" autocomplete="username" class="input-field w-full" required autofocus />
        </div>
        <div>
          <label class="block text-sm text-txt-muted mb-1">Mot de passe</label>
          <div class="relative">
            <input v-model="form.password" :type="showPassword ? 'text' : 'password'" autocomplete="current-password" class="input-field w-full pr-12" required />
            <button type="button" @click="showPassword = !showPassword" class="absolute inset-y-0 right-0 px-3 text-txt-dim hover:text-txt" :aria-label="showPassword ? 'Masquer le mot de passe' : 'Afficher le mot de passe'">
              {{ showPassword ? 'Masquer' : 'Afficher' }}
            </button>
          </div>
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
import { useSettingsStore } from '@/stores/settings'

const router = useRouter()
const auth = useAuthStore()
const sync = useSyncStore()
const settings = useSettingsStore()
const loading = ref(false)
const success = ref(false)
const error = ref('')
const showPassword = ref(false)

const form = reactive({
  email: '',
  password: '',
})

function dashboardPath() {
  return auth.user?.role === 'admin' ? '/admin' : '/dashboard'
}

async function handleLogin() {
  if (loading.value) return
  if (!form.email || !form.password) { error.value = 'Saisissez votre email et votre mot de passe'; return }
  loading.value = true
  error.value = ''
  success.value = false
  try {
    await auth.login(form.email.trim().toLowerCase(), form.password)
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
