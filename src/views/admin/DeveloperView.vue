<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex items-center gap-3">
      <Bug class="w-6 h-6 text-neon-violet" />
      <h3 class="font-gaming text-lg font-bold">Outils Développeur</h3>
      <span class="badge bg-amber-500/20 text-amber-400 border-amber-500/30">Debug uniquement</span>
    </div>
    <p class="text-sm text-txt-dim">Testez le login, Neon et consultez les logs pour identifier les erreurs avec précision (Android 14).</p>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <div class="card">
        <h4 class="font-bold mb-3 flex items-center gap-2"><LogIn class="w-4 h-4 text-neon-violet" /> Test Login</h4>
        <div class="space-y-3">
          <input v-model="testEmail" placeholder="admin@gamelounge.com" class="input-field w-full" />
          <input v-model="testPassword" placeholder="admin123" type="password" class="input-field w-full" />
          <button @click="testLogin" :disabled="loadingLogin" class="btn-neon-violet w-full">
            {{ loadingLogin ? 'Test en cours...' : 'Tester login' }}
          </button>
          <div v-if="loginResult" class="p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap max-h-32 overflow-auto">{{ loginResult }}</div>
        </div>
      </div>

      <div class="card">
        <h4 class="font-bold mb-3 flex items-center gap-2"><Cloud class="w-4 h-4 text-neon-blue" /> Test Neon</h4>
        <div class="space-y-3">
          <button @click="testNeon" :disabled="loadingNeon" class="btn-neon-violet w-full">
            {{ loadingNeon ? 'Test en cours...' : 'Tester Neon (pull/push)' }}
          </button>
          <button @click="testSync" :disabled="loadingNeon" class="btn-neon-outline w-full">Lancer sync_run</button>
          <div v-if="neonResult" class="p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap max-h-32 overflow-auto">{{ neonResult }}</div>
        </div>
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

    <div class="card">
      <h4 class="font-bold mb-3 flex items-center gap-2"><Server class="w-4 h-4" /> Logs Backend (Rust) - 1.log</h4>
      <p class="text-xs text-txt-dim mb-2">Tous les logs Rust capturés dans <code>1.log</code> (aucun logcat nécessaire)</p>
      <div class="flex gap-2">
        <button @click="fetchBackendLogs" class="btn-neon-outline flex-1">Statut Neon</button>
        <button @click="fetchRustLogs" class="btn-neon-violet flex-1">Voir 1.log (Rust)</button>
      </div>
      <div v-if="backendStatus" class="mt-3 p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap max-h-32 overflow-auto">{{ backendStatus }}</div>
      <div v-if="rustLogs" class="mt-3 p-3 rounded-xl bg-black/50 text-xs font-mono whitespace-pre-wrap max-h-64 overflow-auto">{{ rustLogs }}</div>
      <button @click="testNeonConnection" class="btn-neon-outline w-full mt-3">Tester connexion Neon (où ça bloque)</button>
      <div v-if="neonTestResult" class="mt-2 p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap">{{ neonTestResult }}</div>
    </div>

    <div class="card">
      <h4 class="font-bold mb-3">Actions rapides</h4>
      <div class="grid grid-cols-2 gap-2">
        <button @click="clearData" class="btn-neon-outline text-sm">Clear local DB</button>
        <button @click="importTarifs" class="btn-neon-violet text-sm">Importer tarifs PDF</button>
      </div>
    </div>

    <div class="card">
      <h4 class="font-bold mb-3 flex items-center gap-2"><KeyRound class="w-4 h-4 text-amber-400" /> Diagnostic login</h4>
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-2">
        <button @click="diagUsers" class="btn-neon-outline text-sm">Diag users local+Neon</button>
        <button @click="resetAdmin" class="btn-neon-outline text-sm text-amber-400">Reset admin (admin123)</button>
        <button @click="refreshRustLogsAfter = true; fetchRustLogs()" class="btn-neon-outline text-sm">Voir logs auth</button>
      </div>
      <div v-if="diagResult" class="mt-3 p-3 rounded-xl bg-bg-surface text-xs font-mono whitespace-pre-wrap max-h-64 overflow-auto">{{ diagResult }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { api } from '@/utils/api'
import { Bug, LogIn, Cloud, FileText, Server, KeyRound } from 'lucide-vue-next'

const testEmail = ref('admin@gamelounge.com')
const testPassword = ref('admin123')
const loadingLogin = ref(false)
const loginResult = ref('')
const loadingNeon = ref(false)
const neonResult = ref('')
const logs = ref<string[]>([])
const backendStatus = ref('')
const rustLogs = ref('')
const neonTestResult = ref('')

function logColor(l: string) {
  if (l.includes('[BOOT]') || l.includes('[DASHBOARD_START]')) return 'text-neon-green'
  if (l.includes('[TAURI_INVOKE_ERROR]') || l.includes('[auth]') && l.includes('failed')) return 'text-neon-red'
  if (l.includes('[NEON_SYNC]') || l.includes('[neon]')) return 'text-neon-blue'
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
  // Test initial
  fetchBackendLogs()
  // Capture BOOT logs déjà présents
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

async function testNeon() {
  loadingNeon.value = true
  neonResult.value = ''
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke('neon_status')
    neonResult.value = `neon_status:\n${JSON.stringify(status, null, 2)}`
    // Teste aussi debug_is_allowed
    const debug = await invoke('debug_is_allowed')
    neonResult.value += `\n\ndebug_is_allowed: ${debug}`
  } catch (e: any) {
    neonResult.value = `❌ Neon test échoué: ${e.message || String(e)}`
  } finally { loadingNeon.value = false }
}

async function testSync() {
  loadingNeon.value = true
  neonResult.value = ''
  try {
    const res = await api.post('/sync/run')
    if (res.started === false) { neonResult.value = `⏳ Sync déjà en cours`; return }
    // Sync en arrière-plan : on poll jusqu'au résultat final
    let last: any = null
    for (let i = 0; i < 80; i++) {
      await new Promise(r => setTimeout(r, 1500))
      try { last = (await api.get('/sync/poll')).last_sync } catch {}
      if (last && last.running !== true) break
    }
    neonResult.value = `✅ sync_run (arrière-plan):\n${JSON.stringify(last || res, null, 2)}`
  } catch (e: any) {
    neonResult.value = `❌ sync_run échoué: ${e.message}`
  } finally { loadingNeon.value = false }
}

async function fetchBackendLogs() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const s = await invoke('neon_status')
    backendStatus.value = JSON.stringify(s, null, 2)
  } catch (e: any) {
    backendStatus.value = `Erreur: ${e.message}`
  }
}
async function fetchRustLogs() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    rustLogs.value = await invoke('get_rust_logs')
    if (!rustLogs.value) rustLogs.value = await invoke('get_memory_logs').then((a: string[]) => a.join('\n'))
  } catch (e: any) {
    rustLogs.value = `Erreur get_rust_logs: ${e.message}`
  }
}
async function testNeonConnection() {
  neonTestResult.value = 'Test en cours...'
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke('test_neon_connection')
    neonTestResult.value = JSON.stringify(res, null, 2)
    // Rafraîchit aussi les logs Rust
    fetchRustLogs()
  } catch (e: any) {
    neonTestResult.value = `❌ Erreur: ${e.message}`
  }
}

const diagResult = ref('')
const refreshRustLogsAfter = false

async function diagUsers() {
  diagResult.value = 'Diagnostic en cours...'
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const local = await invoke('auth_debug_info')
    let neonPart = ''
    try {
      const neon = await invoke('auth_debug_neon_users')
      neonPart = `\n\n=== NEON ===\n${JSON.stringify(neon, null, 2)}`
    } catch (e: any) {
      neonPart = `\n\n=== NEON === erreur: ${e?.message || e}`
    }
    diagResult.value = `=== LOCAL ===\n${JSON.stringify(local, null, 2)}${neonPart}`
  } catch (e: any) {
    diagResult.value = `❌ Erreur diag: ${e?.message || e}`
  }
}

async function resetAdmin() {
  if (!confirm('Réinitialiser admin@gamelounge.com avec le mot de passe admin123 ?')) return
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke('debug_reset_admin')
    diagResult.value = `✅ ${JSON.stringify(res, null, 2)}\n\nTeste maintenant admin@gamelounge.com / admin123`
  } catch (e: any) {
    diagResult.value = `❌ Erreur reset: ${e?.message || e}`
  }
}

function clearLogs() { logs.value = [] }
function copyLogs() {
  const text = logs.value.join('\n')
  navigator.clipboard?.writeText(text).then(() => alert('Logs copiés')).catch(() => {})
}
async function clearData() {
  if (!confirm('Vider localStorage et secureStore ?')) return
  localStorage.clear()
  try {
    const { Store } = await import('@tauri-apps/plugin-store')
    const store = await Store.load('secure.dat')
    await store.clear()
    await store.save()
  } catch {}
  alert('Données vidées, redémarrage...')
  location.reload()
}
async function importTarifs() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke('import_default_tarifs')
    alert(`Import: ${JSON.stringify(res)}`)
  } catch (e: any) { alert(`Import échoué: ${e.message}`) }
}
</script>
