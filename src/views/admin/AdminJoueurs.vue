<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Gestion des joueurs</h3>
      <button @click="showAdd = true" class="btn-neon-violet flex items-center gap-2">
        <UserPlus class="w-4 h-4" /> Ajouter
      </button>
    </div>

    <div class="relative mb-4 w-full max-w-full min-w-0 overflow-hidden">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim shrink-0" />
      <input v-model="search" @input="onSearch" placeholder="Rechercher..." class="input-field pl-10 w-full max-w-full min-w-0" />
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des joueurs..." />
    </div>

    <div v-else-if="joueurs.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Users class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun joueur</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="j in joueurs" :key="j.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewJoueur(j)">
        <div class="w-11 h-11 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold shrink-0">
          {{ j.nom?.charAt(0) }}
        </div>
        <div class="flex-1 min-w-0 overflow-hidden">
          <p class="font-medium truncate">{{ j.nom }}</p>
          <p class="text-xs text-txt-dim truncate">{{ j.telephone }} · {{ j.email }}</p>
        </div>
        <div class="text-right shrink-0 min-w-0">
          <p class="font-gaming font-bold text-neon-yellow truncate">{{ j.jetons_solde || 0 }} jetons</p>
        </div>
        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <button @click="editJoueur(j)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
            <Pencil class="w-4 h-4" />
          </button>
          <button @click="deleteJoueur(j.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Supprimer">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <Modal :open="showAdd || editingJoueur" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingJoueur ? 'Modifier' : 'Nouveau' }} joueur</h3>
        <div class="space-y-4">
          <input v-model="form.nom" placeholder="Nom complet" class="input-field" />
          <input v-model="form.telephone" placeholder="Téléphone" class="input-field" />
          <input v-model="form.email" placeholder="Email (optionnel)" class="input-field" />
          <div v-if="editingJoueur">
            <label class="text-sm text-txt-muted">Jetons</label>
            <input v-model.number="form.jetons_solde" type="number" class="input-field" />
          </div>
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveJoueur" :disabled="!form.nom" class="btn-neon-violet flex-1">{{ editingJoueur ? 'Modifier' : 'Créer' }}</button>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="showDetail" @close="showDetail = false" size="lg">
      <div class="p-6" v-if="detailJoueur">
        <div class="flex flex-col sm:flex-row sm:items-center gap-4 mb-6 w-full max-w-full min-w-0 overflow-hidden">
          <div class="w-14 h-14 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold text-xl shrink-0">
            {{ detailJoueur.nom?.charAt(0) }}
          </div>
          <div class="flex-1 min-w-0 overflow-hidden">
            <h3 class="font-gaming text-xl font-bold truncate">{{ detailJoueur.nom }}</h3>
            <p class="text-sm text-txt-dim truncate">{{ detailJoueur.telephone }} · {{ detailJoueur.email }}</p>
          </div>
          <div class="sm:ml-auto text-right shrink-0">
            <p class="font-gaming text-2xl font-bold text-neon-yellow">{{ detailJoueur.jetons_solde || 0 }}</p>
            <p class="text-xs text-txt-dim">jetons</p>
          </div>
        </div>
        <div class="grid grid-cols-2 gap-2 sm:gap-4 mb-6 w-full max-w-full min-w-0">
          <div class="stat-card">
            <span class="stat-value text-neon-blue text-xl">{{ detailData.sessions?.length || 0 }}</span>
            <span class="stat-label">Sessions totales</span>
          </div>
          <div class="stat-card">
            <span class="stat-value text-neon-green text-xl">{{ detailData.factures?.length || 0 }}</span>
            <span class="stat-label">Factures</span>
          </div>
        </div>
        <h4 class="font-gaming font-bold text-sm text-txt-muted mb-3">DERNIÈRES SESSIONS</h4>
        <div class="space-y-2 max-h-48 overflow-y-auto mb-4 w-full max-w-full min-w-0 overflow-hidden">
          <div v-for="s in detailData.sessions?.slice(0, 10)" :key="s.id" class="flex flex-col sm:flex-row sm:items-center justify-between gap-1 p-2 rounded-lg bg-bg-surface text-sm w-full max-w-full min-w-0 overflow-hidden">
            <span>{{ s.console_nom }} — {{ s.jeu_nom }}</span>
            <span class="text-txt-dim">{{ formatCurrency(s.montant) }}</span>
          </div>
          <p v-if="!detailData.sessions?.length" class="text-txt-dim text-sm text-center py-2">Aucune session</p>
        </div>
        <div class="flex gap-3">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <button @click="editJoueur(detailJoueur); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2"><Pencil class="w-4 h-4" /> Modifier</button>
        </div>
      </div>
    </Modal>
    <!-- responsive table overflow helper: ensures horizontal scroll on mobile -->
    <div class="w-full overflow-x-auto -mx-4 sm:mx-0 hidden" aria-hidden="true"><table class="min-w-[600px] w-full"><tbody><tr><td></td></tr></tbody></table></div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'
import Modal from '@/components/ui/Modal.vue'
import { UserPlus, Pencil, Search, Users, Trash2 } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { formatCurrency } from '@/utils/helpers'
import { isValidNom, isValidPhone, isValidEmail, sanitizeInput } from '@/utils/validators'

const joueurs = ref([])
const loading = ref(true)
const search = ref('')
const showAdd = ref(false)
const editingJoueur = ref(null)
const showDetail = ref(false)
const detailJoueur = ref(null)
const detailData = ref({})
const form = reactive({ nom: '', telephone: '', email: '', jetons_solde: 0 })

let timeout = null
function onSearch() {
  clearTimeout(timeout)
  timeout = setTimeout(async () => {
    loading.value = true
    try { joueurs.value = await api.get(`/joueurs?search=${search.value}`) } catch {} finally { loading.value = false }
  }, 300)
}

async function fetchData() {
  loading.value = true
  try { joueurs.value = await api.get('/joueurs') } catch {} finally { loading.value = false }
}

function closeForm() { showAdd.value = false; editingJoueur.value = null }

function editJoueur(j) { editingJoueur.value = j.id; form.nom = j.nom; form.telephone = j.telephone; form.email = j.email; form.jetons_solde = j.jetons_solde || 0; showAdd.value = false }

async function viewJoueur(j) {
  detailJoueur.value = j
  showDetail.value = true
  try { detailData.value = await api.get(`/joueurs/${j.id}/historique`) }
  catch { detailData.value = {} }
}

async function saveJoueur() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères, lettres/chiffres/ -\'&)')
  if (form.telephone && !isValidPhone(form.telephone)) return toast.error('Téléphone invalide (8-15 chiffres, ex: +243...)')
  if (form.email && !isValidEmail(form.email)) return toast.error('Email invalide')
  form.nom = sanitizeInput(form.nom, 50)
  if (form.email) form.email = sanitizeInput(form.email, 100)
  try {
    if (editingJoueur.value) { await api.put(`/joueurs/${editingJoueur.value}`, { ...form }); toast.success('Joueur modifié') }
    else { await api.post('/joueurs', { nom: form.nom, telephone: form.telephone, email: form.email }); toast.success('Joueur créé') }
    closeForm(); form.nom = ''; form.telephone = ''; form.email = ''; form.jetons_solde = 0
    fetchData()
  } catch (e) { toast.error(e.message) }
}

async function deleteJoueur(id) {
  if (!confirm('Supprimer ce joueur ?')) return
  try { await api.delete(`/joueurs/${id}`); toast.success('Joueur supprimé'); fetchData() }
  catch (e) { toast.error(e.message) }
}

onMounted(fetchData)
</script>
