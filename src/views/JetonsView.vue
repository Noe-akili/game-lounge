<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <h3 class="font-gaming text-lg font-bold">Jetons & Fidélité</h3>
      <button @click="openAdd" class="btn-neon-violet flex items-center gap-2">
        <Plus class="w-4 h-4" /> Nouvelle transaction
      </button>
    </div>

    <div v-if="loading" class="card">
      <Loader variant="neon" size="lg" text="Chargement des jetons..." />
    </div>

    <template v-else>
      <div class="grid grid-cols-2 gap-3">
        <div class="stat-card">
          <span class="stat-value text-neon-yellow">{{ totalJetons }}</span>
          <span class="stat-label">Jetons distribués (total)</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-blue">{{ transactions.length }}</span>
          <span class="stat-label">Transactions</span>
        </div>
      </div>

      <div class="card">
        <div class="flex items-center justify-between mb-3">
          <h4 class="font-gaming font-bold">Historique des transactions</h4>
          <span class="badge-violet">{{ filtered.length }} résultat(s)</span>
        </div>
        <div class="relative mb-3">
          <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim" />
          <input v-model="search" placeholder="Rechercher joueur ou raison..." class="input-field pl-10" />
        </div>
        <div v-if="filtered.length === 0" class="text-center py-8">
          <Coins class="w-12 h-12 text-txt-dim mx-auto mb-3" />
          <p class="text-txt-dim text-sm">Aucune transaction</p>
        </div>
        <div v-else class="space-y-2 max-h-[60vh] overflow-y-auto">
          <div v-for="t in filtered" :key="t.id" class="flex items-center gap-3 p-3 bg-bg-surface rounded-xl hover:border-neon-violet/20 border border-transparent transition-colors cursor-pointer" @click="viewDetail(t)">
            <div class="w-10 h-10 rounded-xl flex items-center justify-center shrink-0" :class="t.type === 'depense' ? 'bg-neon-red/20' : 'bg-neon-yellow/20'">
              <Coins class="w-5 h-5" :class="t.type === 'depense' ? 'text-neon-red' : 'text-neon-yellow'" />
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium truncate">{{ t.joueur_nom || 'Joueur #' + t.joueur_id }}</p>
              <p class="text-xs text-txt-dim truncate">{{ t.raison || '—' }} · {{ formatDate(t.created_at) }}</p>
            </div>
            <span class="font-gaming font-bold shrink-0" :class="t.type === 'depense' ? 'text-neon-red' : 'text-neon-yellow'">{{ t.type === 'depense' ? '-' : '+' }}{{ t.quantite }}</span>
            <span class="badge shrink-0" :class="t.type === 'gain' ? 'badge-green' : t.type === 'bonus' ? 'badge-violet' : 'badge-red'">{{ t.type }}</span>
          </div>
        </div>
      </div>
    </template>

    <Modal :open="showForm" @close="showForm = false">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">Nouvelle transaction</h3>
        <div class="space-y-4">
          <select v-model.number="form.joueur_id" class="input-field">
            <option :value="null" disabled>Choisir un joueur</option>
            <option v-for="j in joueurs" :key="j.id" :value="j.id">{{ j.nom }} — {{ j.telephone }}</option>
          </select>
          <select v-model="form.type" class="input-field">
            <option value="gain">Gain</option>
            <option value="bonus">Bonus</option>
            <option value="depense">Dépense</option>
          </select>
          <input v-model.number="form.quantite" type="number" placeholder="Quantité" class="input-field" />
          <input v-model="form.raison" placeholder="Raison" class="input-field" />
          <div class="flex gap-3">
            <button @click="showForm = false" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="createTransaction" :disabled="!form.joueur_id || !form.quantite" class="btn-neon-violet flex-1">Créer</button>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="showDetail" @close="showDetail = false">
      <div class="p-6" v-if="selected">
        <h3 class="font-gaming text-xl font-bold mb-4">Transaction #{{ selected.id }}</h3>
        <div class="space-y-3">
          <div class="flex justify-between"><span class="text-txt-dim">Joueur</span><span>{{ selected.joueur_nom || '#' + selected.joueur_id }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Type</span><span class="badge" :class="selected.type === 'gain' ? 'badge-green' : selected.type === 'bonus' ? 'badge-violet' : 'badge-red'">{{ selected.type }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Quantité</span><span class="font-gaming font-bold text-neon-yellow">{{ selected.quantite }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Raison</span><span>{{ selected.raison || '—' }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Date</span><span class="text-sm">{{ formatDate(selected.created_at) }}</span></div>
        </div>
        <button @click="showDetail = false" class="btn-neon-outline w-full mt-6">Fermer</button>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { api } from '@/utils/api'
import { toast } from 'sonner'
import { formatDate } from '@/utils/helpers'
import Modal from '@/components/ui/Modal.vue'
import Loader from '@/components/ui/Loader.vue'
import { Coins, Plus, Search } from 'lucide-vue-next'
import { isValidId, isValidJetonType, isValidQuantite, sanitizeInput } from '@/utils/validators'

const loading = ref(true)
const transactions = ref([])
const joueurs = ref([])
const search = ref('')
const showForm = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const form = reactive({ joueur_id: null, type: 'gain', quantite: 1, raison: '' })

const totalJetons = computed(() => transactions.value.filter(t => t.type !== 'depense').reduce((s, t) => s + (t.quantite || 0), 0))
const filtered = computed(() => {
  if (!search.value) return transactions.value
  const q = search.value.toLowerCase()
  return transactions.value.filter(t => (t.joueur_nom && t.joueur_nom.toLowerCase().includes(q)) || (t.raison && t.raison.toLowerCase().includes(q)))
})

async function fetchData() {
  loading.value = true
  try {
    transactions.value = await api.get('/jetons')
    try { joueurs.value = await api.get('/joueurs') } catch {}
  } catch {} finally { loading.value = false }
}

function openAdd() { Object.assign(form, { joueur_id: null, type: 'gain', quantite: 1, raison: '' }); showForm.value = true }

async function createTransaction() {
  if (!isValidId(form.joueur_id)) return toast.error('Joueur invalide')
  if (!isValidJetonType(form.type)) return toast.error('Type invalide')
  if (!isValidQuantite(form.quantite)) return toast.error('Quantité invalide (1-10000)')
  if (form.raison) form.raison = sanitizeInput(form.raison, 500)
  try { await api.post('/jetons', { ...form }); toast.success('Transaction créée'); showForm.value = false; fetchData() }
  catch (e) { toast.error(e.message) }
}

async function viewDetail(t) {
  try { selected.value = await api.get(`/jetons/${t.id}`); if (!selected.value.joueur_nom) selected.value.joueur_nom = t.joueur_nom } catch { selected.value = t }
  showDetail.value = true
}

onMounted(fetchData)
</script>
