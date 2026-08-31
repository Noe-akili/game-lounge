<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Catalogue des jeux</h3>
      <button @click="showAdd = true" class="btn-neon-violet flex items-center gap-2">
        <Plus class="w-4 h-4" /> Ajouter un jeu
      </button>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des jeux..." />
    </div>

    <div v-else-if="jeux.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Gamepad2 class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun jeu</p>
    </div>

    <div v-else class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="j in jeux" :key="j.id" class="card-hover text-center group w-full max-w-full min-w-0 overflow-hidden flex flex-col">
        <div class="w-20 h-20 mx-auto rounded-xl bg-bg-surface flex items-center justify-center mb-3 overflow-hidden">
          <img v-if="j.jaquette_url" :src="j.jaquette_url" class="w-full h-full object-cover rounded-xl" />
          <Gamepad2 v-else class="w-10 h-10 text-txt-dim" />
        </div>
        <p class="font-medium text-sm truncate w-full max-w-full">{{ j.titre }}</p>
        <p class="text-xs text-txt-dim truncate w-full max-w-full">{{ j.genre }}</p>
        <p class="text-xs text-txt-dim mt-1 truncate w-full max-w-full">{{ consoleName(j.console_id) }}</p>
        <div class="flex gap-2 mt-3 justify-center opacity-0 group-hover:opacity-100 transition-opacity flex-wrap shrink-0">
          <button @click="editJeu(j)" class="p-1.5 rounded-lg hover:bg-bg-hover text-txt-dim"><Pencil class="w-3.5 h-3.5" /></button>
          <button @click="deleteJeu(j.id)" class="p-1.5 rounded-lg hover:bg-neon-red/10 text-neon-red"><Trash2 class="w-3.5 h-3.5" /></button>
        </div>
      </div>
    </div>

    <Modal :open="showAdd || editingJeu" @close="showAdd = false; editingJeu = null">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingJeu ? 'Modifier' : 'Ajouter' }} un jeu</h3>
        <div class="space-y-4">
          <input v-model="form.titre" placeholder="Titre du jeu" class="input-field" />
          <input v-model="form.genre" placeholder="Genre" class="input-field" />
          <select v-model="form.console_id" class="input-field">
            <option :value="null">Choisir une console</option>
            <option v-for="c in consolesList" :key="c.id" :value="c.id">{{ c.nom }}</option>
          </select>
          <input v-model="form.jaquette_url" placeholder="URL jaquette (optionnel)" class="input-field" />
          <div class="flex gap-3">
            <button @click="showAdd = false; editingJeu = null" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveJeu" :disabled="!form.titre" class="btn-neon-violet flex-1">{{ editingJeu ? 'Modifier' : 'Créer' }}</button>
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
import { Plus, Gamepad2, Pencil, Trash2 } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { isValidTitre, isValidGenre, sanitizeInput } from '@/utils/validators'

const jeux = ref([])
const consolesList = ref([])
const loading = ref(true)
const showAdd = ref(false)
const editingJeu = ref(null)
const form = reactive({ titre: '', genre: '', console_id: null, jaquette_url: '' })

function consoleName(id) { return consolesList.value.find(c => c.id === id)?.nom || 'N/A' }

async function fetchData() {
  loading.value = true
  try { jeux.value = await api.get('/jeux') }
  catch (e: any) { toast.error('Jeux: ' + (e.message || 'erreur')) }
  try { consolesList.value = await api.get('/consoles') }
  catch (e: any) { toast.error('Consoles: ' + (e.message || 'erreur')) }
  finally { loading.value = false }
}

function editJeu(j) { editingJeu.value = j.id; form.titre = j.titre; form.genre = j.genre; form.console_id = j.console_id; form.jaquette_url = j.jaquette_url || ''; showAdd.value = false }

async function saveJeu() {
  if (!isValidTitre(form.titre)) return toast.error('Titre invalide (2-100 caractères, lettres/chiffres/ -\'&)')
  if (!isValidGenre(form.genre)) return toast.error('Genre invalide (2-50 caractères)')
  form.titre = sanitizeInput(form.titre, 100)
  if (form.genre) form.genre = sanitizeInput(form.genre, 50)
  if (form.jaquette_url) form.jaquette_url = sanitizeInput(form.jaquette_url, 500)
  try {
    if (editingJeu.value) { await api.put(`/jeux/${editingJeu.value}`, { ...form }); toast.success('Jeu modifié') }
    else { await api.post('/jeux', { ...form }); toast.success('Jeu ajouté') }
    showAdd.value = false; editingJeu.value = null; form.titre = ''; form.genre = ''; form.console_id = null; form.jaquette_url = ''
    fetchData()
  } catch (e) { toast.error(e.message) }
}

async function deleteJeu(id) {
  if (!confirm('Supprimer ce jeu ?')) return
  try { await api.delete(`/jeux/${id}`); toast.success('Jeu supprimé'); fetchData() }
  catch (e) { toast.error(e.message) }
}

onMounted(fetchData)
</script>
