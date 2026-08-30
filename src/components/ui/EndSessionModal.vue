<template>
  <Modal :open="open" size="lg" @close="$emit('close')">
    <div class="p-6" v-if="session">
      <h2 class="font-gaming text-2xl font-bold mb-6 flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl bg-neon-red/20 flex items-center justify-center">
          <Square class="w-5 h-5 text-neon-red" />
        </div>
        Terminer la session
      </h2>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
        <div class="space-y-4">
          <h4 class="font-gaming font-bold text-txt-muted uppercase text-xs tracking-wider">Détails de la session</h4>
          <div class="space-y-3">
            <div class="flex justify-between"><span class="text-txt-dim">Console</span><span class="font-medium">{{ session.console_nom }}</span></div>
            <div class="flex justify-between"><span class="text-txt-dim">Joueur</span><span class="font-medium">{{ session.joueur_nom }}</span></div>
            <div class="flex justify-between"><span class="text-txt-dim">Jeu</span><span class="font-medium">{{ session.jeu_nom }}</span></div>
            <div class="flex justify-between items-center">
              <span class="text-txt-dim">Durée</span>
              <span class="font-medium font-gaming text-neon-blue text-lg">{{ timerDisplay }}</span>
            </div>
          </div>
        </div>

        <div class="space-y-4">
          <h4 class="font-gaming font-bold text-txt-muted uppercase text-xs tracking-wider">Facturation</h4>
          <div class="space-y-3">
            <div class="flex justify-between">
              <span class="text-txt-dim">Tarif horaire</span>
              <span class="font-medium">{{ formatCurrency(session.tarif_prix || 2000) }}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-txt-dim">Temps joué</span>
              <span class="font-medium">{{ timerDisplay }}</span>
            </div>
            <div class="h-px bg-white/10 my-2"></div>
            <div class="flex justify-between items-end">
              <span class="font-bold text-lg">TOTAL</span>
              <span class="font-gaming font-bold text-neon-green text-2xl">{{ formatCurrency(montantTotal) }}</span>
            </div>
          </div>
        </div>
      </div>

      <div v-if="jetonsGagnes > 0" class="card bg-bg-surface mb-6">
        <div class="flex items-center gap-3">
          <div class="w-12 h-12 rounded-xl bg-neon-yellow/20 flex items-center justify-center">
            <Trophy class="w-6 h-6 text-neon-yellow" />
          </div>
          <div>
            <p class="font-medium">Jetons attribués</p>
            <p class="text-sm text-txt-dim">{{ session.joueur_nom }} recevra <span class="font-bold text-neon-yellow">+{{ jetonsGagnes }}</span> jeton(s)</p>
          </div>
        </div>
      </div>

      <div class="mb-6">
        <h4 class="font-gaming font-bold text-txt-muted mb-3 uppercase text-xs tracking-wider">Mode de paiement</h4>
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
          <button v-for="mode in modesPaiement" :key="mode.value"
            @click="paiementMode = mode.value"
            class="p-3 rounded-xl border text-center transition-all text-sm"
            :class="paiementMode === mode.value ? 'border-neon-violet bg-neon-violet/10 text-neon-violet' : 'border-white/5 hover:border-white/20'">
            <component :is="mode.icon" class="w-5 h-5 mx-auto mb-1" />
            {{ mode.label }}
          </button>
        </div>
      </div>

      <div class="flex gap-3">
        <button @click="$emit('close')" class="btn-neon-outline flex-1">Annuler</button>
        <button @click="endSession" :disabled="loading"
          class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
          <Loader2 v-if="loading" class="w-5 h-5 animate-spin" />
          <CheckCircle v-else class="w-5 h-5" />
          Encaisser & Terminer
        </button>
      </div>
    </div>

    <div class="p-6" v-else-if="loading">
      <div class="flex items-center justify-center py-12">
        <Loader2 class="w-8 h-8 animate-spin text-neon-violet" />
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import { api } from '@/utils/api'
import { formatCurrency, formatDuration, calcJetonsEarned } from '@/utils/helpers'
import { toast } from 'sonner'
import Modal from './Modal.vue'
import { Square, Trophy, CheckCircle, Loader2, Banknote, CreditCard, Smartphone, Coins } from 'lucide-vue-next'
import { isValidModePaiement } from '@/utils/validators'

const props = defineProps({ open: Boolean, console: Object })
const emit = defineEmits(['close', 'ended'])

const loading = ref(false)
const session = ref(null)
const elapsed = ref(0)
const paiementMode = ref('especes')
let timer = null

const modesPaiement = [
  { value: 'especes', label: 'Espèces', icon: Banknote },
  { value: 'carte', label: 'Carte', icon: CreditCard },
  { value: 'mobile_money', label: 'Mobile', icon: Smartphone },
  { value: 'jetons', label: 'Jetons', icon: Coins },
]

  const sessionId = computed(() => props.console?.session_id || props.console?.id || null)
  const timerDisplay = computed(() => formatDuration(elapsed.value))

  const montantTotal = computed(() => {
    const minutes = elapsed.value / 60
    const tarif = session.value?.tarif_prix || props.console?.tarif_prix || 2000
    return Math.max(500, Math.ceil(minutes / 60) * tarif)
  })

  const jetonsGagnes = computed(() => {
    const minutes = elapsed.value / 60
    return Math.max(1, Math.floor(minutes / 60) || 1)
  })

  watch(() => props.open, async (v) => {
    if (v && sessionId.value) {
      loading.value = true
      session.value = null
      elapsed.value = 0
      paiementMode.value = 'especes'

      try {
        session.value = await api.get(`/sessions/${sessionId.value}`)
        elapsed.value = session.value.duree_secondes ?? session.value.duree_minutes * 60 ?? 0
        // fallback if duree_secondes not provided, compute from debut
        if (!elapsed.value && session.value.debut) {
          elapsed.value = Math.floor((Date.now() - new Date(session.value.debut).getTime()) / 1000)
        }
      } catch (e: any) {
        // Fallback: use console data directly if session fetch fails
        console.warn('Session fetch failed, using console data', e)
        session.value = {
          id: sessionId.value,
          console_nom: props.console.nom,
          joueur_nom: props.console.joueur_nom,
          jeu_nom: props.console.jeu_nom,
          tarif_prix: props.console.tarif_prix,
          statut: props.console.session_statut || 'en_cours',
          debut: props.console.session_debut,
          duree_secondes: 0,
        } as any
        elapsed.value = 0
        if (props.console.session_debut) {
          elapsed.value = Math.floor((Date.now() - new Date(props.console.session_debut).getTime()) / 1000)
        }
      } finally {
        loading.value = false
      }

      if (timer) clearInterval(timer)
      timer = setInterval(() => {
        if (session.value?.statut === 'en_cours') elapsed.value++
      }, 1000)
    } else {
      if (timer) clearInterval(timer)
      if (v && !sessionId.value) {
        toast.error('Aucune session active sur cette console')
        emit('close')
      }
    }
  })

onUnmounted(() => { if (timer) clearInterval(timer) })

  async function endSession() {
    if (!sessionId.value) {
      toast.error('Session introuvable')
      return
    }
    if (!isValidModePaiement(paiementMode.value)) return toast.error('Mode de paiement invalide')
    loading.value = true
    try {
      const result = await api.put(`/sessions/${sessionId.value}/terminer`, {
        mode_paiement: paiementMode.value
      })
      toast.success(`Facture ${result.facture?.numero_facture} générée — ${formatCurrency(result.montant)}`)
      emit('ended')
      emit('close')
    } catch (e: any) {
      console.error('Erreur terminaison session:', e)
      toast.error(e.message || 'Erreur lors de la terminaison')
    } finally {
      loading.value = false
    }
  }
</script>
