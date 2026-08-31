<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Tarifs</h3>
      <button @click="showAdd = true" class="btn-neon-violet flex items-center gap-2">
        <Plus class="w-4 h-4" /> Ajouter
      </button>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des tarifs..." />
    </div>

    <div v-else class="space-y-4 w-full max-w-full min-w-0 overflow-hidden">
      <div class="flex items-center gap-2">
        <select v-model="filterConsole" class="input-field w-32 text-sm py-2">
          <option value="">Tous consoles</option>
          <option value="PS4">PS4</option>
          <option value="PS5">PS5</option>
        </select>
        <select v-model="filterJeu" class="input-field flex-1 text-sm py-2">
          <option value="">Tous jeux</option>
          <option v-for="jeu in uniqueJeux" :key="jeu" :value="jeu">{{ jeu }}</option>
        </select>
      </div>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
        <div v-for="t in filteredTarifs" :key="t.id" class="card flex flex-col gap-2 p-3 w-full max-w-full min-w-0 overflow-hidden hover:border-neon-violet/20 transition-colors">
          <div class="flex items-start justify-between gap-2">
            <div class="flex-1 min-w-0">
              <p class="font-medium truncate text-sm">{{ t.description }}</p>
              <p class="text-xs text-txt-dim truncate">{{ t.console_type || '' }} · {{ t.jeu || '' }} · {{ t.type }} · {{ t.duree_minutes }}min</p>
            </div>
            <span class="font-gaming font-bold text-neon-green shrink-0">{{ formatCurrency(t.prix) }}</span>
          </div>
          <div class="flex items-center gap-2 justify-end">
            <span class="badge shrink-0" :class="t.actif ? 'badge-green' : 'badge-red'">{{ t.actif ? 'Actif' : 'Inactif' }}</span>
            <button @click="editTarif(t)" class="p-1.5 rounded-lg hover:bg-bg-hover text-txt-dim shrink-0"><Pencil class="w-3.5 h-3.5" /></button>
            <button @click="deleteTarif(t.id)" class="p-1.5 rounded-lg hover:bg-neon-red/10 text-neon-red shrink-0"><Trash2 class="w-3.5 h-3.5" /></button>
          </div>
        </div>
      </div>
      <p v-if="filteredTarifs.length === 0" class="text-center text-txt-dim py-6">Aucun tarif</p>
    </div>

    <Modal :open="showAdd || editing" @close="showAdd = false; editing = null">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editing ? 'Modifier' : 'Nouveau' }} tarif</h3>
        <div class="space-y-4">
          <select v-model="form.type" class="input-field"><option value="partie">Partie</option><option value="session">Session</option><option value="horaire">Par heure</option><option value="forfait">Forfait</option></select>
          <div class="grid grid-cols-2 gap-3">
            <select v-model="form.console_type" class="input-field"><option value="PS4">PS4</option><option value="PS5">PS5</option></select>
            <select v-model="form.jeu" class="input-field">
              <option value="">Aucun jeu</option>
              <option v-for="j in filteredJeux" :key="j.id" :value="j.titre">{{ j.titre }}</option>
            </select>
          </div>
          <input v-model.number="form.duree_minutes" type="number" placeholder="Durée (minutes)" class="input-field" />
          <input v-model.number="form.prix" type="number" placeholder="Prix (FC)" class="input-field" />
          <input v-model="form.description" placeholder="Description" class="input-field" />
          <div class="flex gap-3">
            <button @click="showAdd = false; editing = null" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveTarif" :disabled="!form.duree_minutes || !form.prix" class="btn-neon-violet flex-1">{{ editing ? 'Modifier' : 'Créer' }}</button>
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
import { formatCurrency } from '@/utils/helpers'
import { toast } from 'sonner'
import Modal from '@/components/ui/Modal.vue'
import { Plus, Pencil, Trash2 } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { isValidTarifType, isValidDuree, isValidPrix, sanitizeInput } from '@/utils/validators'

const tarifs = ref([])
const jeux = ref([])
const loading = ref(true)
const showAdd = ref(false)
const editing = ref(null)
const filterConsole = ref('')
const filterJeu = ref('')
const form = reactive({ type: 'session', duree_minutes: 60, prix: 2000, description: '', console_type: 'PS5', jeu: '' })

const filteredJeux = computed(() => {
  if (!form.console_type) return jeux.value
  return jeux.value.filter((j: any) => !j.console_id || j.console_id === null || true)
})

const filteredTarifs = computed(() => {
  return tarifs.value.filter((t: any) => {
    if (filterConsole.value && t.console_type !== filterConsole.value) return false
    if (filterJeu.value && t.jeu !== filterJeu.value) return false
    return true
  })
})

const uniqueJeux = computed(() => {
  const set = new Set(tarifs.value.map((t: any) => t.jeu).filter(Boolean))
  return Array.from(set).sort()
})

async function fetchData() {
  loading.value = true
  try {
    const [tarifsData, jeuxData] = await Promise.all([
      api.get('/tarifs'),
      api.get('/jeux')
    ])
    tarifs.value = tarifsData
    jeux.value = jeuxData
  } catch (e: any) { toast.error('Erreur chargement: ' + (e.message || 'Serveur indisponible')) }
  finally { loading.value = false }
}
function editTarif(t) { editing.value = t.id; form.type = t.type; form.duree_minutes = t.duree_minutes; form.prix = t.prix; form.description = t.description || ''; form.console_type = t.console_type || 'PS5'; form.jeu = t.jeu || ''; showAdd.value = false }

async function saveTarif() {
  if (!isValidTarifType(form.type)) return toast.error('Type de tarif invalide')
  if (!isValidDuree(form.duree_minutes)) return toast.error('Durée invalide (1-1000 minutes)')
  if (!isValidPrix(form.prix)) return toast.error('Prix invalide (1-1000000)')
  if (form.description) form.description = sanitizeInput(form.description, 500)
  try {
    if (editing.value) { await api.put(`/tarifs/${editing.value}`, { ...form }); toast.success('Tarif modifié') }
    else { await api.post('/tarifs', { ...form }); toast.success('Tarif créé') }
    showAdd.value = false; editing.value = null; Object.assign(form, { type: 'session', duree_minutes: 60, prix: 2000, description: '', console_type: 'PS5', jeu: '' })
    fetchData()
  } catch (e: any) { toast.error(e.message) }
}

async function deleteTarif(id) {
  if (!confirm('Supprimer ce tarif ?')) return
  try { await api.delete(`/tarifs/${id}`); toast.success('Supprimé'); fetchData() }
  catch (e) { toast.error(e.message) }
}

onMounted(fetchData)
</script>
