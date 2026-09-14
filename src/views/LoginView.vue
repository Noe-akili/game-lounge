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
          <input v-model.trim="form.email" type="email" inputmode="email" autocomplete="username" class="input-field w-full" :disabled="loading" required autofocus />
        </div>
        <div>
          <label class="block text-sm text-txt-muted mb-1">Mot de passe</label>
          <div class="relative">
            <input v-model="form.password" :type="showPassword ? 'text' : 'password'" autocomplete="current-password" class="input-field w-full pr-12" :disabled="loading" required />
            <button type="button" @click="showPassword = !showPassword" class="absolute inset-y-0 right-0 px-3 text-txt-dim hover:text-txt" :aria-label="showPassword ? 'Masquer le mot de passe' : 'Afficher le mot de passe'">
              {{ showPassword ? 'Masquer' : 'Afficher' }}
            </button>
          </div>
        </div>
        <button type="submit" :disabled="loading" class="btn-neon-violet w-full flex items-center justify-center gap-2">
          <Loader2 v-if="loading" class="w-4 h-4 animate-spin" />
          {{ loading ? 'Connexion...' : 'Se connecter' }}
        </button>
        <p v-if="success" class="text-sm text-neon-green text-center font-medium">Connexion réussie ✓</p>
        <p v-if="error" class="text-sm text-neon-red text-center">{{ error }}</p>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
// Écran de CONNEXION (obligatoire).
//
// Flux après succès (mission §2) :
//   login OK -> token/user sauvegardés (store auth = source unique)
//            -> SQLite déjà chargée (les vues lisent l'API = SQLite locale)
//            -> première installation (initial_sync_completed = false)
//               ? écran d'initialisation (progression réelle de la 1ère sync)
//               : dashboard immédiat — la sync Supabase tourne en arrière-plan
//                 (boucle auto_sync Rust, événements bindés au niveau App).
//
// L'utilisateur ne voit JAMAIS "token missing" : le store refuse toute session
// sans token et les erreurs techniques sont traduites en messages clairs.
import { ref, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Loader2 } from 'lucide-vue-next'
import { useAuthStore } from '@/stores/auth'
import { useSyncStore } from '@/stores/sync'
import { useSettingsStore } from '@/stores/settings'
import { isTauriRuntime } from '@/lib/tauriApi'

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

// État de connexion du backend (Tauri/SQLite prêt ?) — affiché sur l'écran.
const backendReady = ref(isTauriRuntime())

onMounted(async () => {
  if (backendReady.value) return
  // WebView Android : l'injection Tauri peut être retardée au boot ; on attend
  // qu'elle soit prête plutôt que d'afficher une erreur trompeuse.
  const { waitForTauri } = await import('@/lib/transport')
  backendReady.value = await waitForTauri(4000)
})

function friendlyError(e: any): string {
  const status = e?.status
  if (status === 401) return 'Identifiants incorrects'
  if (status === 429) return e?.message || 'Trop de tentatives, réessayez plus tard'
  if (status === 503 || status === 504) return 'Connexion au serveur impossible. Vérifiez internet et réessayez.'
  if (status && status >= 500) return 'Erreur serveur, réessayez dans un instant'
  return e?.message || 'Identifiants incorrects'
}

async function handleLogin() {
  if (loading.value) return
  if (!form.email || !form.password) { error.value = 'Saisissez votre email et votre mot de passe'; return }
  loading.value = true
  error.value = ''
  success.value = false
  try {
    await auth.login(form.email.trim().toLowerCase(), form.password)
    success.value = true // "Connexion réussie ✓"

    // Première installation ? -> écran d'initialisation avec progression réelle.
    // Sinon -> dashboard immédiatement. Le réseau ne peut PAS bloquer : en cas
    // d'échec du sondage on va au dashboard (les données SQLite s'affichent,
    // la sync reprendra en arrière-plan).
    sync.bindListeners()
    const completed = await sync.fetchInitialStatus().catch(() => null)
    if (completed === false) {
      await router.replace('/initialisation')
    } else {
      await router.replace('/admin')
    }
  } catch (e: any) {
    error.value = friendlyError(e)
  } finally {
    loading.value = false
  }
}
</script>
