<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full max-w-full min-w-0">
      <div>
        <h3 class="font-gaming text-lg font-bold">Gestion des joueurs</h3>
        <p class="text-xs text-txt-dim">Consultez, modifiez ou restaurez les joueurs</p>
      </div>
      <div class="flex items-center gap-3 flex-wrap">
        <label class="flex items-center gap-2 cursor-pointer text-xs text-txt-dim bg-bg-card px-3 py-1.5 rounded-lg border border-border/50 hover:border-neon-violet/30 transition-colors">
          <input type="checkbox" v-model="showArchived" @change="fetchData" class="rounded accent-neon-violet cursor-pointer" />
          <span>Afficher archivés</span>
        </label>
        <button @click="showAdd = true" class="btn-neon-violet flex items-center gap-2 text-sm">
          <UserPlus class="w-4 h-4" /> Ajouter
        </button>
      </div>
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
      <p class="text-txt-dim">Aucun joueur trouvé</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="j in joueurs" :key="j.id" 
        class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap transition-colors cursor-pointer"
        :class="j.deleted ? 'opacity-65 border-dashed border-neon-red/40 bg-bg-surface/40' : 'hover:border-neon-violet/20'" 
        @click="viewJoueur(j)">
        <!-- Sticker/icône du joueur -->
        <div class="w-11 h-11 rounded-full flex items-center justify-center font-bold shrink-0 overflow-hidden"
          :class="j.deleted ? 'bg-neon-red/10 text-neon-red' : 'bg-neon-violet/20 text-neon-violet'">
          <span v-if="j.sticker" class="text-xl leading-none">{{ j.sticker }}</span>
          <template v-else>{{ j.nom?.charAt(0) }}</template>
        </div>
        <div class="flex-1 min-w-0 overflow-hidden">
          <div class="flex items-center gap-2">
            <p class="font-medium truncate" :class="{ 'line-through text-txt-dim': j.deleted }">{{ j.nom }}</p>
            <span v-if="j.deleted" class="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-neon-red/10 text-neon-red border border-neon-red/20">Archivé</span>
          </div>
          <p class="text-xs text-txt-dim truncate">{{ j.telephone }} · {{ j.email }}</p>
        </div>
        <div class="text-right shrink-0 min-w-0">
          <p class="font-gaming font-bold text-neon-yellow truncate">{{ j.jetons_solde || 0 }} jetons</p>
        </div>
        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <template v-if="j.deleted">
            <button @click="restoreJoueur(j.id)" class="p-2 rounded-lg hover:bg-neon-green/10 text-neon-green transition-colors" title="Restaurer">
              <RotateCcw class="w-4 h-4" />
            </button>
            <button @click="permanentDeleteJoueur(j.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Supprimer définitivement">
              <Trash2 class="w-4 h-4" />
            </button>
          </template>
          <template v-else>
            <button @click="editJoueur(j)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
              <Pencil class="w-4 h-4" />
            </button>
            <button @click="deleteJoueur(j.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Archiver">
              <Trash2 class="w-4 h-4" />
            </button>
          </template>
        </div>
      </div>
    </div>

    <Modal :open="showAdd || !!editingJoueur" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingJoueur ? 'Modifier' : 'Nouveau' }} joueur</h3>
        <div class="space-y-4">
          <input v-model="form.nom" placeholder="Nom complet" class="input-field" />
          <input v-model="form.telephone" placeholder="Téléphone" class="input-field" />
          <input v-model="form.email" placeholder="Email (optionnel)" class="input-field" />
          <div>
            <label class="text-sm text-txt-muted">Sticker / icône unique (optionnel)</label>
            <div class="flex items-center gap-2">
              <input v-model="form.sticker" placeholder="Choisir une icône ci-contre" readonly class="input-field flex-1" />
              <div class="flex gap-1" aria-label="Choisir une seule icône">
                <button v-for="e in ['🎮','👾','🕹️','🎯','🏆','⚡','🔥','⭐']" :key="e" type="button" @click="form.sticker = e"
                  class="w-8 h-8 rounded-lg bg-bg-surface hover:bg-bg-hover text-lg flex items-center justify-center">{{ e }}</button>
              </div>
            </div>
          </div>
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
          <div class="w-14 h-14 rounded-full flex items-center justify-center font-bold text-xl shrink-0 overflow-hidden"
            :class="detailJoueur.deleted ? 'bg-neon-red/10 text-neon-red' : 'bg-neon-violet/20 text-neon-violet'">
            <span v-if="detailJoueur.sticker" class="text-2xl leading-none">{{ detailJoueur.sticker }}</span>
            <template v-else>{{ detailJoueur.nom?.charAt(0) }}</template>
          </div>
          <div class="flex-1 min-w-0 overflow-hidden">
            <div class="flex items-center gap-2">
              <h3 class="font-gaming text-xl font-bold truncate">{{ detailJoueur.nom }}</h3>
              <span v-if="detailJoueur.deleted" class="text-xs uppercase font-bold tracking-wider px-2 py-0.5 rounded bg-neon-red/10 text-neon-red border border-neon-red/20">Archivé</span>
            </div>
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

        <!-- HISTORIQUE DÉTAILLÉ DES JETONS (DÉBITS & REMBOURSEMENTS) -->
        <div class="mb-4">
          <h4 class="font-gaming font-bold text-sm text-txt-muted mb-2 flex items-center gap-2">
            <Coins class="w-4 h-4 text-neon-yellow" />
            HISTORIQUE JETONS (DÉBITS & REMBOURSEMENTS)
          </h4>
          <div class="space-y-2 max-h-48 overflow-y-auto w-full max-w-full min-w-0 overflow-hidden pr-1">
            <div v-for="t in detailData.transactions" :key="t.id" class="flex items-center justify-between gap-3 p-2.5 rounded-xl bg-bg-surface border border-white/5 text-sm">
              <div class="flex items-center gap-2.5 min-w-0">
                <div class="w-7 h-7 rounded-lg flex items-center justify-center shrink-0"
                  :class="t.type === 'gain' ? 'bg-neon-green/20 text-neon-green' : t.type === 'bonus' ? 'bg-neon-violet/20 text-neon-violet' : 'bg-neon-red/20 text-neon-red'">
                  <Coins class="w-3.5 h-3.5" />
                </div>
                <div class="min-w-0">
                  <p class="font-medium truncate text-xs sm:text-sm">{{ t.raison || (t.type === 'gain' ? 'Crédit jetons' : 'Débit jetons') }}</p>
                  <p class="text-[11px] text-txt-dim">{{ formatDate(t.created_at) }}</p>
                </div>
              </div>
              <div class="text-right shrink-0">
                <p class="font-gaming font-bold text-sm" :class="t.type === 'depense' ? 'text-neon-red' : 'text-neon-yellow'">
                  {{ t.type === 'depense' ? '-' : '+' }}{{ t.quantite }}
                </p>
                <span class="text-[10px] uppercase font-bold tracking-wider" :class="t.type === 'gain' ? 'text-neon-green' : t.type === 'depense' ? 'text-neon-red' : 'text-neon-violet'">
                  {{ t.type === 'gain' ? 'Remboursement / Gain' : t.type === 'depense' ? 'Débit' : 'Bonus' }}
                </span>
              </div>
            </div>
            <p v-if="!detailData.transactions?.length" class="text-txt-dim text-xs text-center py-3 bg-bg-surface/50 rounded-xl border border-white/5">
              Aucun mouvement de jetons enregistré pour ce joueur
            </p>
          </div>
        </div>
        <div class="flex gap-3">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <template v-if="detailJoueur.deleted">
            <button @click="restoreJoueur(detailJoueur.id); showDetail = false" class="btn-neon-green flex-1 flex items-center justify-center gap-2">
              <RotateCcw class="w-4 h-4" /> Restaurer
            </button>
            <button @click="permanentDeleteJoueur(detailJoueur.id); showDetail = false" class="btn-neon-red flex-1 flex items-center justify-center gap-2">
              <Trash2 class="w-4 h-4" /> Supprimer définitif
            </button>
          </template>
          <template v-else>
            <button @click="editJoueur(detailJoueur); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
              <Pencil class="w-4 h-4" /> Modifier
            </button>
          </template>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'
import Modal from '@/components/ui/Modal.vue'
import { UserPlus, Pencil, Search, Users, Trash2, RotateCcw } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { formatCurrency } from '@/utils/helpers'
import { isValidNom, isValidPhone, isValidEmail, sanitizeInput } from '@/utils/validators'

const joueurs = ref<any[]>([])
const loading = ref(true)
const search = ref('')
const showAdd = ref(false)
const showArchived = ref(false)
const editingJoueur = ref<any>(null)
const showDetail = ref(false)
const detailJoueur = ref<any>(null)
const detailData = ref<any>({})
const form = reactive({ nom: '', telephone: '', email: '', jetons_solde: 0, sticker: '' })

let timeout: any = null
function onSearch() {
  clearTimeout(timeout)
  timeout = setTimeout(async () => {
    loading.value = true
    try {
      const qDel = showArchived.value ? '&include_deleted=1' : ''
      joueurs.value = await api.get(`/joueurs?search=${encodeURIComponent(search.value)}${qDel}`)
    } catch {} finally {
      loading.value = false
    }
  }, 300)
}

async function fetchData() {
  loading.value = true
  try {
    const q = showArchived.value ? '?include_deleted=1' : ''
    joueurs.value = await api.get(`/joueurs${q}`)
  } catch (e: any) {
    toast.error('Erreur chargement joueurs: ' + (e.message || ''))
  } finally {
    loading.value = false
  }
}

function closeForm() {
  showAdd.value = false
  editingJoueur.value = null
}

function editJoueur(j: any) {
  editingJoueur.value = j.id
  form.nom = j.nom
  form.telephone = j.telephone
  form.email = j.email
  form.jetons_solde = j.jetons_solde || 0
  form.sticker = j.sticker || ''
  showAdd.value = false
}

async function viewJoueur(j: any) {
  detailJoueur.value = j
  showDetail.value = true
  try {
    detailData.value = await api.get(`/joueurs/${j.id}/historique`)
  } catch {
    detailData.value = {}
  }
}

async function saveJoueur() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères)')
  if (form.telephone && !isValidPhone(form.telephone)) return toast.error('Téléphone invalide (8-15 chiffres, ex: +243...)')
  if (form.email && !isValidEmail(form.email)) return toast.error('Email invalide')
  form.nom = sanitizeInput(form.nom, 50)
  if (form.email) form.email = sanitizeInput(form.email, 100)
  try {
    if (editingJoueur.value) {
      await api.put(`/joueurs/${editingJoueur.value}`, { ...form })
      toast.success('Joueur modifié')
    } else {
      await api.post('/joueurs', { nom: form.nom, telephone: form.telephone, email: form.email, sticker: form.sticker })
      toast.success('Joueur créé')
    }
    closeForm()
    form.nom = ''
    form.telephone = ''
    form.email = ''
    form.jetons_solde = 0
    form.sticker = ''
    await fetchData()
  } catch (e: any) {
    toast.error(e.message)
  }
}

async function deleteJoueur(id: number) {
  if (!confirm('Archiver ce joueur ?')) return
  try {
    await api.delete(`/joueurs/${id}`)
    toast.success('Joueur archivé')
    await fetchData()
  } catch (e: any) {
    toast.error(e.message)
  }
}

async function restoreJoueur(id: number) {
  try {
    await api.post(`/joueurs/${id}/restore`, {})
    toast.success('Joueur restauré avec succès !')
    await fetchData()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de restaurer'))
  }
}

async function permanentDeleteJoueur(id: number) {
  if (!confirm('ATTENTION : Supprimer DÉFINITIVEMENT ce joueur ? Cette action est irréversible.')) return
  try {
    await api.delete(`/joueurs/${id}/permanent`)
    toast.success('Joueur supprimé définitivement')
    await fetchData()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de supprimer'))
  }
}

onMounted(fetchData)
</script>
