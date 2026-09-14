<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex items-center gap-3">
      <Bug class="w-6 h-6 text-neon-violet" />
      <h3 class="font-gaming text-lg font-bold">Outils Développeur</h3>
      <span class="badge bg-amber-500/20 text-amber-400 border-amber-500/30">Debug uniquement</span>
    </div>
    <p class="text-sm text-txt-dim">Test du login et diagnostic auth (local + cloud). Toute opération passe par l'authentification réelle admin.</p>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <div class="card">
        <h4 class="font-bold mb-3 flex items-center gap-2"><LogIn class="w-4 h-4 text-neon-violet" /> Test Login</h4>
        <div class="space-y-3">
          <input v-model="testEmail" placeholder="Email administrateur" class="input-field w-full" />
          <input v-model="testPassword" placeholder="Mot de passe" type="password" class="input-field w-full" />
          <button @click="testLogin" :disabled="loadingLogin" class="btn-neon-violet w-full">
            {{ loadingLogin ? 'Test en cours...' : 'Tester login' }}
          </button>
          <div v-if="loginResult" class="p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap max-h-32 overflow-auto">{{ loginResult }}</div>
        </div>
      </div>

      <div class="card">
        <h4 class="font-bold mb-3 flex items-center gap-2"><KeyRound class="w-4 h-4 text-amber-400" /> Diagnostic auth</h4>
        <button @click="diagUsers" class="btn-neon-outline text-sm w-full">Diag users local+cloud</button>
        <p class="text-xs text-txt-dim mt-2">Compare les comptes de la base locale et du cloud Supabase (utile quand le login échoue). Les opérations sont journalisées côté backend.</p>
        <div v-if="diagResult" class="mt-3 p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap max-h-64 overflow-auto">{{ diagResult }}</div>
      </div>
    </div>

    <div class="card">
      <h4 class="font-bold mb-3 flex items-center gap-2"><FileText class="w-4 h-4" /> Logs Frontend</h4>
      <div class="flex gap-2 mb-3">
        <button @click="clearLogs" class="btn-neon-outline text-xs">Effacer</button>
        <button @click="copyLogs" class="btn-neon-outline text-xs">Copier</button>
        <span class="text-xs text-txt-dim ml-auto">{{ logs.length }} lignes</span>
      </div>
      <div class="bg-black/50 rounded-xl p-3 max-h-64 overflow-auto font-mono text-xs">
        <div v-for="(l, i) in logs" :key="i" :class="logColor(l)">{{ l }}</div>
        <div v-if="logs.length===0" class="text-txt-dim">Aucun log - les logs BOOT/TRANSPORT/TAURI/AUTH s'affichent ici</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { api } from '@/utils/api'
import { Bug, LogIn, FileText, KeyRound } from 'lucide-vue-next'

const testEmail = ref('')
const testPassword = ref('')
const loadingLogin = ref(false)
const loginResult = ref('')
const logs = ref<string[]>([])

function logColor(l: string) {
  if (l.includes('[BOOT]') || l.includes('[DASHBOARD_START]')) return 'text-neon-green'
  if (l.includes('[TAURI_INVOKE_ERROR]') || l.includes('[auth]') && l.includes('failed')) return 'text-neon-red'
  if (l.includes('[SUPABASE_SYNC]') || l.includes('[cloud]') || l.includes('[supabase]')) return 'text-neon-blue'
  if (l.includes('[DEBUG')) return 'text-amber-400'
  return 'text-txt-dim'
}

function captureLog(...args: any[]) {
  const msg = args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' ')
  const line = `[${new Date().toLocaleTimeString()}] ${msg}`
  logs.value.push(line)
  if (logs.value.length > 500) logs.value.shift()
}

let origLog: any, origWarn: any, origError: any
onMounted(() => {
  origLog = console.log
  origWarn = console.warn
  origError = console.error
  console.log = (...a: any[]) => { captureLog(...a); origLog(...a) }
  console.warn = (...a: any[]) => { captureLog('WARN', ...a); origWarn(...a) }
  console.error = (...a: any[]) => { captureLog('ERROR', ...a); origError(...a) }
  captureLog('[DEV] Outils développeur chargés')
})

onUnmounted(() => {
  console.log = origLog
  console.warn = origWarn
  console.error = origError
})

async function testLogin() {
  loadingLogin.value = true
  loginResult.value = ''
  try {
    const data = await api.post('/auth/login', { email: testEmail.value, password: testPassword.value })
    loginResult.value = `✅ Login OK via ${data.source || 'local'}:\n${JSON.stringify(data, null, 2)}`
  } catch (e: any) {
    loginResult.value = `❌ Login échoué (${e.status || ''}): ${e.message}\n${JSON.stringify(e.data || {}, null, 2)}`
  } finally { loadingLogin.value = false }
}

function adminToken() {
  return localStorage.getItem('gl_token') || undefined
}

const diagResult = ref('')

async function diagUsers() {
  diagResult.value = 'Diagnostic en cours...'
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const local = await invoke('auth_debug_info', { token: adminToken() })
    let supabasePart = ''
    try {
      const supa = await invoke('auth_debug_supabase_users', { token: adminToken() })
      supabasePart = `\n\n=== SUPABASE ===\n${JSON.stringify(supa, null, 2)}`
    } catch (e: any) {
      supabasePart = `\n\n=== SUPABASE === erreur: ${e?.message || e}`
    }
    diagResult.value = `=== LOCAL ===\n${JSON.stringify(local, null, 2)}${supabasePart}`
  } catch (e: any) {
    diagResult.value = `❌ Erreur diag: ${e?.message || e}`
  }
}

function clearLogs() { logs.value = [] }
function copyLogs() {
  const text = logs.value.join('\n')
  navigator.clipboard?.writeText(text).then(() => alert('Logs copiés')).catch(() => {})
}
</script>
