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
          <p class="font-medium truncate">
            {{ p.regle_type === 'temps' ? 'Temps de jeu' : 'Montant dépensé' }} —
            {{ p.regle_type === 'temps' ? `Seuil : ${p.seuil} min` : `Montant : ${formatCurrency(p.seuil)}` }}
          </p>
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
          <label class="text-sm text-txt-muted">Nom de l'application</label>
          <input v-model="appNameDraft" maxlength="40" class="input-field w-full max-w-full min-w-0" placeholder="Ex. Play Zone Lubumbashi" />
          <p class="text-xs text-txt-dim mt-1">Visible dans la connexion, les menus et le titre de l'application.</p>
        </div>
        <div class="w-full max-w-full min-w-0">
          <label class="text-sm text-txt-muted">TVA par défaut (%)</label>
          <input v-model.number="config.taux_tva" type="number" class="input-field w-full max-w-full min-w-0" />
        </div>
        <button @click="saveGeneral" class="btn-neon-violet w-full sm:w-auto">Enregistrer</button>
      </div>
    </div>

    <div class="card w-full max-w-full min-w-0 overflow-hidden">
      <div class="flex items-center gap-3 mb-4">
        <Cloud class="w-5 h-5 text-neon-blue shrink-0" />
        <h4 class="font-gaming font-bold truncate">Synchronisation cloud (Supabase)</h4>
      </div>
      <div v-if="!syncStatus.supabaseEnabled" class="text-center py-6">
        <CloudOff class="w-10 h-10 text-txt-dim mx-auto mb-2" />
        <p class="text-txt-dim text-sm">Cloud Supabase non configuré</p>
        <p class="text-xs text-txt-dim mt-1">Vérifiez la connexion internet</p>
      </div>
      <div v-else-if="!syncStatus.supabaseAvailable" class="text-center py-6">
        <CloudOff class="w-10 h-10 text-amber-400 mx-auto mb-2" />
        <p class="text-amber-400 text-sm">Supabase configuré (offline)</p>
        <p class="text-xs text-txt-dim mt-1">Données locales synchronisées à la reconnexion — {{ syncStatus.mode }}</p>
        <button @click="runSync" class="btn-neon-violet mt-3">Tester connexion</button>
      </div>
      <div v-else class="space-y-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="flex items-center justify-between p-3 bg-bg-surface rounded-xl">
          <div class="flex items-center gap-3">
            <div class="w-3 h-3 rounded-full" :class="syncStatus.enabled ? 'bg-neon-green animate-pulse' : 'bg-txt-dim'"></div>
            <span class="font-medium text-sm">{{ syncStatus.enabled ? 'Sync active' : 'Sync désactivée' }}</span>
          </div>
          <button @click="toggleSync" class="relative w-12 h-6 rounded-full transition-colors" :class="syncStatus.enabled ? 'bg-neon-green' : 'bg-bg-hover'">
            <div class="absolute top-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform" :class="syncStatus.enabled ? 'translate-x-6' : 'translate-x-0.5'"></div>
          </button>
        </div>
        <div v-if="syncStatus.lastSync" class="text-xs text-txt-dim px-1">
          Dernière sync : {{ formatDate(syncStatus.lastSync) }}
        </div>
        <button @click="runSync" :disabled="syncing || !syncStatus.enabled" class="btn-neon-violet w-full flex items-center justify-center gap-2">
          <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': syncing }" />
          {{ syncing ? 'Synchronisation en cours...' : 'Synchroniser maintenant' }}
        </button>
        <p class="text-xs text-txt-dim text-center">Compare et fusionne les données locales SQLite ↔ Supabase cloud</p>
      </div>
    </div>

    <div class="card w-full max-w-full min-w-0 overflow-hidden">
      <div class="flex items-center gap-3 mb-2">
        <Trash2 class="w-5 h-5 text-neon-red shrink-0" />
        <h4 class="font-gaming font-bold truncate">Historique de synchronisation</h4>
      </div>
      <p class="text-xs text-txt-muted mb-4">
        Supprime l'historique local des changements déjà synchronisés et ne conserve que les
        <strong>1000 entrées les plus récentes</strong>. Les changements pas encore envoyés à
        Supabase ne sont jamais touchés : aucune donnée n'est perdue.
      </p>
      <button
        @click="purgerHistorique"
        :disabled="purging"
        class="btn rounded-xl bg-neon-red/20 hover:bg-neon-red/30 text-neon-red border border-neon-red/30 w-full sm:w-auto flex items-center justify-center gap-2"
      >
        <Trash2 class="w-4 h-4" :class="{ 'animate-spin': purging }" />
        {{ purging ? 'Nettoyage...' : 'Nettoyer les synchronisations (garder 1000)' }}
      </button>
    </div>

    <div class="card w-full max-w-full min-w-0 overflow-hidden">
      <div class="flex items-center gap-3 mb-2">
        <Cloud class="w-5 h-5 text-neon-red shrink-0" />
        <h4 class="font-gaming font-bold truncate">Historique de synchronisation (Supabase)</h4>
      </div>
      <p class="text-xs text-txt-muted mb-4">
        Supprime sur Supabase les anciennes entrées du journal de synchronisation
        (<strong>sync_changes</strong>) et ne conserve que les
        <strong>1000 plus récentes</strong> afin de ne pas surcharger la base cloud.
        Nécessite Internet.
      </p>
      <button
        @click="purgerCloud"
        :disabled="purgingCloud"
        class="btn rounded-xl bg-neon-red/20 hover:bg-neon-red/30 text-neon-red border border-neon-red/30 w-full sm:w-auto flex items-center justify-center gap-2"
      >
        <Cloud class="w-4 h-4" :class="{ 'animate-spin': purgingCloud }" />
        {{ purgingCloud ? 'Purge en cours...' : 'Purger le journal Supabase (garder 1000)' }}
      </button>
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

        <div>
          <label class="text-sm text-txt-muted mb-3 block">Fond d'écran</label>
          <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <button @click="changeBgMode('default')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.bgMode === 'default' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <div class="flex items-center gap-2">
                <Monitor class="w-5 h-5 text-txt-dim" />
                <p class="font-bold text-sm">Classique</p>
              </div>
              <p class="text-xs text-txt-dim mt-1">Fond épuré d'origine</p>
            </button>
            <button @click="changeBgMode('stars')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.bgMode === 'stars' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <div class="flex items-center gap-2">
                <Sparkles class="w-5 h-5 text-neon-yellow" />
                <p class="font-bold text-sm">Étoiles</p>
              </div>
              <p class="text-xs text-txt-dim mt-1">Particules animées réactives</p>
            </button>
            <button @click="changeBgMode('custom')"
              class="p-4 rounded-xl border-2 transition-all text-left"
              :class="settings.bgMode === 'custom' ? 'border-neon-violet bg-neon-violet/10' : 'border-white/10 hover:border-white/20'">
              <div class="flex items-center gap-2">
                <Image class="w-5 h-5 text-neon-green" />
                <p class="font-bold text-sm">Photo</p>
              </div>
              <p class="text-xs text-txt-dim mt-1">{{ settings.bgCustomImage ? "Changer l'image" : "Importer une image" }}</p>
            </button>
          </div>
          <div v-if="settings.bgCustomImage" class="mt-3 p-3 bg-bg-surface rounded-xl flex items-center justify-between border border-white/5">
            <span class="text-xs text-txt-muted">Photo personnalisée active (adaptée au thème)</span>
            <button @click="deleteCustomBg" class="text-xs text-neon-red hover:underline">Retirer l'image</button>
          </div>
          <input ref="fileInputRef" type="file" accept="image/*" class="hidden" @change="onPhotoSelected" />
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
            <label class="text-sm text-txt-muted">
              {{ form.regle_type === 'montant' ? 'Montant (FC)' : 'Seuil (minutes)' }}
            </label>
            <input
              v-model.number="form.seuil"
              type="number"
              :placeholder="form.regle_type === 'montant' ? 'Ex: 5000' : 'Ex: 60'"
              class="input-field"
            />
          </div>
          <div>
            <label class="text-sm text-txt-muted">Jetons attribués</label>
            <input v-model.number="form.jetons_attribues" type="number" placeholder="Ex: 1" class="input-field" />
          </div>
          <div>
            <label class="text-sm text-txt-muted">Valeur d'un jeton (FC)</label>
            <input v-model.number="form.valeur_jeton" type="number" min="1" placeholder="Ex: 100" class="input-field" />
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
          <div class="flex justify-between">
            <span class="text-txt-dim">{{ selected.regle_type === 'montant' ? 'Montant' : 'Seuil' }}</span>
            <span>{{ selected.regle_type === 'montant' ? formatCurrency(selected.seuil) : `${selected.seuil} min` }}</span>
          </div>
          <div class="flex justify-between"><span class="text-txt-dim">Jetons</span><span class="font-gaming font-bold text-neon-yellow">{{ selected.jetons_attribues }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Valeur d'un jeton</span><span class="font-gaming font-bold text-neon-yellow">{{ formatCurrency(selected.valeur_jeton ?? 100) }}</span></div>
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
import { toast } from 'vue-sonner'
import Loader from '@/components/ui/Loader.vue'
import Modal from '@/components/ui/Modal.vue'
import { Plus, Pencil, Trash2, Settings, Moon, Sun, Cloud, CloudOff, RefreshCw, Sparkles, Image as ImageIcon, Monitor } from 'lucide-vue-next'
import { isValidRegleType, isValidSeuil, isValidJetonsAttribues } from '@/utils/validators'
import { useSettingsStore } from '@/stores/settings'
import { formatDate, formatCurrency } from '@/utils/helpers'

const settings = useSettingsStore()
const loading = ref(true)
const parametres = ref([])
const showForm = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const editingId = ref(null)
const form = reactive({ regle_type: 'temps', seuil: 60, jetons_attribues: 1, valeur_jeton: 100, actif: true })
const config = reactive({ taux_tva: settings.tauxTva || 20 })
const appNameDraft = ref(settings.appName)
const syncStatus = ref({ enabled: false, supabaseEnabled: false, lastSync: null, syncing: false })
const syncing = ref(false)
const purging = ref(false)
const purgingCloud = ref(false)

function changeFont(mode: string) {
  settings.setFont(mode)
  toast.success(`Police: ${mode === 'gaming' ? 'Montserrat' : 'Inter'}`)
}


const fileInputRef = ref<HTMLInputElement | null>(null)

function changeBgMode(mode: 'default' | 'stars' | 'custom') {
  if (mode === 'custom' && !settings.bgCustomImage) {
    triggerPhotoUpload()
    return
  }
  settings.setBgMode(mode)
  toast.success(`Fond: ${mode === 'stars' ? 'Étoiles animées' : mode === 'custom' ? 'Photo personnalisée' : 'Classique'}`)
}

function triggerPhotoUpload() {
  fileInputRef.value?.click()
}

function deleteCustomBg() {
  settings.removeCustomBg()
  toast.info('Photo personnalisée retirée')
}

function onPhotoSelected(e: Event) {
  const target = e.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  if (file.size > 10 * 1024 * 1024) {
    toast.error('Image trop volumineuse (max 10 Mo)')
    return
  }

  const reader = new FileReader()
  reader.onload = (event) => {
    const rawData = event.target?.result as string
    const img = new window.Image()
    img.onload = () => {
      // Redimensionnement optimisé pour fluidité mobile et stockage
      const canvas = document.createElement('canvas')
      let width = img.width
      let height = img.height
      const maxDim = 1600
      if (width > maxDim || height > maxDim) {
        if (width > height) {
          height = Math.round((height * maxDim) / width)
          width = maxDim
        } else {
          width = Math.round((width * maxDim) / height)
          height = maxDim
        }
      }
      canvas.width = width
      canvas.height = height
      const ctx = canvas.getContext('2d')
      if (ctx) {
        ctx.drawImage(img, 0, 0, width, height)
        const optimized = canvas.toDataURL('image/jpeg', 0.82)
        settings.setCustomBg(optimized)
        toast.success('Photo personnalisée appliquée avec adaptation thématique')
      }
    }
    img.src = rawData
  }
  reader.readAsDataURL(file)
  target.value = ''
}

function changeTheme(mode: string) {
  settings.setTheme(mode)
  toast.success(`Thème: ${mode === 'dark' ? 'Sombre' : 'Clair'}`)
}

async function saveGeneral() {
  if (!appNameDraft.value.trim()) return toast.error("Le nom de l'application est requis")
  await settings.saveAppName(appNameDraft.value)
  appNameDraft.value = settings.appName
  if (config.taux_tva != null && !isNaN(Number(config.taux_tva))) {
    await settings.saveTva(Number(config.taux_tva))
  }
  toast.success("Paramètres généraux (nom & TVA) enregistrés et synchronisés !")
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
  Object.assign(form, { regle_type: 'temps', seuil: 60, jetons_attribues: 1, valeur_jeton: 100, actif: true })
  showForm.value = true
}

function editParametre(p) {
  editingId.value = p.id
  Object.assign(form, { regle_type: p.regle_type, seuil: p.seuil, jetons_attribues: p.jetons_attribues, valeur_jeton: p.valeur_jeton ?? 100, actif: !!p.actif })
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
  if (!Number.isInteger(Number(form.valeur_jeton)) || Number(form.valeur_jeton) < 1 || Number(form.valeur_jeton) > 1000000) return toast.error('Valeur d\'un jeton invalide (1-1000000 FC)')
  if (form.seuil === null || form.jetons_attribues === null || form.valeur_jeton === null) return toast.error('Champs requis')
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

async function fetchSyncStatus() {
  try { syncStatus.value = await api.get('/sync/status') } catch {}
}

async function toggleSync() {
  try {
    const res = await api.post('/sync/toggle', { enabled: !syncStatus.value.enabled })
    syncStatus.value = { ...syncStatus.value, enabled: res.enabled }
    toast.success(res.enabled ? 'Sync activée' : 'Sync désactivée')
  } catch (e: any) { toast.error(e.message) }
}

/// Nettoyage de l'historique de synchronisation (bouton Paramètres) : ne garde
/// que les 1000 entrées les plus récentes déjà traitées. Les entrées PENDING
/// (pas encore envoyées) sont conservées par le moteur Rust.
async function purgerHistorique() {
  if (!confirm('Supprimer l\'historique de synchronisation et ne garder que les 1000 entrées les plus récentes ?')) return
  purging.value = true
  try {
    const res = await api.post('/sync/outbox/purge', { mode: 'keep1000' })
    toast.success(`Historique nettoyé : ${res.deleted || 0} entrée(s) supprimée(s), ${res.remaining ?? 0} conservée(s).`)
  } catch (e: any) {
    toast.error('Erreur lors du nettoyage : ' + (e.message || e))
  } finally {
    purging.value = false
  }
}

/// Purge du journal cloud (sync_changes) côté Supabase : ne garde que les
/// 1000 entrées les plus récentes. Admin uniquement.
async function purgerCloud() {
  if (!confirm('Supprimer sur Supabase les anciennes entrées du journal de synchronisation (garder les 1000 plus récentes) ?')) return
  purgingCloud.value = true
  try {
    const res = await api.post('/sync/cloud/purge', { keep: 1000 })
    toast.success(`Journal Supabase purgé : ${res.deleted || 0} entrée(s) supprimée(s), ${res.remaining ?? 0} conservée(s).`)
  } catch (e: any) {
    toast.error('Erreur lors de la purge Supabase : ' + (e.message || e))
  } finally {
    purgingCloud.value = false
  }
}

async function runSync() {
  syncing.value = true
  try {
    const res = await api.post('/sync/run')
    if (res.started === false) { toast.info('Sync déjà en cours'); return }
    // La sync tourne en arrière-plan (évite le Timeout IPC Android WebView) :
    // on interroge /sync/poll jusqu'à ce qu'elle soit terminée.
    let last: any = null
    for (let i = 0; i < 80; i++) {
      await new Promise(r => setTimeout(r, 1500))
      try { last = (await api.get('/sync/poll')).last_sync } catch {}
      if (last && last.running !== true) break
    }
    if (last && last.success) {
      const pushed = Object.values(last.pushed || {}).reduce((a: number, b: number) => a + b, 0)
      const pulled = Object.values(last.pulled || {}).reduce((a: number, b: number) => a + b, 0)
      const secs = Math.round((last.duration_ms || 0) / 1000)
      toast.success(`Sync terminée en ${secs}s — ${pushed} envoyé(s), ${pulled} reçu(s)`)
    } else if (last) {
      toast.error(last.message || 'Sync échouée')
    } else {
      toast.error('Sync trop longue, vérifiez la connexion')
    }
    fetchSyncStatus()
  } catch (e: any) { toast.error(e.message) }
  finally { syncing.value = false }
}

onMounted(() => { fetchData(); fetchSyncStatus(); config.taux_tva = settings.tauxTva; window.addEventListener('sync-completed', () => { config.taux_tva = settings.tauxTva }); window.addEventListener('app-data-refresh', () => { config.taux_tva = settings.tauxTva; fetchData() }) })
</script>
