<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full max-w-full min-w-0">
      <div>
        <h3 class="font-gaming text-lg font-bold">Gestion des consoles</h3>
        <p class="text-xs text-txt-dim">Gérez le parc de machines et restaurez les éléments archivés</p>
      </div>
      <div class="flex items-center gap-3 flex-wrap">
        <label class="flex items-center gap-2 cursor-pointer text-xs text-txt-dim bg-bg-card px-3 py-1.5 rounded-lg border border-border/50 hover:border-neon-violet/30 transition-colors">
          <input type="checkbox" v-model="showArchived" @change="fetchConsoles" class="rounded accent-neon-violet cursor-pointer" />
          <span>Afficher archivés</span>
        </label>
        <button @click="openAdd" class="btn-neon-violet flex items-center gap-2 text-sm">
          <Plus class="w-4 h-4" /> Ajouter
        </button>
      </div>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des consoles..." />
    </div>

    <div v-else-if="consoles.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <Monitor class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucune console</p>
    </div>

    <div v-else class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="c in consoles" :key="c.id" 
        class="card-hover relative min-h-44 text-center group w-full max-w-full min-w-0 overflow-hidden flex flex-col justify-end transition-all"
        :class="c.deleted ? 'opacity-65 border-dashed border-neon-red/40 bg-bg-surface/40' : ''">
        <!-- referrerpolicy="no-referrer" : beaucoup d'hébergeurs d'images
             refusent l'affichage quand la page appelante est inconnue (WebView
             Android) — sans lui, l'image restait vide alors que le lien est bon. -->
        <img v-if="imageSrc(c) && !imgErr[c.id]" :src="imageSrc(c)" :alt="c.nom"
          referrerpolicy="no-referrer" loading="lazy" decoding="async"
          class="absolute inset-0 w-full h-full object-cover" @error="imgErr[c.id] = true" />
        <div class="absolute inset-0 bg-gradient-to-t from-bg via-bg/60 to-transparent"></div>
        <div class="relative z-10 p-2">
          <div v-if="!imageSrc(c) || imgErr[c.id]" class="w-12 h-12 mx-auto rounded-xl bg-bg-surface/90 flex items-center justify-center mb-3">
            <Monitor class="w-7 h-7" :class="c.deleted ? 'text-neon-red/70' : statusColor(c.etat)" />
          </div>
          <p class="font-medium text-sm truncate w-full max-w-full" :class="{ 'line-through text-txt-dim': c.deleted }">{{ c.nom }}</p>
          <p class="text-xs text-txt-dim truncate w-full max-w-full">{{ c.type }} — Poste {{ c.poste_numero }}</p>
          <div class="mt-1 flex justify-center">
            <span v-if="c.deleted" class="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-neon-red/10 text-neon-red border border-neon-red/20">Archivé</span>
            <span v-else class="badge inline-block max-w-full truncate" :class="statusBadge(c.etat)">{{ statusLabel(c.etat) }}</span>
          </div>
        </div>
        <div class="relative z-10 flex gap-2 mt-2 pb-2 justify-center opacity-100 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity flex-wrap shrink-0">
          <template v-if="c.deleted">
            <button @click="restoreConsole(c.id)" class="p-1.5 rounded-lg bg-bg-card/80 hover:bg-neon-green/20 text-neon-green border border-neon-green/30 transition-colors" title="Restaurer"><RotateCcw class="w-3.5 h-3.5" /></button>
            <button @click="permanentDeleteConsole(c.id)" class="p-1.5 rounded-lg bg-bg-card/80 hover:bg-neon-red/20 text-neon-red border border-neon-red/30 transition-colors" title="Supprimer définitivement"><Trash2 class="w-3.5 h-3.5" /></button>
          </template>
          <template v-else>
            <button @click="editConsole(c)" class="p-1.5 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier"><Pencil class="w-3.5 h-3.5" /></button>
            <button @click="deleteConsole(c.id)" class="p-1.5 rounded-lg hover:bg-neon-red/10 text-neon-red transition-colors" title="Archiver"><Trash2 class="w-3.5 h-3.5" /></button>
          </template>
        </div>
      </div>
    </div>

    <Modal :open="showForm" @close="showForm = false">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Ajouter' }} une console</h3>
        <div class="space-y-4">
          <input v-model="form.nom" placeholder="Nom (ex: PS5 - Poste 1)" class="input-field" />
          <select v-model="form.type" class="input-field">
            <option value="PS5">PlayStation 5</option>
            <option value="PS4">PlayStation 4</option>
            <option value="PS3">PlayStation 3</option>
            <option value="Xbox Series">Xbox Series</option>
            <option value="Nintendo Switch">Nintendo Switch</option>
          </select>
          <input v-model.number="form.poste_numero" type="number" placeholder="Numéro de poste" class="input-field" min="1" max="100" />
          <select v-model="form.etat" class="input-field">
            <option value="disponible">Disponible (Libre)</option>
            <option value="occupee">Occupé</option>
            <option value="pause">En pause</option>
            <option value="maintenance">Maintenance</option>
            <option value="hors_service">Hors service</option>
          </select>
          <input v-model="form.image_url" placeholder="URL image (ex: https://site.com/ps5.jpg)" class="input-field" @input="formImgErr = false" />
          <!-- APERÇU IMMÉDIAT : on voit tout de suite si le lien fonctionne au
               lieu de découvrir une carte vide après enregistrement. -->
          <div v-if="apercuUrl" class="rounded-xl overflow-hidden border border-border/50 bg-bg-surface">
            <img v-if="!formImgErr" :src="apercuUrl" alt="Aperçu" referrerpolicy="no-referrer"
              class="w-full h-32 object-cover" @error="formImgErr = true" />
            <p v-else class="text-xs text-neon-red p-3">
              Image inaccessible. Utilisez un lien direct vers un fichier image (.jpg, .png, .webp),
              accessible sans connexion, et vérifiez que le téléphone a Internet.
              Un fichier du téléphone ne peut pas être utilisé ici.
            </p>
          </div>
          <div class="flex gap-3">
            <button @click="showForm = false" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveConsole" :disabled="!form.nom" class="btn-neon-violet flex-1">{{ editingId ? 'Modifier' : 'Ajouter' }}</button>
          </div>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'
import Modal from '@/components/ui/Modal.vue'
import { Plus, Monitor, Pencil, Trash2, RotateCcw } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { isValidNom, isValidConsoleType, isValidPosteNumero, sanitizeInput } from '@/utils/validators'
import { normalizeImageUrl } from '@/utils/helpers'

const consoles = ref<any[]>([])
const loading = ref(true)
const showForm = ref(false)
const showArchived = ref(false)
const editingId = ref<number | null>(null)
const form = reactive({ nom: '', type: 'PS5', poste_numero: 1, etat: 'disponible', image_url: '' })
const imgErr = reactive<Record<number, boolean>>({})
const formImgErr = ref(false)

// Lien d'image réellement utilisable pour l'affichage (corrige les liens
// enregistrés sans « https:// », qui étaient lus comme un chemin interne).
function imageSrc(c: any) {
  return normalizeImageUrl(c?.image_url)
}
const apercuUrl = computed(() => normalizeImageUrl(form.image_url))

function statusColor(etat: string) {
  return etat === 'disponible' ? 'text-neon-green' : etat === 'occupee' ? 'text-neon-red' : 'text-neon-yellow'
}
function statusBadge(etat: string) {
  return etat === 'disponible' ? 'badge-green' : etat === 'occupee' ? 'badge-red' : 'badge-yellow'
}
function statusLabel(etat: string) {
  return etat === 'disponible' ? 'Disponible' : etat === 'occupee' ? 'En jeu' : 'Maintenance'
}

async function fetchConsoles() {
  loading.value = true
  try {
    const q = showArchived.value ? '?include_deleted=1' : ''
    consoles.value = await api.get(`/consoles${q}`)
  } catch (e: any) {
    toast.error('Erreur chargement consoles: ' + (e.message || ''))
  } finally {
    loading.value = false
  }
}

function openAdd() {
  editingId.value = null
  form.nom = ''
  form.type = 'PS5'
  form.poste_numero = consoles.value.length + 1
  form.image_url = ''
  form.etat = 'disponible'
  showForm.value = true
}

function editConsole(c: any) {
  editingId.value = c.id
  form.nom = c.nom
  form.type = c.type
  form.poste_numero = c.poste_numero
  form.image_url = c.image_url || ''
  form.etat = c.etat || 'disponible'
  showForm.value = true
}

async function saveConsole() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères)')
  if (!isValidConsoleType(form.type)) return toast.error('Type de console invalide')
  if (!isValidPosteNumero(form.poste_numero)) return toast.error('Numéro de poste invalide (1-100)')
  form.nom = sanitizeInput(form.nom, 50)
  // Le lien est normalisé AVANT l'envoi : « site.com/x.jpg » devient
  // « https://site.com/x.jpg », sinon la WebView cherche un fichier local.
  if (form.image_url) {
    const propre = normalizeImageUrl(sanitizeInput(form.image_url, 500))
    if (!propre) return toast.error("Lien d'image invalide : donnez une adresse web (https://…) et non un fichier du téléphone")
    form.image_url = propre
  }
  try {
    if (editingId.value) {
      await api.put(`/consoles/${editingId.value}`, form)
      toast.success('Console modifiée')
    } else {
      await api.post('/consoles', form)
      toast.success('Console créée')
    }
    showForm.value = false
    await fetchConsoles()
  } catch (e: any) {
    toast.error(e.message)
  }
}

async function deleteConsole(id: number) {
  if (!confirm('Archiver cette console ?')) return
  try {
    await api.delete(`/consoles/${id}`)
    toast.success('Console archivée')
    await fetchConsoles()
  } catch (e: any) {
    toast.error(e.message)
  }
}

async function restoreConsole(id: number) {
  try {
    await api.post(`/consoles/${id}/restore`, {})
    toast.success('Console restaurée avec succès !')
    await fetchConsoles()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de restaurer'))
  }
}

async function permanentDeleteConsole(id: number) {
  if (!confirm('ATTENTION : Supprimer DÉFINITIVEMENT cette console ? Cette action est irréversible.')) return
  try {
    await api.delete(`/consoles/${id}/permanent`)
    toast.success('Console supprimée définitivement')
    await fetchConsoles()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de supprimer'))
  }
}

onMounted(fetchConsoles)
</script>
