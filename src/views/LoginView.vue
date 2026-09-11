<template>
  <div class="min-h-screen flex items-center justify-center bg-bg p-4">
    <div class="w-full max-w-md card space-y-5">
      <div class="text-center">
        <h1 class="font-gaming text-3xl font-bold text-txt">GAME LOUNGE</h1>
        <p class="text-txt-dim mt-2">Connexion</p>
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
        <p v-if="error" class="text-sm text-neon-red text-center">{{ error }}</p>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const auth = useAuthStore()
const loading = ref(false)
const error = ref('')

const form = reactive({
  email: 'admin@gamelounge.com',
  password: 'admin123',
})

async function handleLogin() {
  if (loading.value) return
  loading.value = true
  error.value = ''
  try {
    await auth.login(form.email.trim().toLowerCase(), form.password.trim())
    await router.push(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
  } catch (e: any) {
    error.value = e?.message || 'Identifiants incorrects'
  } finally {
    loading.value = false
  }
}
</script>
