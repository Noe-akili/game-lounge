<template>
  <div class="min-h-screen flex items-center justify-center bg-bg p-4 relative overflow-hidden">
    <div class="absolute inset-0 overflow-hidden">
      <div class="absolute top-1/4 left-1/4 w-96 h-96 bg-neon-violet/5 rounded-full blur-3xl"></div>
      <div class="absolute bottom-1/4 right-1/4 w-96 h-96 bg-neon-blue/5 rounded-full blur-3xl"></div>
    </div>

    <motion.div
      :initial="{ opacity: 0, y: 24 }"
      :animate="{ opacity: 1, y: 0 }"
      :transition="{ duration: 0.5, ease: 'easeOut' }"
      class="w-full max-w-md relative z-10"
    >
      <motion.div
        :initial="{ opacity: 0, scale: 0.8 }"
        :animate="{ opacity: 1, scale: 1 }"
        :transition="{ duration: 0.5, delay: 0.1, ease: 'easeOut' }"
        class="text-center mb-8"
      >
        <motion.div :animate="{ rotate: [0, -3, 3, 0] }" :transition="{ duration: 4, repeat: Infinity, ease: 'easeInOut' }" class="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-neon-violet/20 mb-4 shadow-neon-violet">
          <Gamepad2 class="w-10 h-10 text-neon-violet" />
        </motion.div>
        <h1 class="font-gaming text-4xl font-bold text-txt">GAME LOUNGE</h1>
        <p class="text-txt-dim mt-2">Connectez-vous à votre compte</p>
      </motion.div>

      <motion.form
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.4, delay: 0.2 }"
        @submit.prevent="handleLogin" class="card space-y-5">
        <div>
          <label class="block text-sm font-medium text-txt-muted mb-2">Email</label>
          <div class="relative">
            <Mail class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim" />
            <input v-model="form.email" type="email" placeholder="admin@gamelounge.com"
              class="input-field pl-10" required />
          </div>
        </div>

        <div>
          <label class="block text-sm font-medium text-txt-muted mb-2">Mot de passe</label>
          <div class="relative">
            <Lock class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim" />
            <input v-model="form.password" :type="showPass ? 'text' : 'password'"
              placeholder="••••••••" class="input-field pl-10 pr-10" required />
            <button type="button" @click="showPass = !showPass"
              class="absolute right-3 top-1/2 -translate-y-1/2 text-txt-dim hover:text-txt transition-colors">
              <EyeOff v-if="showPass" class="w-4 h-4" />
              <Eye v-else class="w-4 h-4" />
            </button>
          </div>
        </div>

        <label class="flex items-center gap-2 text-sm text-txt-muted cursor-pointer">
          <input type="checkbox" v-model="remember" class="w-4 h-4 rounded bg-bg-surface border-white/20 text-neon-violet focus:ring-neon-violet/50" />
          Se souvenir de moi
        </label>

        <button type="submit" :disabled="loading" class="btn-neon-violet w-full flex items-center justify-center gap-2">
          <Loader2 v-if="loading" class="w-5 h-5 animate-spin" />
          <LogIn v-else class="w-5 h-5" />
          <span>Se connecter</span>
        </button>

        <p v-if="error" class="text-center text-sm text-neon-red">{{ error }}</p>

        <!-- Mode diagnostic : uniquement sur appareil avec /storage/.../tarif -->
        <div v-if="debugAllowed" class="mt-4 space-y-2">
          <div class="relative flex items-center gap-2">
            <div class="flex-1 h-px bg-white/10"></div>
            <span class="text-xs text-txt-dim">ou diagnostic (cet appareil)</span>
            <div class="flex-1 h-px bg-white/10"></div>
          </div>
          <button type="button" @click="handleDebugBypass" :disabled="importing" class="w-full flex items-center justify-center gap-2 py-3 px-4 rounded-xl border border-amber-500/30 bg-amber-500/10 text-amber-400 hover:bg-amber-500/20 transition-colors text-sm font-medium disabled:opacity-50">
            <Loader2 v-if="importing" class="w-4 h-4 animate-spin" />
            <span v-else>🔓</span>
            <span>{{ importing ? 'Import tarifs...' : 'Mode diagnostic (cet appareil)' }}</span>
          </button>
          <p class="text-xs text-txt-dim text-center">Importe /storage/.../tarif → local + Neon</p>
        </div>
        <p v-else class="text-xs text-txt-dim text-center mt-3">Mode diagnostic non disponible (dossier tarif absent)</p>
      </motion.form>

      <p class="text-center text-xs text-txt-dim mt-6">© 2024 Game Lounge — Tous droits réservés</p>
    </motion.div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { motion } from 'motion-v'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { toast } from 'vue-sonner'
import { Gamepad2, Mail, Lock, Eye, EyeOff, LogIn, Loader2 } from 'lucide-vue-next'
import { isValidEmail, isValidPassword } from '@/utils/validators'

const router = useRouter()
const auth = useAuthStore()

const loading = ref(false)
const error = ref('')
const showPass = ref(false)
const remember = ref(false)
const debugAllowed = ref(false)
const importing = ref(false)

const form = reactive({
  email: 'admin@gamelounge.com',
  password: 'admin123',
})

onMounted(async () => {
  console.log('[LOGIN_VIEW] mounted, auth.isAuthenticated=', auth.isAuthenticated)
  console.log('[LOGIN_VIEW] userAgent=', navigator.userAgent)
  console.log('[LOGIN_VIEW] is Android 14?', /Android\s*14/.test(navigator.userAgent) || navigator.userAgent.includes('Android 14'))
  // Vérifie si debug autorisé sur cet appareil (présence /storage/.../tarif)
  try {
    const { isDebugAllowed } = await import('@/lib/debug')
    debugAllowed.value = await isDebugAllowed()
    console.log('[DEBUG_CHECK] allowed=', debugAllowed.value)
  } catch {
    debugAllowed.value = false
  }
  if (typeof window !== 'undefined' && window.location.search.includes('debug') && debugAllowed.value) {
    console.log('[DEBUG_BYPASS] auto bypass via ?debug')
    handleDebugBypass()
  }
})

async function handleDebugBypass() {
  console.log('[DEBUG_BYPASS] handleDebugBypass start')
  if (!debugAllowed.value) {
    // Double vérification côté Rust
    try {
      const { isDebugAllowed } = await import('@/lib/debug')
      const ok = await isDebugAllowed()
      if (!ok) {
        toast.error('Mode diagnostic non autorisé sur cet appareil')
        return
      }
      debugAllowed.value = true
    } catch {
      toast.error('Vérification appareil échouée')
      return
    }
  }
  try {
    auth.debugBypassLogin()
    toast.success('Mode diagnostic activé - import tarifs...')
    // Import données par défaut depuis /storage/.../tarif et sync Neon
    try {
      importing.value = true
      const { importDefaultTarifs } = await import('@/lib/debug')
      const res = await importDefaultTarifs()
      console.log('[IMPORT] success', res)
      toast.success(`${res.imported || 0} tarifs importés (local + Neon)`)
    } catch (e: any) {
      console.warn('[IMPORT] failed or no data', e?.message)
      // Pas bloquant, on continue vers dashboard
    } finally {
      importing.value = false
    }
    console.log('[DEBUG_BYPASS] navigating to /admin')
    await router.push('/admin')
    console.log('[DEBUG_BYPASS] navigation success')
  } catch (e: any) {
    console.error('[DEBUG_BYPASS] failed', e)
    toast.error('Bypass échoué: ' + (e.message || 'erreur'))
  }
}

async function handleLogin() {
  console.log('[LOGIN_SUBMIT] handleLogin start')
  // Anti double-clic (important sur APK où le scrypt prend 1-2s)
  if (loading.value) return
  if (!isValidEmail(form.email)) return toast.error('Email invalide')
  if (!isValidPassword(form.password)) return toast.error('Mot de passe invalide (min 6 caractères, au moins une lettre)')
  loading.value = true
  error.value = ''
  // Normalise les entrées (évite espace invisible sur clavier Android)
  const email = form.email.trim().toLowerCase()
  const password = form.password.trim()
  console.log('[AUTH_START] email=', email)
  try {
    await auth.login(email, password)
    console.log('[AUTH_SUCCESS] login done, role=', auth.user?.role)
    toast.success('Connexion réussie !')
    // Petit délai pour laisser le temps au toast avant navigation sur WebView lente
    await new Promise(r => setTimeout(r, 100))
    const target = auth.user?.role === 'admin' ? '/admin' : '/dashboard'
    console.log('[ROUTER_NAVIGATION] push to', target)
    await router.push(target)
    console.log('[ROUTER_NAVIGATION] push success')
  } catch (e: any) {
    // Extraction robuste du message (Tauri renvoie parfois {message,status} en JSON)
    let msg = e?.message || e?.data?.message || 'Identifiants incorrects'
    try {
      if (typeof msg === 'string' && msg.startsWith('{')) {
        const p = JSON.parse(msg)
        if (p.message) msg = p.message
      }
    } catch {}
    error.value = msg
    console.error('[login] failed', e)
    if (msg.includes('internet') || msg.includes('indisponible') || msg.includes('Failed to fetch') || msg.includes('Connexion requise')) {
      toast.error('Pas de connexion internet — connectez-vous pour la première fois')
    } else if (msg.includes('incorrects') || msg.includes('401') || msg.includes('Token')) {
      toast.error('Email ou mot de passe incorrect')
    } else if (msg.includes('Trop de tentatives') || msg.includes('429')) {
      toast.error('Trop de tentatives, réessayez dans quelques minutes')
    } else {
      toast.error(msg)
    }
  } finally {
    loading.value = false
  }
}
</script>
