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

        <!-- BYPASS (secours) : entre sans mot de passe via la session admin
             locale (auth_bootstrap_admin). À utiliser quand le login normal
             est bloqué (cloud injoignable, compte inconnu de l'appareil…). -->
        <button type="button" @click="bypassLogin" :disabled="loginPending"
                class="w-full text-center text-xs text-txt-dim hover:text-neon-violet underline underline-offset-4 py-1">
          Entrer sans mot de passe (secours)
        </button>

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
import { api } from '@/utils/api'
import { isTauriRuntime } from '@/lib/tauriApi'
import { listen } from '@tauri-apps/api/event'

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
  // IMPORTANT : binder le listener des étapes DÈS le montage, même si Tauri
  // est déjà prêt (cas normal sur Android). Un early-return ici empêchait
  // l'affichage des étapes backend sur l'app Android.
  bindLoginSteps()
  if (backendReady.value) return
  // WebView Android : l'injection Tauri peut être retardée au boot ; on attend
  // qu'elle soit prête plutôt que d'afficher une erreur trompeuse.
  const { waitForTauri } = await import('@/lib/transport')
  backendReady.value = await waitForTauri(4000)
  bindLoginSteps() // 2e appel sans risque : la fonction est idempotente (listener déjà posé)
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
  // Termine visuellement l'étape précédente si elle était en cours.
  const prev = steps.value[steps.value.length - 1]
  if (prev && prev.status === 'active') prev.status = 'ok'
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
let loginStepsReady: Promise<void> = Promise.resolve()
function bindLoginSteps() {
  if (unlistenSteps || typeof window === 'undefined') return loginStepsReady
  loginStepsReady = listen('login-step', (e: any) => {
    const p = e?.payload || {}
    const step = String(p.step || '')
    const detail = String(p.detail || '')
    if (!detail) return
    if (step === 'error') failStep(detail)
    else if (step === 'success') pushStep(detail, 'ok')
    else pushStep(detail)
  }).then((un: any) => {
    unlistenSteps = un
  }).catch((e: any) => {
    console.warn('[LOGIN_EVENT_LISTENER_ERROR]', e)
    pushStep('Journal backend indisponible, vérification toujours en cours…')
  })
  return loginStepsReady
}

// ============ COMPTE À REBOURS 60s + VERDICT UNIQUE ============
const LOGIN_TIMEOUT_S = 20
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

/// Annonce le verdict UNE seule fois :
///  - réponse du backend (succès OU erreur définitive) -> verdict immédiat,
///    navigation incluse en cas de succès ;
///  - 0 atteint sans réponse -> "une minute écoulée" + formulaire déverrouillé
///    pour permettre une nouvelle tentative (le backend continue d'écouter :
///    s'il répond ensuite, la console et la navigation suivent).
function announceVerdict() {
  const fromCountdown = countdownActive.value
  stopCountdown()
  if (pendingOutcome === 'success') {
    success.value = true
    verdict.value = '✓ Connecté — ouverture de votre espace…'
    loginPending.value = false
    router.replace(homePath())
  } else if (pendingOutcome === 'fail') {
    success.value = false
    verdict.value = pendingError
    loginPending.value = false
  } else if (fromCountdown && remaining.value <= 0) {
    // Timeout sans réponse backend.
    success.value = false
    verdict.value = "Aucune réponse du backend (20s). Le moteur local n'a pas répondu à temps — réessayez."
    loginPending.value = false
  }
}

function friendlyError(e: any): string {
  const status = e?.status
  const raw = typeof e?.message === 'string' ? e.message : ''
  if (status === 401) return 'Identifiants incorrects'
  if (status === 429) return raw || 'Trop de tentatives, réessayez plus tard'
  if (status === 503 || status === 504) {
    // Timeout IPC : le backend n'a pas répondu à l'app dans le délai ->
    // message COMPRÉHENSIBLE au lieu du jargon technique.
    if (raw.includes('Timeout IPC')) {
      return 'Le serveur ne répond pas (délai dépassé). Vérifiez votre connexion internet et réessayez.'
    }
    // Message réseau générique UNIQUEMENT si le backend n'a pas fourni de cause
    // précise (ex: "Tauri non disponible", "Base temporairement verrouillée") :
    // on garde le message spécifique pour un diagnostic fiable.
    const msg = raw.trim()
    if (msg) return msg
    return 'Connexion au serveur impossible. Vérifiez internet et réessayez.'
  }
  if (status && status >= 500) return raw || 'Erreur serveur, réessayez dans un instant'
  return raw || 'Identifiants incorrects'
}

function homePath() {
  return auth.user?.role === 'admin' ? '/admin' : '/dashboard'
}

/// BYPASS : session admin locale (auth_bootstrap_admin), sans mot de passe.
/// Utilise le même transport (503 = backend pas prêt) et le même verdict.
async function bypassLogin() {
  if (loginPending.value) return
  loginPending.value = true
  error.value = ''
  success.value = false
  pushStep('Contournement : session admin locale…')
  try {
    const data = await api.post('/auth/bootstrap')
    await auth.setSessionFromBootstrap(data)
    pushStep('Session de secours créée ✓', 'ok')
    await router.replace(homePath())
  } catch (e: any) {
    const msg = friendlyError(e)
    failStep(msg)
    error.value = msg
  } finally {
    loginPending.value = false
  }
}

async function handleLogin() {
  if (loginPending.value) return
  if (!form.email || !form.password) { error.value = 'Saisissez votre email et votre mot de passe'; verdict.value = ''; return }
  loginPending.value = true
  error.value = ''
  success.value = false
  steps.value = []
  // Le listener est prêt AVANT l'invoke : aucune étape Rust ne doit être perdue.
  await loginStepsReady
  pushStep('Identifiants envoyés au backend…')
  pushStep('Appel IPC Tauri en cours…')
  startCountdown()
  try {
    await auth.login(form.email.trim().toLowerCase(), form.password)
    pushStep('Session créée ✓', 'ok')
    pendingOutcome = 'success'
    announceVerdict() // verdict immédiat + redirection
  } catch (e: any) {
    pendingError = friendlyError(e)
    pendingOutcome = 'fail'
    failStep(pendingError) // ligne rouge dans le journal
    announceVerdict() // erreur définitive -> verdict immédiat (pas d'attente jusqu'à 0)
  }
}
</script>
