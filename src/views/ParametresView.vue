<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <h3 class="font-gaming text-lg font-bold">Paramètres</h3>
    <div v-if="loading" class="card">
      <Loader variant="neon" size="lg" text="Chargement..." />
    </div>
    <div v-else class="space-y-4">
            <!-- AFFICHAGE & PERSONNALISATION -->
      <div class="card">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
          <Palette class="w-5 h-5 text-neon-blue" />
          Affichage & Thème
        </h4>

        <div class="space-y-4">
          <div>
            <label class="text-xs text-txt-dim mb-2 block">Thème</label>
            <div class="grid grid-cols-2 gap-2">
              <button @click="settings.setTheme('dark')" class="p-3 rounded-xl border text-left text-xs transition-all flex items-center gap-2" :class="settings.themeMode === 'dark' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10'">
                <Moon class="w-4 h-4 text-neon-violet" /> Sombre
              </button>
              <button @click="settings.setTheme('light')" class="p-3 rounded-xl border text-left text-xs transition-all flex items-center gap-2" :class="settings.themeMode === 'light' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10'">
                <Sun class="w-4 h-4 text-neon-yellow" /> Clair
              </button>
            </div>
          </div>

          <div>
            <label class="text-xs text-txt-dim mb-2 block">Fond d'écran</label>
            <div class="grid grid-cols-3 gap-2">
              <button @click="settings.setBgMode('default')" class="p-2.5 rounded-xl border text-center text-xs transition-all" :class="settings.bgMode === 'default' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10'">
                Classique
              </button>
              <button @click="settings.setBgMode('stars')" class="p-2.5 rounded-xl border text-center text-xs transition-all" :class="settings.bgMode === 'stars' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10'">
                Étoiles ✨
              </button>
              <button @click="fileInputRef?.click()" class="p-2.5 rounded-xl border text-center text-xs transition-all" :class="settings.bgMode === 'custom' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10'">
                Photo 🖼️
              </button>
            </div>
            <div v-if="settings.bgCustomImage" class="mt-2 flex items-center justify-between text-xs text-txt-dim">
              <span>Photo personnalisée active</span>
              <button @click="settings.removeCustomBg()" class="text-neon-red hover:underline">Retirer</button>
            </div>
            <input ref="fileInputRef" type="file" accept="image/*" class="hidden" @change="onPhotoSelected" />
          </div>
        </div>
      </div>

      <div class="card">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
          <RefreshCw class="w-5 h-5 text-neon-blue" :class="{ 'animate-spin': syncing }" />
          Synchronisation des données
        </h4>
        <p class="text-xs text-txt-dim mb-4">Synchroniser immédiatement les données locales avec Supabase.</p>
        <button @click="handleSync" :disabled="syncing" class="btn-neon-blue w-full flex items-center justify-center gap-2">
          <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': syncing }" />
          {{ syncing ? 'Synchronisation en cours...' : 'Lancer la synchronisation' }}
        </button>
      </div>

      <div class="card">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
          <KeyRound class="w-5 h-5 text-neon-violet" />
          Modifier mon mot de passe
        </h4>
        <form @submit.prevent="handleChangePassword" class="space-y-3">
          <div>
            <label class="block text-xs text-txt-dim mb-1">Nouveau mot de passe</label>
            <input v-model="newPassword" type="password" placeholder="Min 6 caractères..." class="input-field w-full" required />
          </div>
          <div>
            <label class="block text-xs text-txt-dim mb-1">Confirmer le nouveau mot de passe</label>
            <input v-model="confirmPassword" type="password" placeholder="Répéter le mot de passe..." class="input-field w-full" required />
          </div>
          <button type="submit" :disabled="savingPassword" class="btn-neon-violet w-full flex items-center justify-center gap-2">
            <Check class="w-4 h-4" />
            {{ savingPassword ? 'Mise à jour...' : 'Enregistrer le mot de passe' }}
          </button>
        </form>
      </div>

      <div class="card">
        <h4 class="font-gaming font-bold mb-4">Préférences</h4>
        <div class="space-y-4">
          <label class="flex items-center justify-between gap-4 p-3 bg-bg-surface rounded-xl">
            <div>
              <p class="font-medium">Notifications</p>
              <p class="text-xs text-txt-dim">Recevoir les notifications de session</p>
            </div>
            <input type="checkbox" v-model="prefs.notifications" class="w-10 h-6 rounded-full appearance-none bg-white/10 checked:bg-neon-violet relative before:absolute before:w-4 before:bg-white before:rounded-full before:top-1 before:left-1 checked:before:translate-x-4 before:transition-all" />
          </label>
          <label class="flex items-center justify-between gap-4 p-3 bg-bg-surface rounded-xl">
            <div>
              <p class="font-medium">Sons</p>
              <p class="text-xs text-txt-dim">Activer les sons de notification</p>
            </div>
            <input type="checkbox" v-model="prefs.sounds" class="w-10 h-6 rounded-full appearance-none bg-white/10 checked:bg-neon-violet relative before:absolute before:w-4 before:bg-white before:rounded-full before:top-1 before:left-1 checked:before:translate-x-4 before:transition-all" />
          </label>
        </div>
      </div>

      <div class="card">
        <h4 class="font-gaming font-bold mb-4">Compte</h4>
        <div class="space-y-3">
          <div class="flex items-center gap-3 p-3 bg-bg-surface rounded-xl">
            <div class="w-10 h-10 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold">{{ userInitials }}</div>
            <div>
              <p class="font-medium">{{ auth.user?.nom }}</p>
              <p class="text-xs text-txt-dim">{{ auth.user?.email }} · {{ auth.user?.role }}</p>
            </div>
          </div>
          <button @click="handleLogout" class="btn-neon-outline w-full flex items-center justify-center gap-2">
            <LogOut class="w-4 h-4" /> Déconnexion
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import Loader from '@/components/ui/Loader.vue'
import { RefreshCw, KeyRound, Check, LogOut } from 'lucide-vue-next'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'

const auth = useAuthStore()
const router = useRouter()
const loading = ref(true)
const prefs = reactive({ notifications: true, sounds: true })

const syncing = ref(false)
const savingPassword = ref(false)

const settings = useSettingsStore()
const fileInputRef = ref<HTMLInputElement | null>(null)

function onPhotoSelected(e: Event) {
  const target = e.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  const reader = new FileReader()
  reader.onload = (event) => {
    const rawData = event.target?.result as string
    const img = new Image()
    img.onload = () => {
      const canvas = document.createElement('canvas')
      let width = img.width
      let height = img.height
      const maxDim = 1600
      if (width > maxDim || height > maxDim) {
        if (width > height) {
          height = Math.round((height * maxDim) / width)
          width = maxDim
        } else {
          width = Math.round((width * maxDim) / height)
          height = maxDim
        }
      }
      canvas.width = width
      canvas.height = height
      const ctx = canvas.getContext('2d')
      if (ctx) {
        ctx.drawImage(img, 0, 0, width, height)
        settings.setCustomBg(canvas.toDataURL('image/jpeg', 0.82))
        toast.success('Photo personnalisée appliquée')
      }
    }
    img.src = rawData
  }
  reader.readAsDataURL(file)
  target.value = ''
}

const newPassword = ref('')
const confirmPassword = ref('')

const userInitials = computed(() => {
  const nom = auth.user?.nom || ''
  return nom.split(' ').map((w: string) => w[0]).join('').toUpperCase().slice(0, 2)
})

async function handleSync() {
  syncing.value = true
  try {
    await api.post('/sync/run')
    toast.success('Synchronisation terminée avec succès !')
  } catch (e: any) {
    toast.error('Erreur synchronisation: ' + (e.message || 'Échec'))
  } finally {
    syncing.value = false
  }
}

async function handleChangePassword() {
  if (!newPassword.value || newPassword.value.length < 6) {
    return toast.error('Le mot de passe doit comporter au moins 6 caractères')
  }
  if (newPassword.value !== confirmPassword.value) {
    return toast.error('Les mots de passe ne correspondent pas')
  }
  savingPassword.value = true
  try {
    if (!auth.user?.id) throw new Error('Utilisateur non identifié')
    // Le serveur vérifie en relisant la base : si password_change n'est pas à
    // true, on n'annonce PAS un succès.
    const res: any = await api.put(`/users/${auth.user.id}`, { password: newPassword.value })
    if (!res?.password_change) throw new Error("Le serveur n'a pas confirmé l'enregistrement")
    toast.success('Mot de passe modifié — utilisez-le à la prochaine connexion')
    newPassword.value = ''
    confirmPassword.value = ''
  } catch (e: any) {
    toast.error(`Échec${e?.status ? ' (' + e.status + ')' : ''} : ${e?.message || 'mot de passe non modifié'}`)
  } finally {
    savingPassword.value = false
  }
}

function handleLogout() {
  auth.logout()
  router.push('/login')
}

onMounted(() => setTimeout(() => (loading.value = false), 400))
</script>
