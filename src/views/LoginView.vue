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
          <input v-model.trim="form.email" type="email" inputmode="email" autocomplete="username" class="input-field w-full" :disabled="loginPending" required autofocus />
        </div>
        <div>
          <label class="block text-sm text-txt-muted mb-1">Mot de passe</label>
          <div class="relative">
            <input v-model="form.password" :type="showPassword ? 'text' : 'password'" autocomplete="current-password" class="input-field w-full pr-12" :disabled="loginPending" required />
            <button type="button" @click="showPassword = !showPassword" class="absolute inset-y-0 right-0 px-3 text-txt-dim hover:text-txt" :aria-label="showPassword ? 'Masquer le mot de passe' : 'Afficher le mot de passe'">
              {{ showPassword ? 'Masquer' : 'Afficher' }}
            </button>
          </div>
        </div>
        <button type="submit" :disabled="loginPending" class="btn-neon-violet w-full flex items-center justify-center gap-2">
          <Loader2 v-if="loginPending" class="w-4 h-4 animate-spin" />
          {{ loginPending ? 'Connexion...' : 'Se connecter' }}
        </button>

        <!-- Compte à rebours (lancé dès que le backend reçoit email+mot de passe) -->
        <div v-if="countdownActive && !verdict" class="flex items-center justify-center gap-2 text-sm text-txt-muted">
          <Loader2 class="w-4 h-4 animate-spin text-neon-violet" />
          Vérification en cours… réponse dans <span class="font-mono font-bold text-neon-violet">{{ countdownDisplay }}</span>
        </div>

        <!-- Verdict final : affiché à la réponse OU à 0 (jamais avant) -->
        <div v-if="verdict" class="text-sm text-center font-medium" :class="verdictClass">{{ verdict }}</div>

        <!-- Console des étapes : montre OÙ le login en est / bloque -->
        <div v-if="steps.length" class="rounded-lg border border-white/10 bg-black/40 p-3 font-mono text-[11px] leading-5">
          <div class="text-txt-dim mb-1 flex items-center justify-between">
            <span>— Journal de connexion —</span>
            <button v-if="!loginPending" type="button" @click="clearConsole" class="text-txt-dim hover:text-txt text-[10px]">effacer</button>
          </div>
          <div ref="consoleEl" class="max-h-40 overflow-y-auto space-y-0.5">
            <div v-for="(s, i) in steps" :key="i" class="flex gap-2">
              <span class="text-txt-dim shrink-0">{{ s.t }}</span>
              <span class="shrink-0" :class="s.status === 'fail' ? 'text-neon-red' : s.status === 'ok' ? 'text-neon-green' : 'text-neon-violet'">
                {{ s.status === 'fail' ? '✗' : s.status === 'ok' ? '✓' : '•' }}
              </span>
              <span :class="s.status === 'fail' ? 'text-neon-red' : 'text-txt-muted'">{{ s.label }}</span>
            </div>
          </div>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
// Écran de CONNEXION — TOTALEMENT INDÉPENDANT du métier (règle absolue).
//
// Ici : UNIQUEMENT l'authentification (email + mot de passe -> token/user).
// AUCUN appel métier. Tout cela est lancé APRÈS, une fois l'écran d'accueil
// affiché (App.vue + AppLayout).
//
// Transparence utilisateur :
//  - Console d'étapes : le backend Rust émet des événements "login-step" à
//    chaque phase (identifiants reçus -> serveur joint -> compte trouvé ->
//    mot de passe vérifié -> session). La console les affiche en temps réel,
//    ce qui permet de voir OÙ ça bloque (réseau, compte, mot de passe…).
//  - Compte à rebours 60s : lancé dès la soumission. TANT QUE le compte
//    tourne, aucun verdict (ni succès ni échec). À la réponse du backend OU
//    à zéro, un verdict unique et clair est annoncé à l'utilisateur.
import { ref, reactive, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { Loader2 } from 'lucide-vue-next'
import { useAuthStore } from '@/stores/auth'
import { useSettingsStore } from '@/stores/settings'
import { isTauriRuntime } from '@/lib/tauriApi'

const router = useRouter()
const auth = useAuthStore()
const settings = useSettingsStore()
const success = ref(false)
const error = ref('')
const showPassword = ref(false)

// Un login est "en cours" tant que la promesse tourne ET que le verdict n'a
// pas été annoncé (le verdict à 0 n'interrompt pas la promesse : le backend
// peut encore répondre, la console continue de montrer la suite).
const loginPending = ref(false)

const form = reactive({
  email: '',
  password: '',
})

// État de connexion du backend (Tauri/SQLite prêt ?) — nécessaire à l'AUTH
// elle-même (l'appel de login passe par l'IPC), affiché sur l'écran.
const backendReady = ref(isTauriRuntime())

onMounted(async () => {
  if (backendReady.value) return
  // WebView Android : l'injection Tauri peut être retardée au boot ; on attend
  // qu'elle soit prête plutôt que d'afficher une erreur trompeuse.
  const { waitForTauri } = await import('@/lib/transport')
  backendReady.value = await waitForTauri(4000)
  bindLoginSteps()
})

// ================= CONSOLE DES ÉTAPES =================
type StepStatus = 'active' | 'ok' | 'fail'
type Step = { label: string; status: StepStatus; t: string }
const steps = ref<Step[]>([])
const consoleEl = ref<HTMLElement | null>(null)

function nowLabel(): string {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

function pushStep(label: string, status: StepStatus = 'active') {
  // Termine visuellement l'étape précédente (succès si elle n'a pas échoué).
  const prev = steps.value[steps.value.length - 1]
  if (prev && prev.status === 'active') prev.status = status === 'fail' ? 'ok' : 'ok'
  steps.value.push({ label, status, t: nowLabel() })
  if (steps.value.length > 30) steps.value.shift()
  nextTick(() => { consoleEl.value?.scrollTo({ top: consoleEl.value.scrollHeight }) })
}

function failStep(label: string) {
  const prev = steps.value[steps.value.length - 1]
  if (prev && prev.status === 'active') prev.status = 'ok'
  steps.value.push({ label, status: 'fail', t: nowLabel() })
  if (steps.value.length > 30) steps.value.shift()
  nextTick(() => { consoleEl.value?.scrollTo({ top: consoleEl.value.scrollHeight }) })
}

function clearConsole() {
  steps.value = []
}

// Étapes émises par le backend Rust (commande auth_login) via l'événement
// "login-step" : on voit en direct où la vérification en est.
let unlistenSteps: (() => void) | null = null
function bindLoginSteps() {
  if (unlistenSteps || typeof window === 'undefined') return
  const listen = (window as any).__TAURI__?.event?.listen
  if (typeof listen !== 'function') return
  listen('login-step', (e: any) => {
    const p = e?.payload || {}
    const step = String(p.step || '')
    const detail = String(p.detail || '')
    if (!detail) return
    if (step === 'error') failStep(detail)
    else if (step === 'success') pushStep(detail, 'ok')
    else pushStep(detail)
  }).then((un: any) => { unlistenSteps = un })
}

// ============ COMPTE À REBOURS 60s + VERDICT UNIQUE ============
const LOGIN_TIMEOUT_S = 60
const countdownActive = ref(false)
const remaining = ref(LOGIN_TIMEOUT_S)
const verdict = ref('')
let countdownTimer: ReturnType<typeof setInterval> | null = null
// Résultat reçu du backend mais retenue jusqu'à la fin du compte à rebours.
let pendingOutcome: 'success' | 'fail' | null = null
let pendingError = ''

const countdownDisplay = computed(() => {
  const s = Math.max(0, remaining.value)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
})

const verdictClass = computed(() =>
  success.value ? 'text-neon-green' : 'text-neon-red'
)

function startCountdown() {
  stopCountdown()
  remaining.value = LOGIN_TIMEOUT_S
  countdownActive.value = true
  verdict.value = ''
  pendingOutcome = null
  pendingError = ''
  countdownTimer = setInterval(() => {
    remaining.value -= 1
    if (remaining.value <= 0) announceVerdict()
  }, 1000)
}

function stopCountdown() {
  if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null }
  countdownActive.value = false
}

/// Annonce le verdict UNE seule fois : à la réponse du backend si elle arrive
/// avant 0, sinon à 0 ("une minute écoulée"). Le résultat du backend qui
/// arrive après le verdict est quand même appliqué (session/erreur).
function announceVerdict() {
  stopCountdown()
  if (pendingOutcome === 'success') {
    success.value = true
    verdict.value = '✓ Connecté — ouverture de votre espace…'
    loginPending.value = false
  } else if (pendingOutcome === 'fail') {
    success.value = false
    verdict.value = pendingError
    loginPending.value = false
  } else if (remaining.value <= 0) {
    // 60s écoulées sans réponse du backend : on l'annonce clairement,
    // MAIS on continue d'écouter (le backend peut encore répondre —
    // la console montrera la suite).
    success.value = false
    verdict.value = '⏱ Une minute écoulée sans réponse du serveur. Vérifiez votre connexion internet et réessayez.'
    loginPending.value = false
  }
}

function friendlyError(e: any): string {
  const status = e?.status
  if (status === 401) return 'Identifiants incorrects'
  if (status === 429) return e?.message || 'Trop de tentatives, réessayez plus tard'
  if (status === 503 || status === 504) {
    // Message réseau générique UNIQUEMENT si le backend n'a pas fourni de cause
    // précise (ex: "Tauri non disponible", "Base temporairement verrouillée",
    // "Timeout IPC") : on garde le message spécifique pour un diagnostic fiable.
    const msg = typeof e?.message === 'string' ? e.message.trim() : ''
    if (msg) return msg
    return 'Connexion au serveur impossible. Vérifiez internet et réessayez.'
  }
  if (status && status >= 500) return e?.message || 'Erreur serveur, réessayez dans un instant'
  return e?.message || 'Identifiants incorrects'
}

function homePath() {
  return auth.user?.role === 'admin' ? '/admin' : '/dashboard'
}

async function handleLogin() {
  if (loginPending.value) return
  if (!form.email || !form.password) { error.value = 'Saisissez votre email et votre mot de passe'; verdict.value = ''; return }
  loginPending.value = true
  error.value = ''
  success.value = false
  steps.value = []
  // Règle demandée : le compte à rebours démarre dès que le backend reçoit
  // l'email + le mot de passe (c'est-à-dire dès la soumission).
  pushStep('Identifiants envoyés au backend…')
  startCountdown()
  try {
    await auth.login(form.email.trim().toLowerCase(), form.password)
    // Succès : verdict immédiat si le compte est fini, sinon retenu jusqu'à 0.
    pushStep('Session créée ✓', 'ok')
    pendingOutcome = 'success'
    if (!countdownActive.value || remaining.value <= 0) announceVerdict()
  } catch (e: any) {
    pendingError = friendlyError(e)
    pendingOutcome = 'fail'
    if (!countdownActive.value || remaining.value <= 0) announceVerdict()
    else if (e?.status === 401) {
      // Refus clair du backend : inutile de faire attendre l'utilisateur
      // jusqu'à 0 pour un verdict certain. (Verdict immédiat.)
      announceVerdict()
    }
  }
}
</script>
