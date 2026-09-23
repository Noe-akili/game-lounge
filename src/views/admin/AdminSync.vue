<template>
  <div class="space-y-6">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold font-gaming text-txt flex items-center gap-2">
          <RefreshCw class="w-6 h-6 text-neon-blue" :class="{ 'animate-spin': isRefreshing }" />
          Nettoyage & Synchronisation
        </h1>
        <p class="text-sm text-txt-muted">Gérez la file d'attente locale, purgez les éléments synchronisés et réinitialisez l'état si nécessaire.</p>
      </div>

      <div class="flex items-center gap-2">
        <button
          @click="loadStats"
          :disabled="isRefreshing"
          class="btn btn-secondary flex items-center gap-2"
        >
          <RotateCw class="w-4 h-4" :class="{ 'animate-spin': isRefreshing }" />
          Actualiser
        </button>
        <button
          @click="triggerSync"
          :disabled="isSyncing"
          class="btn btn-primary flex items-center gap-2"
        >
          <Zap class="w-4 h-4" :class="{ 'animate-pulse': isSyncing }" />
          {{ isSyncing ? 'Synchronisation...' : 'Synchroniser maintenant' }}
        </button>
      </div>
    </div>

    <!-- Statistiques de la table de synchronisation -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      <div class="card p-5 border border-white/10 flex flex-col justify-between">
        <div class="flex items-center justify-between text-txt-muted text-sm">
          <span>En attente (PENDING)</span>
          <Clock class="w-5 h-5 text-neon-yellow" />
        </div>
        <div class="mt-2 text-3xl font-bold text-txt">
          {{ stats.pending }}
        </div>
        <div class="text-xs text-txt-muted mt-1">Changements locaux prêts à monter sur Supabase</div>
      </div>

      <div class="card p-5 border border-white/10 flex flex-col justify-between">
        <div class="flex items-center justify-between text-txt-muted text-sm">
          <span>Synchronisés (ACKED)</span>
          <CheckCircle2 class="w-5 h-5 text-neon-green" />
        </div>
        <div class="mt-2 text-3xl font-bold text-neon-green">
          {{ stats.acked }}
        </div>
        <div class="text-xs text-txt-muted mt-1">Éléments déjà confirmés par Supabase</div>
      </div>

      <div class="card p-5 border border-white/10 flex flex-col justify-between">
        <div class="flex items-center justify-between text-txt-muted text-sm">
          <span>Échecs (FAILED)</span>
          <AlertTriangle class="w-5 h-5 text-neon-red" />
        </div>
        <div class="mt-2 text-3xl font-bold text-neon-red">
          {{ stats.failed }}
        </div>
        <div class="text-xs text-txt-muted mt-1">Lignes rejetées par le serveur</div>
      </div>

      <div class="card p-5 border border-white/10 flex flex-col justify-between">
        <div class="flex items-center justify-between text-txt-muted text-sm">
          <span>Conflits détectés</span>
          <ShieldAlert class="w-5 h-5 text-neon-violet" />
        </div>
        <div class="mt-2 text-3xl font-bold text-neon-violet">
          {{ stats.conflicts }}
        </div>
        <div class="text-xs text-txt-muted mt-1">Conflits tracés dans l'historique</div>
      </div>
    </div>

    <!-- Actions de nettoyage -->
    <div class="card p-6 border border-white/10 space-y-6">
      <h2 class="text-lg font-bold font-gaming text-txt flex items-center gap-2">
        <Trash2 class="w-5 h-5 text-neon-red" />
        Opérations de Nettoyage de la File (sync_outbox)
      </h2>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <!-- Action 1 : Purge ACKED -->
        <div class="p-4 rounded-xl bg-bg-surface/50 border border-white/5 flex flex-col justify-between gap-4">
          <div>
            <div class="font-semibold text-txt flex items-center gap-2">
              <Sparkles class="w-4 h-4 text-neon-green" />
              Nettoyer les éléments synchronisés (ACKED)
            </div>
            <p class="text-xs text-txt-muted mt-1">
              Supprime uniquement les lignes de la file locale qui ont déjà été reçues et confirmées par Supabase. Recommandé et sans aucun risque.
            </p>
          </div>
          <button
            @click="purgeOutbox('acked')"
            :disabled="stats.acked === 0 || isOperating"
            class="btn btn-secondary self-start flex items-center gap-2"
          >
            <CheckCircle2 class="w-4 h-4" />
            Purger {{ stats.acked }} élément(s) ACKED
          </button>
        </div>

        <!-- Action 2 : Vider FAILED -->
        <div class="p-4 rounded-xl bg-bg-surface/50 border border-white/5 flex flex-col justify-between gap-4">
          <div>
            <div class="font-semibold text-txt flex items-center gap-2">
              <AlertTriangle class="w-4 h-4 text-neon-yellow" />
              Effacer les échecs (FAILED)
            </div>
            <p class="text-xs text-txt-muted mt-1">
              Retire de la file les lignes qui ont échoué définitivement lors de précédents envois cloud.
            </p>
          </div>
          <button
            @click="purgeOutbox('failed')"
            :disabled="stats.failed === 0 || isOperating"
            class="btn btn-secondary self-start flex items-center gap-2 text-neon-yellow"
          >
            <Trash2 class="w-4 h-4" />
            Effacer {{ stats.failed }} échec(s)
          </button>
        </div>

        <!-- Action 3 : Vider les conflits -->
        <div class="p-4 rounded-xl bg-bg-surface/50 border border-white/5 flex flex-col justify-between gap-4">
          <div>
            <div class="font-semibold text-txt flex items-center gap-2">
              <ShieldAlert class="w-4 h-4 text-neon-violet" />
              Effacer le journal des conflits
            </div>
            <p class="text-xs text-txt-muted mt-1">
              Réinitialise la table <code class="text-xs text-neon-blue">sync_conflicts</code>. N'affecte pas les données métiers réelles.
            </p>
          </div>
          <button
            @click="clearConflicts"
            :disabled="stats.conflicts === 0 || isOperating"
            class="btn btn-secondary self-start flex items-center gap-2 text-neon-violet"
          >
            <Trash2 class="w-4 h-4" />
            Vider le journal des conflits
          </button>
        </div>

        <!-- Action 4 : Réinitialiser curseurs -->
        <div class="p-4 rounded-xl bg-bg-surface/50 border border-white/5 flex flex-col justify-between gap-4">
          <div>
            <div class="font-semibold text-txt flex items-center gap-2">
              <RotateCcw class="w-4 h-4 text-neon-blue" />
              Réinitialiser les curseurs de synchronisation
            </div>
            <p class="text-xs text-txt-muted mt-1">
              Remet le curseur delta de cet appareil à 0 (curseur actuel : {{ stats.cursor }}). Lors de la prochaine synchronisation, l'application re-vérifiera toutes les modifications cloud.
            </p>
          </div>
          <button
            @click="resetCursors"
            :disabled="isOperating"
            class="btn btn-secondary self-start flex items-center gap-2 text-neon-blue"
          >
            <RotateCcw class="w-4 h-4" />
            Réinitialiser les curseurs
          </button>
        </div>
      </div>

      <!-- Zone Danger : Vider toute la file locale -->
      <div class="mt-6 p-4 rounded-xl bg-neon-red/5 border border-neon-red/20 space-y-3">
        <div class="flex items-center gap-2 text-neon-red font-semibold">
          <AlertOctagon class="w-5 h-5" />
          Zone Avancée : Vider complètement la file d'attente locale (sync_outbox)
        </div>
        <p class="text-xs text-txt-muted">
          Supprime <strong>tous</strong> les éléments de la file d'attente locale ({{ stats.total }} au total), y compris les éléments en attente non encore synchronisés. À utiliser uniquement si la file est bloquée par une requête corrompue.
        </p>
        <button
          @click="purgeOutbox('all')"
          :disabled="stats.total === 0 || isOperating"
          class="btn bg-neon-red/20 hover:bg-neon-red/30 text-neon-red border border-neon-red/30 flex items-center gap-2"
        >
          <Trash2 class="w-4 h-4" />
          Vider entièrement la file outbox ({{ stats.total }})
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { api } from '@/lib/api'
import { useToast } from '@/composables/useToast'
import {
  RefreshCw, RotateCw, Zap, Clock, CheckCircle2,
  AlertTriangle, ShieldAlert, Trash2, Sparkles,
  RotateCcw, AlertOctagon
} from 'lucide-vue-next'

const toast = useToast()

const stats = ref({
  total: 0,
  pending: 0,
  acked: 0,
  failed: 0,
  conflicts: 0,
  cursor: 0,
})

const isRefreshing = ref(false)
const isOperating = ref(false)
const isSyncing = ref(false)

async function loadStats() {
  isRefreshing.value = true
  try {
    const res = await api.get('/sync/outbox/stats')
    stats.value = {
      total: res.total || 0,
      pending: res.pending || 0,
      acked: res.acked || 0,
      failed: res.failed || 0,
      conflicts: res.conflicts || 0,
      cursor: res.cursor || 0,
    }
  } catch (e: any) {
    toast.error('Impossible de charger les statistiques : ' + (e.message || e))
  } finally {
    isRefreshing.value = false
  }
}

async function purgeOutbox(mode: 'acked' | 'failed' | 'all') {
  if (mode === 'all') {
    if (!confirm('Attention : cette action va vider TOUTE la file d\'attente locale, y compris les éléments en attente. Continuer ?')) {
      return
    }
  }
  isOperating.value = true
  try {
    const res = await api.post('/sync/outbox/purge', { mode })
    toast.success(`Nettoyage réussi : ${res.deleted || 0} élément(s) supprimé(s).`)
    await loadStats()
  } catch (e: any) {
    toast.error('Erreur lors du nettoyage : ' + (e.message || e))
  } finally {
    isOperating.value = false
  }
}

async function clearConflicts() {
  isOperating.value = true
  try {
    const res = await api.post('/sync/conflicts/clear')
    toast.success(`Journal des conflits vidé (${res.deleted || 0} lignes effacées).`)
    await loadStats()
  } catch (e: any) {
    toast.error('Erreur : ' + (e.message || e))
  } finally {
    isOperating.value = false
  }
}

async function resetCursors() {
  if (!confirm('Réinitialiser les curseurs de synchronisation ?')) return
  isOperating.value = true
  try {
    await api.post('/sync/cursors/reset')
    toast.success('Curseurs réinitialisés à zéro.')
    await loadStats()
  } catch (e: any) {
    toast.error('Erreur : ' + (e.message || e))
  } finally {
    isOperating.value = false
  }
}

async function triggerSync() {
  isSyncing.value = true
  try {
    await api.post('/sync/run')
    toast.success('Synchronisation lancée en arrière-plan.')
    setTimeout(loadStats, 3000)
  } catch (e: any) {
    toast.error('Erreur lancement sync : ' + (e.message || e))
  } finally {
    isSyncing.value = false
  }
}

onMounted(() => {
  loadStats()
})
</script>
