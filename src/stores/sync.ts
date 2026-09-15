// Store Pinia de synchronisation (mission §16) : reflète le SyncEngine RUST.
// La progression vient des événements émis par Rust (sync-progress etc.) —
// JAMAIS d'un setInterval simulé (mission §7). Le filet de secours est le
// polling /sync/poll existant (AppLayout) si les événements ne passent pas.
// @ts-nocheck
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export type SyncStatus =
  | 'idle'
  | 'initializing'
  | 'syncing'
  | 'completed'
  | 'error'
  | 'offline'
  | 'conflict'

type SyncProgressPayload = {
  phase?: string
  current?: number
  total?: number
  percent?: number
  processed?: number
  message?: string
  status?: string
}

// Phases de la sync initiale (miroir de SYNC_PHASES côté Rust, même ordre)
export const SYNC_PHASES = [
  'users', 'consoles', 'jeux', 'joueurs', 'tarifs', 'parametres_fidelite',
  'messages', 'sessions_jeu', 'jetons_transactions', 'factures', 'lignes_facture',
]

// Libellés lisibles par phase (cohérents avec le design Game Lounge)
export const PHASE_LABELS: Record<string, string> = {
  users: 'Utilisateurs',
  consoles: 'Consoles',
  jeux: 'Jeux',
  joueurs: 'Joueurs',
  tarifs: 'Tarifs',
  parametres_fidelite: 'Paramètres fidélité',
  messages: 'Messages',
  sessions_jeu: 'Sessions',
  jetons_transactions: 'Jetons',
  factures: 'Factures',
  lignes_facture: 'Lignes de facture',
  preparation: 'Préparation',
  finalisation: 'Finalisation',
  envoi: 'Envoi des changements',
  delta: 'Synchronisation des changements',
  termine: 'Terminé',
}

let listenersBound = false

export const useSyncStore = defineStore('sync', () => {
  const status = ref<SyncStatus>('idle')
  const phase = ref('')
  const current = ref(0)
  const total = ref(0)
  const percent = ref(0)
  const processed = ref(0)
  const message = ref('')
  const error = ref('')
  const initialSyncCompleted = ref(null) // null = inconnu (pas encore demandé)
  const tablesDone = ref([])

  const isBusy = computed(() => status.value === 'initializing' || status.value === 'syncing')

  function applyPayload(p) {
    if (p.phase !== undefined) phase.value = p.phase
    if (p.current !== undefined) current.value = p.current
    if (p.total !== undefined) total.value = p.total
    if (p.percent !== undefined) percent.value = p.percent
    if (p.processed !== undefined) processed.value = p.processed
    if (p.message !== undefined) message.value = p.message
  }

  function payloadOf(e) {
    return e?.payload ?? e?.detail ?? {}
  }

  function onProgress(e) {
    const p = payloadOf(e)
    // Rust émet aussi l'état final sur sync-progress : status completed -> terminé.
    if (p.status === 'completed') {
      status.value = 'completed'
      applyPayload(p)
      percent.value = 100
      return
    }
    if (p.status === 'error') return // passe par onError
    if (status.value !== 'error') {
      status.value = status.value === 'initializing' ? 'initializing' : 'syncing'
    }
    applyPayload(p)
  }

  function onStarted(e) {
    const p = payloadOf(e)
    status.value = 'initializing'
    error.value = ''
    applyPayload(p)
  }

  function onCompleted(e) {
    const p = payloadOf(e)
    applyPayload(p)
    status.value = 'completed'
    percent.value = 100
  }

  function onError(e) {
    const p = payloadOf(e)
    status.value = 'error'
    error.value = p.message || 'Erreur de synchronisation'
    applyPayload(p)
  }

  // Filet : le polling existant (/sync/poll dans AppLayout) met à jour running.
  function onPollDetail(d) {
    const last = d?.last_sync
    if (!last) return
    if (last.running === true && status.value !== 'error') {
      if (status.value === 'idle' || status.value === 'completed') status.value = 'syncing'
      if (last.message && !message.value) message.value = last.message
    }
  }

  function bindListeners() {
    if (listenersBound || typeof window === 'undefined') return
    const w = window
    // API événements Tauri v2 (withGlobalTauri = true dans tauri.conf.json)
    const listen = w.__TAURI__?.event?.listen
    if (typeof listen === 'function') {
      listen('sync-started', onStarted)
      listen('sync-progress', onProgress)
      listen('sync-completed', onCompleted)
      listen('sync-error', onError)
      listen('sync-conflict', onError)
      listenersBound = true
    }
    // Filet universel : détail du polling /sync/poll (CustomEvent 'sync-poll')
    window.addEventListener('sync-poll', (e) => onPollDetail(e?.detail))
    listenersBound = true
  }

  function markOffline() {
    status.value = 'offline'
  }

  function reset() {
    status.value = 'idle'
    phase.value = ''
    current.value = 0
    total.value = 0
    percent.value = 0
    processed.value = 0
    message.value = ''
    error.value = ''
  }

  async function fetchInitialStatus() {
    try {
      const { api } = await import('@/utils/api')
      const s = await api.get('/sync/initial-status')
      initialSyncCompleted.value = !!s.initialSyncCompleted
      tablesDone.value = Array.isArray(s.tablesDone) ? s.tablesDone : []
      if (s.initialSyncCompleted && status.value === 'idle') status.value = 'completed'
    } catch {
      // Offline / non connecté : on ne change rien (l'écran d'init décidera)
    }
    return initialSyncCompleted.value
  }

  /// BYPASS écran d'initialisation (hors ligne / cloud injoignable) : marque
  /// l'init comme faite côté Rust (sync_initial_skip) et repasse en "completed"
  /// pour sortir de l'écran. Les données cloud manquantes seront récupérées au
  /// retour du réseau (curseur non avancé -> reprise en delta/snapshot).
  async function skipInitialSync(): Promise<boolean> {
    try {
      const { api } = await import('@/utils/api')
      await api.post('/sync/initial-skip')
      initialSyncCompleted.value = true
      status.value = 'completed'
      return true
    } catch (e: any) {
      error.value = e?.message || 'Impossible de passer en mode local'
      return false
    }
  }

  async function startInitialSync() {
    try {
      const { api } = await import('@/utils/api')
      status.value = 'initializing'
      error.value = ''
      const res = await api.post('/sync/run')
      // started=false = une sync tourne déjà : on continue juste d'écouter
      return res?.started !== false
    } catch (e) {
      status.value = 'error'
      error.value = e?.message || 'Impossible de démarrer la synchronisation'
      return false
    }
  }

  return {
    status, phase, current, total, percent, processed, message, error,
    initialSyncCompleted, tablesDone, isBusy,
    bindListeners, fetchInitialStatus, startInitialSync, skipInitialSync, markOffline, reset,
    onPollDetail, onProgress, onError, onCompleted,
  }
})
