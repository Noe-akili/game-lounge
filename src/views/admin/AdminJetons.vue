<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Gestion des Jetons</h3>
      <button @click="openAddTransaction" class="btn-neon-violet flex items-center gap-2">
        <Plus class="w-4 h-4" /> Nouvelle transaction
      </button>
    </div>

    <div class="relative w-full max-w-full min-w-0 overflow-hidden">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim shrink-0" />
      <input v-model="search" @input="filterTransactions" placeholder="Rechercher par joueur ou raison..." class="input-field pl-10 w-full max-w-full min-w-0" />
    </div>

    <div v-if="loadingTx" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des transactions..." />
    </div>

    <div v-else-if="filteredTransactions.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Coins class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucune transaction</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="t in filteredTransactions" :key="t.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewTransaction(t)">
        <div class="w-11 h-11 rounded-xl flex items-center justify-center shrink-0"
          :class="t.type === 'gain' ? 'bg-neon-green/20' : t.type === 'bonus' ? 'bg-neon-violet/20' : 'bg-neon-red/20'">
          <Coins class="w-5 h-5" :class="t.type === 'gain' ? 'text-neon-green' : t.type === 'bonus' ? 'text-neon-violet' : 'text-neon-red'" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium truncate">{{ t.joueur_nom || 'Joueur #' + t.joueur_id }} <span class="text-txt-dim font-normal">· {{ t.raison || 'Aucune raison' }}</span></p>
          <p class="text-xs text-txt-dim">{{ formatDate(t.created_at) }} · {{ t.type }}</p>
        </div>
        <div class="text-right shrink-0 min-w-0">
          <p class="font-gaming font-bold truncate" :class="t.type === 'depense' ? 'text-neon-red' : 'text-neon-yellow'">{{ t.type === 'depense' ? '-' : '+' }}{{ t.quantite }} jetons</p>
          <span class="badge shrink-0 max-w-full truncate" :class="t.type === 'gain' ? 'badge-green' : t.type === 'bonus' ? 'badge-violet' : 'badge-red'">{{ t.type }}</span>
        </div>
        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <button @click="editTransaction(t)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
            <Pencil class="w-4 h-4" />
          </button>
          <button @click="deleteTransaction(t.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Supprimer">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <div class="card space-y-4 w-full max-w-full min-w-0 overflow-hidden">
      <h4 class="font-gaming font-bold flex items-center gap-2 w-full max-w-full min-w-0"><Coins class="w-5 h-5 text-neon-yellow shrink-0" /> <span class="truncate">Règle d'attribution des jetons</span></h4>

      <div v-if="loading" class="py-4">
        <Loader variant="neon" size="md" text="Chargement des paramètres..." />
      </div>

      <template v-else>
        <select v-model="form.regle_type" class="input-field">
          <option value="temps">1 jeton chaque heure jouée</option>
          <option value="montant">Bonus selon le montant dépensé</option>
        </select>

        <div class="grid grid-cols-2 gap-2 sm:gap-4 w-full max-w-full min-w-0">
          <div class="min-w-0 w-full max-w-full">
            <label class="text-sm text-txt-muted">Seuil (minutes)</label>
            <input v-model.number="form.seuil" type="number" class="input-field w-full max-w-full min-w-0" />
          </div>
          <div class="min-w-0 w-full max-w-full">
            <label class="text-sm text-txt-muted">Jetons attribués</label>
            <input v-model.number="form.jetons_attribues" type="number" class="input-field w-full max-w-full min-w-0" />
          </div>
        </div>

        <label class="flex items-center gap-3 cursor-pointer">
          <input type="checkbox" v-model="form.actif" class="w-5 h-5 rounded bg-bg-surface border-white/20 text-neon-violet" />
          <span class="font-medium">Actif</span>
        </label>

        <div v-if="form.actif" class="space-y-2 bg-bg-surface rounded-xl p-4 w-full max-w-full min-w-0 overflow-hidden">
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-1 p-2 w-full max-w-full min-w-0">
            <span class="text-sm truncate">5 000 FC et plus</span><span class="font-gaming font-bold text-neon-yellow shrink-0">+2 jetons</span>
          </div>
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-1 p-2 w-full max-w-full min-w-0">
            <span class="text-sm truncate">10 000 FC et plus</span><span class="font-gaming font-bold text-neon-yellow shrink-0">+5 jetons</span>
          </div>
        </div>

        <button @click="save" class="btn-neon-violet w-full">Enregistrer les paramètres</button>
      </template>
    </div>

    <Modal :open="showForm" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Nouvelle' }} transaction</h3>
        <div class="space-y-4">
          <div>
            <label class="text-sm text-txt-muted">Joueur</label>
            <select v-model.number="formTx.joueur_id" class="input-field">
              <option :value="null" disabled>Choisir un joueur</option>
              <option v-for="j in joueurs" :key="j.id" :value="j.id">{{ j.nom }} — {{ j.telephone }}</option>
            </select>
          </div>
          <select v-model="formTx.type" class="input-field">
            <option value="gain">Gain</option>
            <option value="bonus">Bonus</option>
            <option value="depense">Dépense</option>
          </select>
          <input v-model.number="formTx.quantite" type="number" placeholder="Quantité de jetons" class="input-field" />
          <input v-model="formTx.raison" placeholder="Raison (ex: Bonus fidélité)" class="input-field" />
          <input v-model.number="formTx.session_id" type="number" placeholder="Session ID (optionnel)" class="input-field" />
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveTransaction" :disabled="!formTx.joueur_id || !formTx.quantite" class="btn-neon-violet flex-1">{{ editingId ? 'Modifier' : 'Créer' }}</button>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="showDetail" @close="showDetail = false">
      <div class="p-6" v-if="selected">
        <h3 class="font-gaming text-xl font-bold mb-4">Détails transaction #{{ selected.id }}</h3>
        <div class="space-y-3">
          <div class="flex justify-between"><span class="text-txt-dim">Joueur</span><span>{{ selected.joueur_nom || 'Joueur #' + selected.joueur_id }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Type</span><span class="badge" :class="selected.type === 'gain' ? 'badge-green' : selected.type === 'bonus' ? 'badge-violet' : 'badge-red'">{{ selected.type }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Quantité</span><span class="font-gaming font-bold text-neon-yellow">{{ selected.quantite }} jetons</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Raison</span><span>{{ selected.raison || '—' }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Session</span><span>{{ selected.session_id || '—' }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Date</span><span class="text-sm">{{ formatDate(selected.created_at) }}</span></div>
        </div>
        <div class="flex gap-3 mt-6">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <button @click="editTransaction(selected); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2"><Pencil class="w-4 h-4" /> Modifier</button>
        </div>
      </div>
    </Modal>
    <!-- responsive table overflow helper: ensures horizontal scroll on mobile -->
    <div class="w-full overflow-x-auto -mx-4 sm:mx-0 hidden" aria-hidden="true"><table class="min-w-[600px] w-full"><tbody><tr><td></td></tr></tbody></table></div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed } from 'vue'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'
import Loader from '@/components/ui/Loader.vue'
import Modal from '@/components/ui/Modal.vue'
import { formatDate } from '@/utils/helpers'
import { Plus, Pencil, Trash2, Coins, Search } from 'lucide-vue-next'
import { isValidId, isValidJetonType, isValidQuantite, isValidRegleType, isValidSeuil, isValidJetonsAttribues, sanitizeInput } from '@/utils/validators'

const loading = ref(true)
const loadingTx = ref(true)
const form = reactive({ regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })

const transactions = ref([])
const joueurs = ref([])
const search = ref('')
const showForm = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const editingId = ref(null)
const formTx = reactive({ joueur_id: null, type: 'gain', quantite: 1, raison: '', session_id: null })

const filteredTransactions = computed(() => {
  if (!search.value) return transactions.value
  const q = search.value.toLowerCase()
  return transactions.value.filter(t => (t.joueur_nom && t.joueur_nom.toLowerCase().includes(q)) || (t.raison && t.raison.toLowerCase().includes(q)) || String(t.type).toLowerCase().includes(q))
})

function filterTransactions() { /* computed handles it */ }

async function fetchParametres() {
  loading.value = true
  try { const data = await api.get('/parametres/fidelite'); if (data) Object.assign(form, data) }
  catch (e: any) { toast.error('Paramètres: ' + (e.message || 'erreur')) }
  finally { loading.value = false }
}

async function fetchTransactions() {
  loadingTx.value = true
  try { transactions.value = await api.get('/jetons') }
  catch (e: any) { toast.error('Transactions: ' + (e.message || 'erreur')) }
  finally { loadingTx.value = false }
}

async function fetchJoueurs() {
  try { joueurs.value = await api.get('/joueurs') }
  catch (e: any) { toast.error('Joueurs: ' + (e.message || 'erreur')) }
}

function openAddTransaction() {
  editingId.value = null
  Object.assign(formTx, { joueur_id: null, type: 'gain', quantite: 1, raison: '', session_id: null })
  showForm.value = true
}

function editTransaction(t) {
  editingId.value = t.id
  Object.assign(formTx, { joueur_id: t.joueur_id, type: t.type, quantite: t.quantite, raison: t.raison || '', session_id: t.session_id || null })
  showForm.value = true
}

function closeForm() { showForm.value = false; editingId.value = null }

async function viewTransaction(t) {
  try { selected.value = await api.get(`/jetons/${t.id}`); if (selected.value && !selected.value.joueur_nom) selected.value.joueur_nom = t.joueur_nom } catch { selected.value = t }
  showDetail.value = true
}

async function saveTransaction() {
  if (!isValidId(formTx.joueur_id)) return toast.error('Joueur invalide')
  if (!isValidJetonType(formTx.type)) return toast.error('Type invalide')
  if (!isValidQuantite(formTx.quantite)) return toast.error('Quantité invalide (1-10000)')
  if (formTx.raison) formTx.raison = sanitizeInput(formTx.raison, 500)
  if (formTx.session_id && !isValidId(formTx.session_id)) return toast.error('Session invalide')
  try {
    const payload: any = { ...formTx }
    if (!payload.session_id) delete payload.session_id
    if (editingId.value) { await api.put(`/jetons/${editingId.value}`, { type: payload.type, quantite: payload.quantite, raison: payload.raison }); toast.success('Transaction modifiée') }
    else { await api.post('/jetons', payload); toast.success('Transaction créée') }
    closeForm(); fetchTransactions()
  } catch (e) { toast.error(e.message) }
}

async function deleteTransaction(id) {
  if (!confirm('Supprimer cette transaction ?')) return
  try { await api.delete(`/jetons/${id}`); toast.success('Transaction supprimée'); fetchTransactions() }
  catch (e) { toast.error(e.message) }
}

async function save() {
  if (!isValidRegleType(form.regle_type)) return toast.error('Type de règle invalide')
  if (!isValidSeuil(form.seuil)) return toast.error('Seuil invalide (1-10000)')
  if (!isValidJetonsAttribues(form.jetons_attribues)) return toast.error('Jetons attribués invalides (1-1000)')
  try { await api.put('/parametres/fidelite', { ...form }); toast.success('Paramètres enregistrés') }
  catch (e) { toast.error(e.message) }
}

onMounted(async () => {
  await Promise.all([fetchParametres(), fetchTransactions(), fetchJoueurs()])
})
</script>
