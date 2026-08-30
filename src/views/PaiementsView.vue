<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <h3 class="font-gaming text-lg font-bold">Paiements récents</h3>
      <div class="flex gap-2">
        <select v-model="filtreStatut" @change="fetchData" class="input-field w-32 text-sm py-2">
          <option value="">Tous</option><option value="payee">Payées</option><option value="en_attente">En attente</option><option value="annulee">Annulées</option>
        </select>
        <button @click="fetchData" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim">
          <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': loading }" />
        </button>
      </div>
    </div>

    <div v-if="loading" class="card">
      <Loader variant="neon" size="lg" text="Chargement des paiements..." />
    </div>

    <div v-else-if="factures.length === 0" class="card text-center py-12">
      <Receipt class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun paiement enregistré</p>
    </div>

    <div v-else class="space-y-2">
      <div v-for="f in factures" :key="f.id" class="card flex items-center gap-4 hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewDetail(f)">
        <div class="w-11 h-11 rounded-xl flex items-center justify-center shrink-0"
          :class="f.statut === 'payee' ? 'bg-neon-green/20' : f.statut === 'annulee' ? 'bg-neon-red/20' : 'bg-neon-yellow/20'">
          <CheckCircle v-if="f.statut === 'payee'" class="w-5 h-5 text-neon-green" />
          <XCircle v-else-if="f.statut === 'annulee'" class="w-5 h-5 text-neon-red" />
          <Clock v-else class="w-5 h-5 text-neon-yellow" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium font-mono text-sm">{{ f.numero_facture }}</p>
          <p class="text-xs text-txt-dim">{{ f.joueur_nom }} · {{ formatDate(f.date_paiement || f.created_at) }}</p>
        </div>
        <div class="text-right shrink-0">
          <p class="font-gaming font-bold text-neon-green">{{ formatCurrency(f.montant_ttc) }}</p>
          <span class="badge" :class="f.statut === 'payee' ? 'badge-green' : f.statut === 'annulee' ? 'badge-red' : 'badge-yellow'">
            {{ f.statut === 'payee' ? 'Payée' : f.statut === 'annulee' ? 'Annulée' : 'En attente' }}
          </span>
        </div>
        <div class="flex gap-1 shrink-0" @click.stop>
          <button @click="viewDetail(f)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim" title="Voir">
            <Eye class="w-4 h-4" />
          </button>
          <button @click="downloadPdf(f)" class="p-2 rounded-lg hover:bg-neon-blue/10 text-neon-blue" title="PDF">
            <Download class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <Modal :open="showDetail" @close="showDetail = false" size="lg">
      <div class="p-6" v-if="selected">
        <div class="flex items-center justify-between mb-6">
          <h3 class="font-gaming text-xl font-bold">Facture {{ selected.numero_facture }}</h3>
          <span class="badge" :class="selected.statut === 'payee' ? 'badge-green' : selected.statut === 'annulee' ? 'badge-red' : 'badge-yellow'">{{ selected.statut }}</span>
        </div>
        <div class="grid grid-cols-2 gap-6 mb-6">
          <div class="space-y-3">
            <div class="flex justify-between"><span class="text-txt-dim">Joueur</span><span>{{ selected.joueur_nom }}</span></div>
            <div class="flex justify-between"><span class="text-txt-dim">Date</span><span>{{ formatDate(selected.created_at) }}</span></div>
            <div class="flex justify-between"><span class="text-txt-dim">Paiement</span><span>{{ selected.mode_paiement || 'N/A' }}</span></div>
          </div>
          <div class="space-y-3">
            <div class="flex justify-between"><span class="text-txt-dim">Montant HT</span><span>{{ formatCurrency(selected.montant_ht) }}</span></div>
            <div class="flex justify-between"><span class="text-txt-dim">TVA ({{ selected.taux_tva }}%)</span><span>{{ formatCurrency(selected.montant_tva) }}</span></div>
            <div class="h-px bg-white/10"></div>
            <div class="flex justify-between font-bold text-lg"><span>TOTAL</span><span class="text-neon-green font-gaming">{{ formatCurrency(selected.montant_ttc) }}</span></div>
          </div>
        </div>
        <div v-if="selected.lignes?.length" class="mb-6">
          <h4 class="font-gaming font-bold text-sm text-txt-muted mb-2">LIGNES</h4>
          <div class="space-y-1">
            <div v-for="l in selected.lignes" :key="l.id" class="flex items-center justify-between p-2 bg-bg-surface rounded-lg text-sm">
              <span>{{ l.description }}</span>
              <span class="font-gaming">{{ formatCurrency(l.total_ligne) }}</span>
            </div>
          </div>
        </div>
        <div class="flex gap-3">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <button @click="downloadPdf(selected)" class="btn-neon-blue flex-1 flex items-center justify-center gap-2">
            <Download class="w-4 h-4" /> Télécharger PDF
          </button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { api } from '@/utils/api'
import { formatCurrency, formatDate } from '@/utils/helpers'
import { toast } from 'sonner'
import Modal from '@/components/ui/Modal.vue'
import { CheckCircle, Clock, XCircle, Receipt, RefreshCw, Eye, Download } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'

const factures = ref([])
const loading = ref(false)
const filtreStatut = ref('')
const showDetail = ref(false)
const selected = ref(null)

async function fetchData() {
  loading.value = true
  try {
    const params = filtreStatut.value ? `?statut=${filtreStatut.value}` : ''
    factures.value = await api.get(`/factures${params}`)
  } catch {} finally { loading.value = false }
}

async function viewDetail(f) {
  try { selected.value = await api.get(`/factures/${f.id}`); showDetail.value = true }
  catch (e) { toast.error(e.message) }
}

async function downloadPdf(f) {
  try {
    const token = localStorage.getItem('gl_token')
    const res = await fetch(`/api/factures/${f.id}/pdf`, { headers: { Authorization: `Bearer ${token}` } })
    if (!res.ok) throw new Error('Erreur PDF')
    const blob = await res.blob()
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a'); a.href = url; a.download = `${f.numero_facture}.pdf`; a.click()
    URL.revokeObjectURL(url)
  } catch { toast.error('Erreur téléchargement PDF') }
}

onMounted(fetchData)
</script>
