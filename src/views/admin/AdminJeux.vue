<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full max-w-full min-w-0">
      <div>
        <h3 class="font-gaming text-lg font-bold">Catalogue des jeux</h3>
        <p class="text-xs text-txt-dim">Gérez les titres disponibles et restaurez les jeux archivés</p>
      </div>
      <div class="flex items-center gap-3 flex-wrap">
        <label class="flex items-center gap-2 cursor-pointer text-xs text-txt-dim bg-bg-card px-3 py-1.5 rounded-lg border border-border/50 hover:border-neon-violet/30 transition-colors">
          <input type="checkbox" v-model="showArchived" @change="fetchJeux" class="rounded accent-neon-violet cursor-pointer" />
          <span>Afficher archivés</span>
        </label>
        <button @click="showAdd = true" class="btn-neon-violet flex items-center gap-2 text-sm">
          <Plus class="w-4 h-4" /> Ajouter un jeu
        </button>
      </div>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des jeux..." />
    </div>

    <div v-else-if="jeux.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Gamepad2 class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun jeu</p>
    </div>

    <div v-else class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="j in jeux" :key="j.id" 
        class="card-hover relative min-h-44 text-center group w-full max-w-full min-w-0 overflow-hidden flex flex-col justify-end transition-all"
        :class="j.deleted ? 'opacity-65 border-dashed border-neon-red/40 bg-bg-surface/40' : ''">
        <img v-if="(j.image_url || j.jaquette_url) && !imgErr[j.id]" :src="j.image_url || j.jaquette_url" :alt="j.titre" class="absolute inset-0 w-full h-full object-cover" @error="imgErr[j.id] = true" />
        <div class="absolute inset-0 bg-gradient-to-t from-bg via-bg/60 to-transparent"></div>
        <div class="relative z-10 p-2">
          <div v-if="(!j.image_url && !j.jaquette_url) || imgErr[j.id]" class="w-12 h-12 mx-auto rounded-xl bg-bg-surface/90 flex items-center justify-center mb-3">
            <Gamepad2 class="w-7 h-7" :class="j.deleted ? 'text-neon-red/70' : 'text-txt-dim'" />
          </div>
          <p class="font-medium text-sm truncate w-full max-w-full" :class="{ 'line-through text-txt-dim': j.deleted }">{{ j.titre }}</p>
          <p class="text-xs text-txt-dim truncate w-full max-w-full">{{ j.genre }}</p>
          <div class="mt-1 flex justify-center">
            <span v-if="j.deleted" class="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-neon-red/10 text-neon-red border border-neon-red/20">Archivé</span>
            <span v-else class="text-xs text-txt-dim truncate w-full max-w-full">{{ consoleName(j.console_id) }}</span>
          </div>
        </div>
        <div class="relative z-10 flex gap-2 mt-2 pb-2 justify-center opacity-100 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity flex-wrap shrink-0">
          <template v-if="j.deleted">
            <button @click="restoreJeu(j.id)" class="p-1.5 rounded-lg bg-bg-card/80 hover:bg-neon-green/20 text-neon-green border border-neon-green/30 transition-colors" title="Restaurer"><RotateCcw class="w-3.5 h-3.5" /></button>
            <button @click="permanentDeleteJeu(j.id)" class="p-1.5 rounded-lg bg-bg-card/80 hover:bg-neon-red/20 text-neon-red border border-neon-red/30 transition-colors" title="Supprimer définitivement"><Trash2 class="w-3.5 h-3.5" /></button>
          </template>
          <template v-else>
            <button @click="editJeu(j)" class="p-1.5 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier"><Pencil class="w-3.5 h-3.5" /></button>
            <button @click="deleteJeu(j.id)" class="p-1.5 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Archiver"><Trash2 class="w-3.5 h-3.5" /></button>
          </template>
        </div>
      </div>
    </div>

    <Modal :open="showAdd || !!editingJeu" @close="closeModal">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingJeu ? 'Modifier' : 'Ajouter' }} un jeu</h3>
        <div class="space-y-4">
          <input v-model="form.titre" placeholder="Titre du jeu" class="input-field" />
          <input v-model="form.genre" placeholder="Genre (ex: Action, Sport, Course...)" class="input-field" />
          <select v-model="form.console_id" class="input-field">
            <option :value="null">Toutes les consoles</option>
            <option v-for="c in consoles" :key="c.id" :value="c.id">{{ c.nom }}</option>
          </select>
          <input v-model="form.jaquette_url" placeholder="URL de la jaquette (ex: https://...)" class="input-field" />
          <div class="flex gap-3">
            <button @click="closeModal" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveJeu" :disabled="!form.titre" class="btn-neon-violet flex-1">{{ editingJeu ? 'Modifier' : 'Ajouter' }}</button>
          </div>
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
import { Plus, Gamepad2, Pencil, Trash2, RotateCcw } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { isValidTitre, isValidGenre, sanitizeInput } from '@/utils/validators'

const jeux = ref<any[]>([])
const consoles = ref<any[]>([])
const loading = ref(true)
const showAdd = ref(false)
const showArchived = ref(false)
const editingJeu = ref<any>(null)
const form = reactive({ titre: '', genre: '', console_id: null, jaquette_url: '' })
const imgErr = reactive<Record<number, boolean>>({})

function consoleName(id: number | null) {
  if (!id) return 'Toutes consoles'
  return consoles.value.find(c => c.id === id)?.nom || '—'
}

function closeModal() {
  showAdd.value = false
  editingJeu.value = null
  form.titre = ''
  form.genre = ''
  form.console_id = null
  form.jaquette_url = ''
}

async function fetchJeux() {
  loading.value = true
  try {
    const q = showArchived.value ? '?include_deleted=1' : ''
    const [jData, cData] = await Promise.all([
      api.get(`/jeux${q}`),
      api.get('/consoles')
    ])
    jeux.value = jData
    consoles.value = cData
  } catch (e: any) {
    toast.error('Erreur chargement jeux: ' + (e.message || ''))
  } finally {
    loading.value = false
  }
}

function editJeu(j: any) {
  editingJeu.value = j
  form.titre = j.titre
  form.genre = j.genre || ''
  form.console_id = j.console_id
  form.jaquette_url = j.jaquette_url || j.image_url || ''
}

async function saveJeu() {
  if (!isValidTitre(form.titre)) return toast.error('Titre invalide (2-100 caractères)')
  if (form.genre && !isValidGenre(form.genre)) return toast.error('Genre invalide (2-50 caractères)')
  form.titre = sanitizeInput(form.titre, 100)
  if (form.genre) form.genre = sanitizeInput(form.genre, 50)
  if (form.jaquette_url) form.jaquette_url = sanitizeInput(form.jaquette_url, 500)

  try {
    if (editingJeu.value) {
      await api.put(`/jeux/${editingJeu.value.id}`, form)
      toast.success('Jeu modifié')
    } else {
      await api.post('/jeux', form)
      toast.success('Jeu ajouté')
    }
    closeModal()
    await fetchJeux()
  } catch (e: any) {
    toast.error(e.message)
  }
}

async function deleteJeu(id: number) {
  if (!confirm('Archiver ce jeu ?')) return
  try {
    await api.delete(`/jeux/${id}`)
    toast.success('Jeu archivé')
    await fetchJeux()
  } catch (e: any) {
    toast.error(e.message)
  }
}

async function restoreJeu(id: number) {
  try {
    await api.post(`/jeux/${id}/restore`, {})
    toast.success('Jeu restauré avec succès !')
    await fetchJeux()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de restaurer'))
  }
}

async function permanentDeleteJeu(id: number) {
  if (!confirm('ATTENTION : Supprimer DÉFINITIVEMENT ce jeu ? Cette action est irréversible.')) return
  try {
    await api.delete(`/jeux/${id}/permanent`)
    toast.success('Jeu supprimé définitivement')
    await fetchJeux()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de supprimer'))
  }
}

onMounted(fetchJeux)
</script>
