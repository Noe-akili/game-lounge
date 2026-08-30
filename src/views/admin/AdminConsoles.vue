<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Gestion des consoles</h3>
      <button @click="openAdd" class="btn-neon-violet flex items-center gap-2">
        <Plus class="w-4 h-4" /> Ajouter
      </button>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des consoles..." />
    </div>

    <div v-else-if="consoles.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Monitor class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucune console</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="c in consoles" :key="c.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap">
        <Monitor class="w-8 h-8 shrink-0" :class="statusColor(c.etat)" />
        <div class="flex-1 min-w-0 overflow-hidden">
          <p class="font-medium truncate">{{ c.nom }}</p>
          <p class="text-xs text-txt-dim truncate">{{ c.type }} — Poste {{ c.poste_numero }}</p>
        </div>
        <span class="badge shrink-0 max-w-full truncate" :class="statusBadge(c.etat)">{{ statusLabel(c.etat) }}</span>
        <div class="flex gap-1 shrink-0 flex-wrap">
          <button @click="editConsole(c)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors">
            <Pencil class="w-4 h-4" />
          </button>
          <button @click="deleteConsole(c.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <Modal :open="showForm" @close="showForm = false">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Ajouter' }} une console</h3>
        <div class="space-y-4">
          <input v-model="form.nom" placeholder="Nom (ex: PS5 - Poste 1)" class="input-field" />
          <select v-model="form.type" class="input-field">
            <option value="PS5">PS5</option><option value="PS4">PS4</option><option value="XBOX">XBOX</option><option value="PC">PC</option><option value="SWITCH">SWITCH</option>
          </select>
          <input v-model.number="form.poste_numero" type="number" placeholder="N° de poste" class="input-field" />
          <select v-model="form.etat" class="input-field">
            <option value="disponible">Disponible</option><option value="maintenance">Maintenance</option><option value="hors_service">Hors service</option>
          </select>
          <div class="flex gap-3">
            <button @click="showForm = false" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveConsole" :disabled="!form.nom || !form.poste_numero" class="btn-neon-violet flex-1">
              {{ editingId ? 'Modifier' : 'Créer' }}
            </button>
          </div>
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
import Modal from '@/components/ui/Modal.vue'
import { Plus, Monitor, Pencil, Trash2 } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { isValidNom, isValidConsoleType, isValidPosteNumero, sanitizeInput } from '@/utils/validators'

const consoles = ref([])
const loading = ref(true)
const showForm = ref(false)
const editingId = ref(null)
const form = reactive({ nom: '', type: 'PS5', poste_numero: 1, etat: 'disponible' })

function statusColor(e) { return { disponible: 'text-neon-green', occupee: 'text-neon-red', pause: 'text-neon-yellow', maintenance: 'text-neon-yellow', hors_service: 'text-neon-red' }[e] || 'text-txt-dim' }
function statusBadge(e) { return { disponible: 'badge-green', occupee: 'badge-red', pause: 'badge-yellow', maintenance: 'badge-yellow', hors_service: 'badge-red' }[e] || 'badge-violet' }
function statusLabel(e) { return { disponible: 'Libre', occupee: 'Occupé', pause: 'En pause', maintenance: 'Maintenance', hors_service: 'Hors service' }[e] || e }

async function fetchData() {
  loading.value = true
  try { consoles.value = await api.get('/consoles') } catch {} finally { loading.value = false }
}

function openAdd() { editingId.value = null; form.nom = ''; form.type = 'PS5'; form.poste_numero = 1; form.etat = 'disponible'; showForm.value = true }
function editConsole(c) { editingId.value = c.id; form.nom = c.nom; form.type = c.type; form.poste_numero = c.poste_numero; form.etat = c.etat; showForm.value = true }

async function saveConsole() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères, lettres/chiffres/ -\'&)')
  if (!isValidConsoleType(form.type)) return toast.error('Type de console invalide')
  if (!isValidPosteNumero(form.poste_numero)) return toast.error('Numéro de poste invalide (1-100)')
  form.nom = sanitizeInput(form.nom, 50)
  try {
    if (editingId.value) { await api.put(`/consoles/${editingId.value}`, { ...form }); toast.success('Console modifiée') }
    else { await api.post('/consoles', { ...form }); toast.success('Console créée') }
    showForm.value = false; fetchData()
  } catch (e) { toast.error(e.message) }
}

async function deleteConsole(id) {
  if (!confirm('Supprimer cette console ?')) return
  try { await api.delete(`/consoles/${id}`); toast.success('Console supprimée'); fetchData() }
  catch (e) { toast.error(e.message) }
}

onMounted(fetchData)
</script>
