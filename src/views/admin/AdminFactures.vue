<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold truncate">Factures</h3>
      <div class="flex gap-2 items-center shrink-0 flex-wrap">
        <label class="flex items-center gap-2 cursor-pointer text-xs text-txt-dim">
          <input type="checkbox" v-model="showArchived" @change="fetchData" class="rounded accent-neon-violet cursor-pointer" />
          <span>Afficher archivées</span>
        </label>
        <select v-model="filtreStatut" @change="fetchData" class="input-field w-40 text-sm py-2">
          <option value="">Tous</option><option value="payee">Payées</option><option value="en_attente">En attente</option><option value="annulee">Annulées</option>
        </select>
        <button v-if="!isEmployee" @click="openAdd" class="btn-neon-violet flex items-center gap-2">
          <Plus class="w-4 h-4" /> Créer
        </button>
      </div>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des factures..." />
    </div>

    <div v-else-if="factures.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Receipt class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucune facture</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="f in factures" :key="f.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap hover:border-neon-violet/20 transition-colors cursor-pointer" :class="f.deleted ? 'opacity-60 border-dashed border-neon-red/40' : ''" @click="viewDetail(f)">
        <div class="w-11 h-11 rounded-xl flex items-center justify-center shrink-0"
          :class="f.statut === 'payee' ? 'bg-neon-green/20' : f.statut === 'annulee' ? 'bg-neon-red/20' : 'bg-neon-yellow/20'">
          <Receipt class="w-5 h-5"
            :class="f.statut === 'payee' ? 'text-neon-green' : f.statut === 'annulee' ? 'text-neon-red' : 'text-neon-yellow'" />
        </div>
        <div class="flex-1 min-w-0 overflow-hidden">
          <p class="font-medium font-mono text-sm truncate">{{ f.numero_facture }}</p>
          <p class="text-xs text-txt-dim truncate">{{ f.joueur_nom }} · {{ formatDate(f.date_paiement || f.created_at) }}</p>
        </div>
        <div class="text-right shrink-0 min-w-0">
          <p class="font-gaming font-bold text-neon-green truncate">{{ formatCurrency(f.montant_ttc) }}</p>
          <span v-if="f.deleted" class="badge badge-red shrink-0">Archivée</span>
          <span v-else class="badge shrink-0 max-w-full truncate" :class="statusBadge(f.statut)">{{ statusLabel(f.statut) }}</span>
        </div>
        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <button @click="viewDetail(f)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim" title="Voir">
            <Eye class="w-4 h-4" />
          </button>
          <button @click="downloadPdf(f)" class="p-2 rounded-lg hover:bg-neon-blue/10 text-neon-blue" title="Exporter PDF">
            <Download class="w-4 h-4" />
          </button>
          <button @click="printPdf(f)" class="p-2 rounded-lg hover:bg-neon-violet/10 text-neon-violet" title="Imprimer">
            <Printer class="w-4 h-4" />
          </button>
          <button @click="editFacture(f)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim" title="Modifier le paiement">
            <Pencil class="w-4 h-4" />
          </button>
          <template v-if="!isEmployee">
            <button v-if="f.statut !== 'annulee'" @click="cancelFacture(f)" class="p-2 rounded-lg hover:bg-neon-yellow/10 text-neon-yellow" title="Annuler">
              <XCircle class="w-4 h-4" />
            </button>
            <template v-if="f.deleted">
              <button @click="restoreFacture(f.id)" class="p-2 rounded-lg hover:bg-neon-green/10 text-neon-green" title="Restaurer">
                <RotateCcw class="w-4 h-4" />
              </button>
              <button @click="permanentDeleteFacture(f.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red" title="Supprimer définitivement">
                <Trash2 class="w-4 h-4" />
              </button>
            </template>
            <button v-else @click="deleteFacture(f.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red" title="Archiver">
              <Trash2 class="w-4 h-4" />
            </button>
          </template>
        </div>
      </div>
    </div>

    <Modal :open="showDetail" @close="showDetail = false" size="lg">
      <div class="p-6" v-if="selected">
        <!-- EN-TÊTE : titre + badge + actions secondaires (Modifier / Fermer) -->
        <div class="flex items-center justify-between gap-3 mb-6">
          <div class="flex items-center gap-3 min-w-0">
            <h3 class="font-gaming text-xl font-bold truncate">Facture {{ selected.numero_facture }}</h3>
            <span class="badge shrink-0" :class="statusBadge(selected.statut)">{{ statusLabel(selected.statut) }}</span>
          </div>
          <div class="flex items-center gap-1 shrink-0">
            <button @click="editFacture(selected); showDetail = false" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim" title="Modifier">
              <Pencil class="w-4 h-4" />
            </button>
            <button @click="showDetail = false" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim" title="Fermer">
              <X class="w-5 h-5" />
            </button>
          </div>
        </div>

        <!-- CONTENU de la facture (scrollable si long) -->
        <div class="max-h-[55vh] overflow-y-auto pr-1 min-w-0">
        <div class="grid grid-cols-2 gap-2 sm:gap-4 mb-6 w-full max-w-full min-w-0">
          <div class="space-y-3 min-w-0 overflow-hidden">
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 min-w-0"><span class="text-txt-dim shrink-0">Joueur</span><span class="truncate min-w-0">{{ selected.joueur_nom }}</span></div>
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 min-w-0"><span class="text-txt-dim shrink-0">Date</span><span class="truncate min-w-0">{{ formatDate(selected.created_at) }}</span></div>
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 min-w-0"><span class="text-txt-dim shrink-0">Paiement</span><span class="truncate min-w-0">{{ selected.mode_paiement || 'N/A' }}</span></div>
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 min-w-0"><span class="text-txt-dim shrink-0">Session</span><span class="truncate min-w-0">#{{ selected.session_id }}</span></div>
          </div>
          <div class="space-y-3 min-w-0 overflow-hidden">
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 min-w-0"><span class="text-txt-dim shrink-0">Montant HT</span><span class="truncate shrink-0">{{ formatCurrency(selected.montant_ht) }}</span></div>
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 min-w-0"><span class="text-txt-dim shrink-0">TVA ({{ selected.taux_tva }}%)</span><span class="truncate shrink-0">{{ formatCurrency(selected.montant_tva) }}</span></div>
            <div class="h-px bg-white/10"></div>
            <div class="flex flex-col sm:flex-row sm:justify-between gap-1 font-bold text-lg min-w-0"><span class="shrink-0">TOTAL</span><span class="text-neon-green font-gaming truncate shrink-0">{{ formatCurrency(selected.montant_ttc) }}</span></div>
          </div>
        </div>

        <div class="mb-6">
          <div class="flex items-center justify-between mb-2">
            <h4 class="font-gaming font-bold text-sm text-txt-muted">LIGNES</h4>
            <button v-if="!isEmployee" @click="openAddLigne" class="btn-neon-violet text-xs py-1 px-3 flex items-center gap-1"><Plus class="w-3 h-3" /> Ajouter ligne</button>
          </div>
          <div v-if="selected.lignes?.length" class="space-y-1 w-full max-w-full min-w-0 overflow-hidden">
            <div v-for="l in selected.lignes" :key="l.id" class="flex flex-col sm:flex-row sm:items-center justify-between gap-1 p-2 bg-bg-surface rounded-lg text-sm group w-full max-w-full min-w-0 overflow-hidden">
              <div class="flex-1 min-w-0 overflow-hidden">
                <p class="truncate">{{ l.description }}</p>
                <p class="text-xs text-txt-dim truncate">{{ l.quantite }} x {{ formatCurrency(l.prix_unitaire) }}</p>
              </div>
              <span class="font-gaming shrink-0 mx-3 truncate">{{ formatCurrency(l.total_ligne) }}</span>
              <div v-if="!isEmployee" class="flex gap-1 shrink-0 flex-wrap opacity-0 group-hover:opacity-100 transition-opacity">
                <button @click="editLigne(l)" class="p-1.5 rounded-lg hover:bg-bg-hover text-txt-dim"><Pencil class="w-3 h-3" /></button>
                <button @click="deleteLigne(l.id)" class="p-1.5 rounded-lg hover:bg-neon-red/10 text-neon-red"><Trash2 class="w-3 h-3" /></button>
              </div>
            </div>
          </div>
          <p v-else class="text-txt-dim text-sm text-center py-3 bg-bg-surface rounded-xl">Aucune ligne</p>
        </div>

        <!-- HISTORIQUE DES JETONS (DÉBITS & REMBOURSEMENTS LIÉS AU PAIEMENT) -->
        <div class="mb-6">
          <div class="flex items-center justify-between mb-2">
            <h4 class="font-gaming font-bold text-sm text-txt-muted flex items-center gap-2">
              <Coins class="w-4 h-4 text-neon-yellow" />
              MOUVEMENTS DE JETONS (DÉBITS / REMBOURSEMENTS)
            </h4>
          </div>
          <div v-if="selected.jetons_transactions?.length" class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
            <div v-for="tx in selected.jetons_transactions" :key="tx.id" class="p-3 bg-bg-surface border border-white/5 rounded-xl text-sm flex items-center justify-between gap-3">
              <div class="flex items-center gap-3 min-w-0">
                <div class="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                  :class="tx.type === 'gain' ? 'bg-neon-green/20 text-neon-green' : 'bg-neon-red/20 text-neon-red'">
                  <Coins class="w-4 h-4" />
                </div>
                <div class="min-w-0">
                  <p class="font-medium truncate">{{ tx.raison || (tx.type === 'gain' ? 'Remboursement de jetons' : 'Débit de jetons') }}</p>
                  <p class="text-xs text-txt-dim">{{ formatDate(tx.created_at) }}</p>
                </div>
              </div>
              <div class="text-right shrink-0">
                <span class="font-gaming font-bold text-base" :class="tx.type === 'gain' ? 'text-neon-green' : 'text-neon-red'">
                  {{ tx.type === 'gain' ? '+' : '-' }}{{ tx.quantite }} jeton{{ tx.quantite > 1 ? 's' : '' }}
                </span>
                <span class="block text-xs uppercase tracking-wider font-semibold" :class="tx.type === 'gain' ? 'text-neon-green' : 'text-neon-red'">
                  {{ tx.type === 'gain' ? 'Remboursement' : 'Débit' }}
                </span>
              </div>
            </div>
          </div>
          <p v-else class="text-txt-dim text-xs text-center py-3 bg-bg-surface/50 rounded-xl border border-white/5">
            Aucun débit ou remboursement de jetons enregistré sur cette facture.
          </p>
        </div>

        </div><!-- /contenu scrollable -->

        <!-- BARRE D'ACTIONS SÉPARÉE du contenu (item 5) : document d'abord,
             fermeture discrète à droite. -->
        <div class="mt-6 pt-4 border-t border-white/10 flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
          <button @click="downloadPdf(selected)" class="btn-neon-blue sm:flex-1 flex items-center justify-center gap-2">
            <Download class="w-4 h-4" /> Exporter PDF
          </button>
          <button @click="printPdf(selected)" class="btn-neon-outline sm:flex-1 flex items-center justify-center gap-2">
            <Printer class="w-4 h-4" /> Imprimer
          </button>
          <button @click="showDetail = false" class="btn-neon-outline sm:w-auto px-6">Fermer</button>
        </div>
      </div>
    </Modal>

    <Modal :open="showLigneForm" @close="closeLigneForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingLigneId ? 'Modifier' : 'Ajouter' }} une ligne</h3>
        <div class="space-y-4">
          <input v-model="ligneForm.description" placeholder="Description" class="input-field" />
          <div class="grid grid-cols-3 gap-2 sm:gap-4 w-full max-w-full min-w-0">
            <input v-model.number="ligneForm.quantite" type="number" placeholder="Qté" class="input-field" />
            <input v-model.number="ligneForm.prix_unitaire" type="number" placeholder="P.U. (FC)" class="input-field" />
            <input v-model.number="ligneForm.total_ligne" type="number" placeholder="Total (FC)" class="input-field" />
          </div>
          <div class="flex gap-3">
            <button @click="closeLigneForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveLigne" :disabled="!ligneForm.description || !ligneForm.quantite" class="btn-neon-violet flex-1">{{ editingLigneId ? 'Modifier' : 'Créer' }}</button>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="showForm" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ isEmployee ? 'Modifier le paiement' : (editingId ? 'Modifier' : 'Créer') + ' une facture' }}</h3>
        <div class="space-y-4">
          <template v-if="!isEmployee || !editingId">
          <div>
            <label class="text-sm text-txt-muted">Joueur</label>
            <select v-model.number="form.joueur_id" class="input-field">
              <option :value="null" disabled>Choisir un joueur</option>
              <option v-for="j in joueursList" :key="j.id" :value="j.id">{{ j.nom }}</option>
            </select>
          </div>
          <div>
            <label class="text-sm text-txt-muted">Session</label>
            <select v-model.number="form.session_id" class="input-field">
              <option :value="null" disabled>Choisir une session</option>
              <option v-for="s in sessionsList" :key="s.id" :value="s.id">#{{ s.id }} — {{ s.joueur_nom }} — {{ s.console_nom }} ({{ s.statut }})</option>
            </select>
          </div>
          <div class="grid grid-cols-2 gap-2 sm:gap-4 w-full max-w-full min-w-0">
            <div class="min-w-0">
              <label class="text-sm text-txt-muted">Montant HT (FC)</label>
              <input v-model.number="form.montant_ht" type="number" class="input-field w-full max-w-full min-w-0" />
            </div>
            <div class="min-w-0">
              <label class="text-sm text-txt-muted">Taux TVA (%)</label>
              <input v-model.number="form.taux_tva" type="number" class="input-field w-full max-w-full min-w-0" />
            </div>
          </div>
          <div class="grid grid-cols-2 gap-2 sm:gap-4 w-full max-w-full min-w-0">
            <div class="min-w-0">
              <label class="text-sm text-txt-muted">Montant TVA (FC)</label>
              <input v-model.number="form.montant_tva" type="number" class="input-field w-full max-w-full min-w-0" />
            </div>
            <div class="min-w-0">
              <label class="text-sm text-txt-muted">Montant TTC (FC)</label>
              <input v-model.number="form.montant_ttc" type="number" class="input-field w-full max-w-full min-w-0" />
            </div>
          </div>
          </template>
          <select v-model="form.mode_paiement" class="input-field">
            <option value="especes">Espèces</option>
            <option value="carte">Carte</option>
            <option value="mobile">Mobile Money</option>
            <option value="jetons">Jetons</option>
          </select>
          <select v-if="!isEmployee" v-model="form.statut" class="input-field">
            <option value="payee">Payée</option>
            <option value="en_attente">En attente</option>
            <option value="annulee">Annulée</option>
          </select>
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveFacture" :disabled="isEmployee ? !form.mode_paiement : (!form.joueur_id || !form.session_id || !form.montant_ttc)" class="btn-neon-violet flex-1">{{ isEmployee ? 'Enregistrer' : (editingId ? 'Modifier' : 'Créer') }}</button>
          </div>
        </div>
      </div>
    </Modal>
    <!-- responsive table overflow helper: ensures horizontal scroll on mobile -->
    <div class="w-full overflow-x-auto -mx-4 sm:mx-0 hidden" aria-hidden="true"><table class="min-w-[600px] w-full"><tbody><tr><td></td></tr></tbody></table></div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { api } from '@/utils/api'
import { useAuthStore } from '@/stores/auth'
import { getFacturePdfBlob, saveFacturePdf } from '@/lib/clientPdf'
import { formatCurrency, formatDate } from '@/utils/helpers'
import { toast } from 'vue-sonner'
import Modal from '@/components/ui/Modal.vue'
import { Receipt, Eye, Download, Printer, XCircle, Plus, Pencil, Trash2, X, RotateCcw } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { isValidId, isValidPrix, isValidFactureStatut, isValidModePaiement, isValidQuantite, sanitizeInput } from '@/utils/validators'

const auth = useAuthStore()
const isEmployee = computed(() => auth.user?.role === 'employe')

const factures = ref([])
const joueursList = ref([])
const sessionsList = ref([])
const loading = ref(false)
const filtreStatut = ref('')
const showArchived = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const showForm = ref(false)
const editingId = ref(null)
const form = reactive({ joueur_id: null, session_id: null, montant_ht: 0, taux_tva: 20, montant_tva: 0, montant_ttc: 0, mode_paiement: 'especes', statut: 'payee' })
const showLigneForm = ref(false)
const editingLigneId = ref(null)
const ligneForm = reactive({ description: '', quantite: 1, prix_unitaire: 0, total_ligne: 0 })

function statusBadge(s) { return { payee: 'badge-green', en_attente: 'badge-yellow', annulee: 'badge-red' }[s] || 'badge-violet' }
function statusLabel(s) { return { payee: 'Payée', en_attente: 'En attente', annulee: 'Annulée' }[s] || s }

async function fetchData() {
  loading.value = true
  try {
    const params = new URLSearchParams()
    if (filtreStatut.value) params.set('statut', filtreStatut.value)
    params.set('include_deleted', showArchived.value ? '1' : '0')
    factures.value = await api.get(`/factures?${params.toString()}`)
  } catch (e: any) { toast.error('Factures: ' + (e.message || 'erreur')) }
  finally { loading.value = false }
}

async function fetchJoueursSessions() {
  try { joueursList.value = await api.get('/joueurs') }
  catch (e: any) { toast.error('Joueurs: ' + (e.message || 'erreur')) }
  try { sessionsList.value = await api.get('/sessions') }
  catch (e: any) { toast.error('Sessions: ' + (e.message || 'erreur')) }
}

function openAdd() {
  editingId.value = null
  Object.assign(form, { joueur_id: null, session_id: null, montant_ht: 0, taux_tva: 20, montant_tva: 0, montant_ttc: 0, mode_paiement: 'especes', statut: 'payee' })
  showForm.value = true
  if (!isEmployee.value) fetchJoueursSessions()
}

function editFacture(f) {
  editingId.value = f.id
  Object.assign(form, { joueur_id: f.joueur_id, session_id: f.session_id, montant_ht: f.montant_ht, taux_tva: f.taux_tva, montant_tva: f.montant_tva, montant_ttc: f.montant_ttc, mode_paiement: f.mode_paiement || 'especes', statut: f.statut })
  showForm.value = true
  fetchJoueursSessions()
}

function closeForm() { showForm.value = false; editingId.value = null }

async function saveFacture() {
  if (form.mode_paiement && !isValidModePaiement(form.mode_paiement)) return toast.error('Mode de paiement invalide')
  if (isEmployee.value) {
    if (!editingId.value) return toast.error('Un employé ne peut pas créer une facture ici')
    try {
      await api.put(`/factures/${editingId.value}`, { mode_paiement: form.mode_paiement })
      toast.success('Mode de paiement modifié')
      closeForm(); fetchData()
    } catch (e) { toast.error(e.message) }
    return
  }
  if (!isValidId(form.joueur_id)) return toast.error('Joueur invalide')
  if (!isValidId(form.session_id)) return toast.error('Session invalide')
  if (!isValidPrix(form.montant_ttc)) return toast.error('Montant TTC invalide (1-1000000)')
  if (form.statut && !isValidFactureStatut(form.statut)) return toast.error('Statut invalide')
  if (!form.montant_tva && form.montant_ht && form.taux_tva) form.montant_tva = Math.round(form.montant_ht * form.taux_tva / 100)
  if (!form.montant_ht && form.montant_ttc && form.taux_tva) form.montant_ht = Math.round(form.montant_ttc / (1 + form.taux_tva / 100))
  try {
    if (editingId.value) { await api.put(`/factures/${editingId.value}`, { statut: form.statut, mode_paiement: form.mode_paiement, montant_ttc: form.montant_ttc }); toast.success('Facture modifiée') }
    else { await api.post('/factures', { ...form }); toast.success('Facture créée') }
    closeForm(); fetchData()
  } catch (e) { toast.error(e.message) }
}

async function deleteFacture(id) {
  if (!confirm('Archiver cette facture ?')) return
  try { await api.delete(`/factures/${id}`); toast.success('Facture archivée'); await fetchData() }
  catch (e) { toast.error(e.message) }
}
async function restoreFacture(id) {
  try { await api.post(`/factures/${id}/restore`, {}); toast.success('Facture restaurée'); await fetchData() }
  catch (e) { toast.error(e.message) }
}
async function permanentDeleteFacture(id) {
  if (!confirm('Supprimer définitivement cette facture ? Cette action est irréversible.')) return
  try { await api.delete(`/factures/${id}/permanent`); toast.success('Facture supprimée définitivement'); await fetchData() }
  catch (e) { toast.error(e.message) }
}

async function viewDetail(f) {
  try { selected.value = await api.get(`/factures/${f.id}`); showDetail.value = true }
  catch (e) { toast.error(e.message) }
}

async function downloadPdf(f) {
  // Enregistre dans le dossier choisi (mémorisé) ; fallback ouverture si
  // Android refuse l'accès direct (Scoped Storage -> SAF).
  try {
    const res = await saveFacturePdf(f.id, { filename: `${f.numero_facture}.pdf` })
    if (res.path) toast.success(`PDF enregistré : ${res.path}`)
    else if (res.opened) toast.info('PDF ouvert — utilisez "Enregistrer" du viewer')
  } catch (e: any) { console.error('[pdf] save failed', e); toast.error(e?.message || 'Erreur enregistrement PDF') }
}

async function printPdf(f) {
  try {
    const blob = await getFacturePdfBlob(f.id)
    const url = URL.createObjectURL(blob)
    const w = window.open(url, '_blank')
    if (!w) toast.error('Autorisez les fenêtres pop-up pour imprimer')
    setTimeout(() => URL.revokeObjectURL(url), 8000)
  } catch (e: any) { toast.error(e?.message || 'Erreur ouverture PDF') }
}

async function cancelFacture(f) {
  const motif = prompt("Motif d'annulation :")
  if (!motif) return
  try { await api.put(`/factures/${f.id}/annuler`, { motif }); toast.success('Facture annulée'); fetchData() }
  catch (e) { toast.error(e.message) }
}

function openAddLigne() {
  if (!selected.value) return
  editingLigneId.value = null
  Object.assign(ligneForm, { description: '', quantite: 1, prix_unitaire: selected.value.montant_ttc || 0, total_ligne: selected.value.montant_ttc || 0 })
  showLigneForm.value = true
}
function editLigne(l) {
  editingLigneId.value = l.id
  Object.assign(ligneForm, { description: l.description, quantite: l.quantite, prix_unitaire: l.prix_unitaire, total_ligne: l.total_ligne })
  showLigneForm.value = true
}
function closeLigneForm() { showLigneForm.value = false; editingLigneId.value = null }
async function saveLigne() {
  if (!selected.value) return
  if (!sanitizeInput(ligneForm.description, 500) || ligneForm.description.trim().length < 2) return toast.error('Description invalide (2-500 caractères, < > interdits)')
  if (!isValidQuantite(ligneForm.quantite)) return toast.error('Quantité invalide (1-10000)')
  if (!isValidPrix(ligneForm.prix_unitaire)) return toast.error('Prix unitaire invalide (1-1000000)')
  ligneForm.description = sanitizeInput(ligneForm.description, 500)
  // auto calc total if not provided
  if (!ligneForm.total_ligne) ligneForm.total_ligne = ligneForm.quantite * ligneForm.prix_unitaire
  if (!isValidPrix(ligneForm.total_ligne)) return toast.error('Total invalide')
  try {
    if (editingLigneId.value) { await api.put(`/lignes_facture/${editingLigneId.value}`, { ...ligneForm }); toast.success('Ligne modifiée') }
    else { await api.post('/lignes_facture', { facture_id: selected.value.id, ...ligneForm }); toast.success('Ligne ajoutée') }
    closeLigneForm()
    selected.value = await api.get(`/factures/${selected.value.id}`)
    fetchData()
  } catch (e) { toast.error(e.message) }
}
async function deleteLigne(id) {
  if (!confirm('Supprimer cette ligne ?')) return
  try { await api.delete(`/lignes_facture/${id}`); toast.success('Ligne supprimée'); selected.value = await api.get(`/factures/${selected.value.id}`); fetchData() }
  catch (e) { toast.error(e.message) }
}

onMounted(() => { fetchData(); if (!isEmployee.value) fetchJoueursSessions() })
</script>
