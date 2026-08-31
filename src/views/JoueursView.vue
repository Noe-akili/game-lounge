<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between gap-4">
      <div class="relative flex-1 max-w-md">
        <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim" />
        <input v-model="search" @input="onSearch" placeholder="Rechercher un joueur..." class="input-field pl-10" />
      </div>
      <button @click="showNew = true" class="btn-neon-violet flex items-center gap-2 shrink-0">
        <UserPlus class="w-4 h-4" /> Ajouter
      </button>
    </div>

    <div v-if="loading" class="card">
      <Loader variant="neon" size="lg" text="Chargement des joueurs..." />
    </div>

    <div v-else-if="joueurs.length === 0" class="card text-center py-12">
      <Users class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun joueur trouvé</p>
    </div>

    <div v-else class="space-y-2">
      <div v-for="j in joueurs" :key="j.id" class="card flex items-center gap-4 hover:border-neon-violet/20 transition-all cursor-pointer" @click="openDetail(j)">
        <div class="w-11 h-11 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold shrink-0">
          {{ j.nom?.charAt(0) }}
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium truncate">{{ j.nom }}</p>
          <p class="text-xs text-txt-dim">{{ j.telephone }} · {{ j.email }}</p>
        </div>
        <div class="text-right shrink-0">
          <p class="font-gaming font-bold text-neon-yellow">{{ j.jetons_solde || 0 }} jetons</p>
          <p class="text-xs text-txt-dim">{{ timeAgo(j.derniere_visite) || 'Jamais' }}</p>
        </div>
        <div class="flex gap-1 shrink-0" @click.stop>
          <button @click="editJoueur(j)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
            <Pencil class="w-4 h-4" />
          </button>
          <button @click="deleteJoueur(j.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Supprimer">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <Modal :open="showNew || editingId" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Nouveau' }} joueur</h3>
        <div class="space-y-4">
          <input v-model="form.nom" placeholder="Nom complet" class="input-field" />
          <input v-model="form.telephone" placeholder="Téléphone" class="input-field" />
          <input v-model="form.email" placeholder="Email (optionnel)" class="input-field" />
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveJoueur" :disabled="!form.nom" class="btn-neon-violet flex-1">{{ editingId ? 'Modifier' : 'Créer' }}</button>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="showDetail" @close="showDetail = false" size="lg">
      <div class="p-6" v-if="detailJoueur">
        <div class="flex items-center gap-4 mb-6">
          <div class="w-14 h-14 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold text-xl">
            {{ detailJoueur.nom?.charAt(0) }}
          </div>
          <div>
            <h3 class="font-gaming text-xl font-bold">{{ detailJoueur.nom }}</h3>
            <p class="text-sm text-txt-dim">{{ detailJoueur.telephone }} · {{ detailJoueur.email }}</p>
          </div>
          <div class="ml-auto text-right">
            <p class="font-gaming text-2xl font-bold text-neon-yellow">{{ detailJoueur.jetons_solde || 0 }}</p>
            <p class="text-xs text-txt-dim">jetons</p>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4 mb-6">
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
        <div class="space-y-2 max-h-48 overflow-y-auto mb-4">
          <div v-for="s in detailData.sessions?.slice(0, 10)" :key="s.id" class="flex items-center justify-between p-2 rounded-lg bg-bg-surface text-sm">
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
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { api } from '@/utils/api'
import { formatCurrency, timeAgo } from '@/utils/helpers'
import { toast } from 'sonner'
import Modal from '@/components/ui/Modal.vue'
import Loader from '@/components/ui/Loader.vue'
import { Search, UserPlus, Users, Pencil, Trash2 } from 'lucide-vue-next'
import { isValidNom, isValidPhone, isValidEmail, sanitizeInput } from '@/utils/validators'

const joueurs = ref([])
const loading = ref(false)
const search = ref('')
const showNew = ref(false)
const showDetail = ref(false)
const detailJoueur = ref(null)
const detailData = ref({})
const editingId = ref(null)
const form = reactive({ nom: '', telephone: '', email: '' })

async function onSearch() {
  clearTimeout(timeout)
  timeout = setTimeout(async () => {
    loading.value = true
    try { joueurs.value = await api.get(`/joueurs?search=${search.value}`) }
    catch (e: any) { toast.error('Erreur chargement: ' + (e.message || 'Serveur indisponible')) }
    finally { loading.value = false }
  }, 300)
}

async function fetchJoueurs() {
  loading.value = true
  try { joueurs.value = await api.get('/joueurs') }
  catch (e: any) { toast.error('Erreur chargement: ' + (e.message || 'Serveur indisponible')) }
  finally { loading.value = false }
}

function closeForm() { showNew.value = false; editingId.value = null }
function editJoueur(j) { editingId.value = j.id; Object.assign(form, { nom: j.nom, telephone: j.telephone, email: j.email || '' }); showNew.value = false }

async function saveJoueur() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères, lettres/chiffres/ -\'&)')
  if (form.telephone && !isValidPhone(form.telephone)) return toast.error('Téléphone invalide (8-15 chiffres, ex: +243...)')
  if (form.email && !isValidEmail(form.email)) return toast.error('Email invalide')
  form.nom = sanitizeInput(form.nom, 50)
  if (form.email) form.email = sanitizeInput(form.email, 100)
  try {
    if (editingId.value) { await api.put(`/joueurs/${editingId.value}`, { ...form }); toast.success('Joueur modifié') }
    else { await api.post('/joueurs', form); toast.success('Joueur créé !') }
    closeForm(); Object.assign(form, { nom: '', telephone: '', email: '' }); fetchJoueurs()
  } catch (e) { toast.error(e.message) }
}

async function createJoueur() { return saveJoueur() }

async function deleteJoueur(id) {
  if (!confirm('Supprimer ce joueur ?')) return
  try { await api.delete(`/joueurs/${id}`); toast.success('Joueur supprimé'); fetchJoueurs() }
  catch (e) { toast.error(e.message) }
}

async function openDetail(j) {
  try { detailJoueur.value = await api.get(`/joueurs/${j.id}`) } catch { detailJoueur.value = j }
  showDetail.value = true
  try { detailData.value = await api.get(`/joueurs/${j.id}/historique`) }
  catch { detailData.value = {} }
}

onMounted(fetchJoueurs)
</script>
