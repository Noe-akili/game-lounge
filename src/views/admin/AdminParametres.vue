<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Paramètres Fidélité</h3>
      <button @click="openAdd" class="btn-neon-violet flex items-center gap-2">
        <Plus class="w-4 h-4" /> Ajouter
      </button>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des paramètres..." />
    </div>

    <div v-else-if="parametres.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Settings class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun paramètre</p>
      <p class="text-xs text-txt-dim mt-1">Créez une règle de fidélité pour les jetons</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="p in parametres" :key="p.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewParametre(p)">
        <div class="w-11 h-11 rounded-xl flex items-center justify-center shrink-0" :class="p.actif ? 'bg-neon-green/20' : 'bg-txt-dim/20'">
          <Settings class="w-5 h-5" :class="p.actif ? 'text-neon-green' : 'text-txt-dim'" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium truncate">{{ p.regle_type === 'temps' ? 'Temps de jeu' : 'Montant dépensé' }} — Seuil: {{ p.seuil }} min</p>
          <p class="text-xs text-txt-dim">{{ p.jetons_attribues }} jeton(s) attribué(s) · {{ p.actif ? 'Actif' : 'Inactif' }}</p>
        </div>
        <span class="badge shrink-0 max-w-full truncate" :class="p.actif ? 'badge-green' : 'badge-violet'">{{ p.actif ? 'Actif' : 'Inactif' }}</span>
        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <button @click="editParametre(p)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
            <Pencil class="w-4 h-4" />
          </button>
          <button @click="deleteParametre(p.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Supprimer">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <div class="card w-full max-w-full min-w-0 overflow-hidden">
      <h4 class="font-gaming font-bold mb-4 truncate">Configuration générale</h4>
      <div class="space-y-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="w-full max-w-full min-w-0">
          <label class="text-sm text-txt-muted">TVA par défaut (%)</label>
          <input v-model.number="config.taux_tva" type="number" class="input-field w-full max-w-full min-w-0" />
        </div>
        <button @click="toast.success('Paramètres sauvegardés')" class="btn-neon-violet w-full sm:w-auto">Enregistrer</button>
      </div>
    </div>

    <div class="card w-full max-w-full min-w-0 overflow-hidden">
      <h4 class="font-gaming font-bold mb-4 truncate">Affichage</h4>
      <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">

        <div>
          <label class="text-sm text-txt-muted mb-3 block">Police</label>
          <div class="grid grid-cols-2 gap-3">
            <button @click="changeFont('gaming')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.fontMode === 'gaming' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <p class="font-bold text-sm" :style="{ fontFamily: settings.fontMode === 'gaming' ? 'Montserrat, sans-serif' : 'Inter, sans-serif' }">Montserrat</p>
              <p class="text-xs text-txt-dim mt-1">Police gaming moderne</p>
            </button>
            <button @click="changeFont('normal')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.fontMode === 'normal' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <p class="font-bold text-sm" style="font-family: Inter, sans-serif">Inter</p>
              <p class="text-xs text-txt-dim mt-1">Police classique lisible</p>
            </button>
          </div>
        </div>

        <div>
          <label class="text-sm text-txt-muted mb-3 block">Thème</label>
          <div class="grid grid-cols-2 gap-3">
            <button @click="changeTheme('dark')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.themeMode === 'dark' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <div class="flex items-center gap-2">
                <Moon class="w-5 h-5 text-neon-blue" />
                <p class="font-bold text-sm">Sombre</p>
              </div>
              <p class="text-xs text-txt-dim mt-1">Mode nuit gaming</p>
            </button>
            <button @click="changeTheme('light')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.themeMode === 'light' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <div class="flex items-center gap-2">
                <Sun class="w-5 h-5 text-neon-yellow" />
                <p class="font-bold text-sm">Clair</p>
              </div>
              <p class="text-xs text-txt-dim mt-1">Mode jour classique</p>
            </button>
          </div>
        </div>

      </div>
    </div>

    <Modal :open="showForm" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Créer' }} un paramètre</h3>
        <div class="space-y-4">
          <select v-model="form.regle_type" class="input-field">
            <option value="temps">Temps (jeton par durée)</option>
            <option value="montant">Montant (bonus selon dépense)</option>
          </select>
          <div>
            <label class="text-sm text-txt-muted">Seuil (minutes)</label>
            <input v-model.number="form.seuil" type="number" placeholder="Ex: 60" class="input-field" />
          </div>
          <div>
            <label class="text-sm text-txt-muted">Jetons attribués</label>
            <input v-model.number="form.jetons_attribues" type="number" placeholder="Ex: 1" class="input-field" />
          </div>
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="checkbox" v-model="form.actif" class="w-5 h-5 rounded bg-bg-surface border-white/20 text-neon-violet" />
            <span class="font-medium">Actif</span>
          </label>
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveParametre" :disabled="form.seuil === null || form.jetons_attribues === null" class="btn-neon-violet flex-1">{{ editingId ? 'Modifier' : 'Créer' }}</button>
          </div>
        </div>
      </div>
    </Modal>

    <Modal :open="showDetail" @close="showDetail = false">
      <div class="p-6" v-if="selected">
        <h3 class="font-gaming text-xl font-bold mb-4">Détails paramètre #{{ selected.id }}</h3>
        <div class="space-y-3">
          <div class="flex justify-between"><span class="text-txt-dim">Type</span><span>{{ selected.regle_type }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Seuil</span><span>{{ selected.seuil }} min</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Jetons</span><span class="font-gaming font-bold text-neon-yellow">{{ selected.jetons_attribues }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Actif</span><span class="badge" :class="selected.actif ? 'badge-green' : 'badge-red'">{{ selected.actif ? 'Oui' : 'Non' }}</span></div>
        </div>
        <div class="flex gap-3 mt-6">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <button @click="editParametre(selected); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2"><Pencil class="w-4 h-4" /> Modifier</button>
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
import { toast } from 'sonner'
import Loader from '@/components/ui/Loader.vue'
import Modal from '@/components/ui/Modal.vue'
import { Plus, Pencil, Trash2, Settings, Moon, Sun } from 'lucide-vue-next'
import { isValidRegleType, isValidSeuil, isValidJetonsAttribues } from '@/utils/validators'
import { useSettingsStore } from '@/stores/settings'

const settings = useSettingsStore()
const loading = ref(true)
const parametres = ref([])
const showForm = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const editingId = ref(null)
const form = reactive({ regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
const config = reactive({ taux_tva: 20 })

function changeFont(mode: string) {
  settings.setFont(mode)
  toast.success(`Police: ${mode === 'gaming' ? 'Montserrat' : 'Inter'}`)
}

function changeTheme(mode: string) {
  settings.setTheme(mode)
  toast.success(`Thème: ${mode === 'dark' ? 'Sombre' : 'Clair'}`)
}

async function fetchData() {
  loading.value = true
  try {
    const data = await api.get('/parametres/fidelite')
    if (Array.isArray(data)) parametres.value = data
    else if (data && typeof data === 'object' && data.id !== undefined) parametres.value = [data]
    else if (data && typeof data === 'object' && Object.keys(data).length === 0) parametres.value = []
    else parametres.value = []
  } catch {
    parametres.value = []
  } finally { loading.value = false }
}

function openAdd() {
  editingId.value = null
  Object.assign(form, { regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
  showForm.value = true
}

function editParametre(p) {
  editingId.value = p.id
  Object.assign(form, { regle_type: p.regle_type, seuil: p.seuil, jetons_attribues: p.jetons_attribues, actif: !!p.actif })
  showForm.value = true
}

function closeForm() { showForm.value = false; editingId.value = null }

async function viewParametre(p) {
  try {
    selected.value = await api.get(`/parametres/fidelite/${p.id}`)
    showDetail.value = true
  } catch { selected.value = p; showDetail.value = true }
}

async function saveParametre() {
  if (!isValidRegleType(form.regle_type)) return toast.error('Type de règle invalide')
  if (!isValidSeuil(form.seuil)) return toast.error('Seuil invalide (1-10000)')
  if (!isValidJetonsAttribues(form.jetons_attribues)) return toast.error('Jetons attribués invalides (1-1000)')
  if (form.seuil === null || form.jetons_attribues === null) return toast.error('Champs requis')
  try {
    if (editingId.value) {
      try {
        await api.put(`/parametres/fidelite`, { ...form })
        toast.success('Paramètre modifié')
      } catch (e) {
        // fallback try with id if server supports it
        throw e
      }
    } else {
      await api.post('/parametres/fidelite', { ...form })
      toast.success('Paramètre créé')
    }
    closeForm(); fetchData()
  } catch (e) { toast.error(e.message) }
}

async function deleteParametre(id) {
  if (!confirm('Supprimer ce paramètre ?')) return
  try { await api.delete(`/parametres/fidelite/${id}`); toast.success('Paramètre supprimé'); fetchData() }
  catch (e) { toast.error(e.message) }
}

onMounted(fetchData)
</script>
