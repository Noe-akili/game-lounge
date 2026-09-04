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
      </motion.form>

      <p class="text-center text-xs text-txt-dim mt-6">© 2024 Game Lounge — Tous droits réservés</p>
    </motion.div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
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

const form = reactive({
  email: 'admin@gamelounge.com',
  password: 'admin123',
})

async function handleLogin() {
  if (!isValidEmail(form.email)) return toast.error('Email invalide')
  if (!isValidPassword(form.password)) return toast.error('Mot de passe invalide (min 6 caractères, au moins une lettre)')
  loading.value = true
  error.value = ''
  try {
    await auth.login(form.email, form.password)
    toast.success('Connexion réussie !')
    router.push(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
  } catch (e: any) {
    const msg = e.message || 'Identifiants incorrects'
    error.value = msg
    if (msg.includes('internet') || msg.includes('indisponible') || msg.includes('Failed to fetch') || msg.includes('Connexion requise')) {
      toast.error('Pas de connexion internet — connectez-vous pour la première fois')
    } else if (msg.includes('incorrects')) {
      toast.error('Email ou mot de passe incorrect')
    } else {
      toast.error(msg)
    }
  } finally {
    loading.value = false
  }
}
</script>
