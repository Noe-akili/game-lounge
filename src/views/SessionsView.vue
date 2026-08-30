<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h3 class="font-gaming text-lg font-bold">Sessions en cours</h3>
      <div class="flex items-center gap-2 sm:gap-3 flex-wrap">
        <span class="badge-violet shrink-0">{{ activeSessions.length }} active(s)</span>
        <button @click="openCreate" class="btn-neon-violet flex items-center gap-2 text-sm shrink-0">
          <Plus class="w-4 h-4" /> Nouvelle
        </button>
        <button @click="fetchData" class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim shrink-0">
          <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': loading }" />
        </button>
      </div>
    </div>

    <div v-if="loading && activeSessions.length === 0" class="card">
      <Loader variant="neon" size="lg" text="Chargement des sessions..." />
    </div>

    <div v-else-if="activeSessions.length === 0" class="card text-center py-12">
      <PlayCircle class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucune session en cours</p>
    </div>

    <div v-else class="space-y-3 w-full max-w-full min-w-0">
      <div v-for="s in activeSessions" :key="s.id" class="card flex flex-col sm:flex-row sm:items-center gap-3 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewDetail(s)">
        <div class="flex items-center gap-3 sm:gap-4 w-full sm:w-auto min-w-0 flex-1">
          <div class="w-12 h-12 rounded-xl flex items-center justify-center shrink-0"
            :class="s.statut === 'en_cours' ? 'bg-neon-green/20' : 'bg-neon-yellow/20'">
            <component :is="s.statut === 'en_cours' ? PlayCircle : PauseCircle" class="w-6 h-6"
              :class="s.statut === 'en_cours' ? 'text-neon-green' : 'text-neon-yellow'" />
          </div>
          <div class="flex-1 min-w-0">
            <p class="font-medium truncate">{{ s.console_nom || 'Console' }} — {{ s.jeu_nom || 'Jeu' }}</p>
            <p class="text-sm text-txt-dim truncate">{{ s.joueur_nom || 'Joueur' }} · {{ formatDuration(s.duree_minutes * 60) }}</p>
          </div>
          <div class="text-right sm:text-right w-full sm:w-auto flex sm:block justify-between items-center gap-2 min-w-0">
            <p class="font-gaming font-bold text-neon-green truncate">{{ formatCurrency(s.montant || 0) }}</p>
            <span class="badge shrink-0" :class="s.statut === 'en_cours' ? 'badge-green' : 'badge-yellow'">
              {{ s.statut === 'en_cours' ? 'En cours' : 'En pause' }}
            </span>
          </div>
        </div>
        <div class="flex gap-2 flex-wrap sm:flex-nowrap w-full sm:w-auto justify-end shrink-0" @click.stop>
          <button v-if="s.statut === 'en_cours'" @click="pauseSession(s)" class="p-2 rounded-lg bg-neon-yellow/10 text-neon-yellow hover:bg-neon-yellow/20 transition-colors" title="Pause">
            <PauseCircle class="w-4 h-4" />
          </button>
          <button v-else @click="resumeSession(s)" class="p-2 rounded-lg bg-neon-blue/10 text-neon-blue hover:bg-neon-blue/20 transition-colors" title="Reprendre">
            <PlayCircle class="w-4 h-4" />
          </button>
          <button @click="endSession(s)" class="p-2 rounded-lg bg-neon-red/10 text-neon-red hover:bg-neon-red/20 transition-colors" title="Terminer">
            <Square class="w-4 h-4" />
          </button>
          <button @click="editSession(s)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim" title="Modifier">
            <Pencil class="w-4 h-4" />
          </button>
          <button @click="deleteSession(s.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red" title="Supprimer">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <div>
      <div class="flex items-center justify-between mb-3">
        <h3 class="font-gaming text-lg font-bold">Historique récent</h3>
        <button @click="showAll = !showAll" class="text-sm text-neon-violet hover:underline">{{ showAll ? 'Voir moins' : 'Voir tout' }}</button>
      </div>
      <div v-if="recentSessions.length === 0" class="card text-center py-8">
        <p class="text-txt-dim text-sm">Aucune session terminée</p>
      </div>
      <div v-else class="space-y-2 w-full max-w-full min-w-0">
        <div v-for="s in (showAll ? allSessionsFiltered : recentSessions)" :key="s.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-3 text-sm w-full max-w-full min-w-0 overflow-hidden hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewDetail(s)">
          <div class="flex items-center gap-2 sm:gap-3 flex-1 min-w-0 w-full">
            <CheckCircle2 class="w-5 h-5 text-neon-green shrink-0" />
            <div class="flex-1 min-w-0">
              <p class="truncate">{{ s.console_nom }} — {{ s.joueur_nom }} · {{ s.jeu_nom }}</p>
              <p class="text-xs text-txt-dim truncate">{{ formatDate(s.created_at) }} · {{ s.statut }}</p>
            </div>
          </div>
          <div class="flex items-center justify-between sm:justify-end gap-2 w-full sm:w-auto shrink-0">
            <span class="font-gaming font-bold text-neon-green shrink-0">{{ formatCurrency(s.montant) }}</span>
            <div class="flex gap-1 shrink-0" @click.stop>
              <button @click="editSession(s)" class="p-1.5 rounded-lg hover:bg-bg-hover text-txt-dim"><Pencil class="w-3.5 h-3.5" /></button>
              <button @click="deleteSession(s.id)" class="p-1.5 rounded-lg hover:bg-neon-red/10 text-neon-red"><Trash2 class="w-3.5 h-3.5" /></button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <Modal :open="showDetail" @close="showDetail = false" size="lg">
      <div class="p-6" v-if="selected">
        <h3 class="font-gaming text-xl font-bold mb-4">Session #{{ selected.id }}</h3>
        <div class="space-y-3">
          <div class="flex justify-between"><span class="text-txt-dim">Console</span><span>{{ selected.console_nom }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Joueur</span><span>{{ selected.joueur_nom }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Jeu</span><span>{{ selected.jeu_nom }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Statut</span><span class="badge" :class="selected.statut === 'en_cours' ? 'badge-green' : selected.statut === 'pause' ? 'badge-yellow' : 'badge-violet'">{{ selected.statut }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Durée</span><span>{{ selected.duree_minutes }} min ({{ formatDuration((selected.duree_minutes||0)*60) }})</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Montant</span><span class="font-gaming text-neon-green">{{ formatCurrency(selected.montant) }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Début</span><span class="text-sm">{{ formatDate(selected.debut) }}</span></div>
          <div v-if="selected.fin" class="flex justify-between"><span class="text-txt-dim">Fin</span><span class="text-sm">{{ formatDate(selected.fin) }}</span></div>
        </div>
        <div class="flex gap-3 mt-6">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <button @click="editSession(selected); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2"><Pencil class="w-4 h-4" /> Modifier</button>
        </div>
      </div>
    </Modal>

    <Modal :open="showForm" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Nouvelle' }} session</h3>
        <div class="space-y-4">
          <div v-if="!editingId">
            <label class="text-sm text-txt-muted">Console</label>
            <select v-model.number="form.console_id" class="input-field">
              <option :value="null" disabled>Choisir une console</option>
              <option v-for="c in consoles" :key="c.id" :value="c.id">{{ c.nom }} — {{ c.type }} ({{ c.etat }})</option>
            </select>
          </div>
          <div>
            <label class="text-sm text-txt-muted">Joueur</label>
            <select v-model.number="form.joueur_id" class="input-field">
              <option :value="null" disabled>Choisir un joueur</option>
              <option v-for="j in joueurs" :key="j.id" :value="j.id">{{ j.nom }}</option>
            </select>
          </div>
          <div>
            <label class="text-sm text-txt-muted">Jeu</label>
            <select v-model.number="form.jeu_id" class="input-field">
              <option :value="null" disabled>Choisir un jeu</option>
              <option v-for="jeu in jeux" :key="jeu.id" :value="jeu.id">{{ jeu.titre }}</option>
            </select>
          </div>
          <div v-if="editingId" class="grid grid-cols-2 gap-3">
            <div>
              <label class="text-sm text-txt-muted">Durée (min)</label>
              <input v-model.number="form.duree_minutes" type="number" class="input-field" />
            </div>
            <div>
              <label class="text-sm text-txt-muted">Montant (FC)</label>
              <input v-model.number="form.montant" type="number" class="input-field" />
            </div>
            <div class="col-span-2">
              <label class="text-sm text-txt-muted">Statut</label>
              <select v-model="form.statut" class="input-field">
                <option value="en_cours">En cours</option>
                <option value="pause">Pause</option>
                <option value="terminee">Terminée</option>
                <option value="annulee">Annulée</option>
              </select>
            </div>
          </div>
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveSession" :disabled="!form.joueur_id || !form.jeu_id || (!editingId && !form.console_id)" class="btn-neon-violet flex-1">{{ editingId ? 'Modifier' : 'Créer' }}</button>
          </div>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed } from 'vue'
import { api } from '@/utils/api'
import { formatDuration, formatCurrency, formatDate } from '@/utils/helpers'
import { toast } from 'sonner'
import { PlayCircle, PauseCircle, Square, CheckCircle2, RefreshCw, Plus, Pencil, Trash2 } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import Modal from '@/components/ui/Modal.vue'
import { isValidId, isValidDuree, isValidPrix, isValidSessionStatut } from '@/utils/validators'

const loading = ref(false)
const activeSessions = ref([])
const recentSessions = ref([])
const allSessions = ref([])
const showAll = ref(false)
const consoles = ref([])
const joueurs = ref([])
const jeux = ref([])

const showForm = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const editingId = ref(null)
const form = reactive({ console_id: null, joueur_id: null, jeu_id: null, duree_minutes: 0, montant: 0, statut: 'en_cours' })

const allSessionsFiltered = computed(() => allSessions.value.filter(s => s.statut === 'terminee' || s.statut === 'annulee').slice(0, 50))

async function fetchData() {
  loading.value = true
  try {
    const all = await api.get('/sessions')
    allSessions.value = all
    activeSessions.value = all.filter(s => s.statut === 'en_cours' || s.statut === 'pause')
    recentSessions.value = all.filter(s => s.statut === 'terminee').slice(0, 10)
  } catch (e) {
    console.error(e)
  } finally {
    loading.value = false
  }
}

async function fetchRefs() {
  try { consoles.value = await api.get('/consoles') } catch {}
  try { joueurs.value = await api.get('/joueurs') } catch {}
  try { jeux.value = await api.get('/jeux') } catch {}
}

function openCreate() {
  editingId.value = null
  Object.assign(form, { console_id: null, joueur_id: null, jeu_id: null, duree_minutes: 0, montant: 0, statut: 'en_cours' })
  showForm.value = true
  fetchRefs()
}

function editSession(s) {
  editingId.value = s.id
  Object.assign(form, { console_id: s.console_id, joueur_id: s.joueur_id, jeu_id: s.jeu_id, duree_minutes: s.duree_minutes || 0, montant: s.montant || 0, statut: s.statut })
  showForm.value = true
  fetchRefs()
}

function closeForm() { showForm.value = false; editingId.value = null }

async function saveSession() {
  if (!editingId.value) {
    if (!isValidId(form.console_id)) return toast.error('Console invalide')
    if (!isValidId(form.joueur_id)) return toast.error('Joueur invalide')
    if (!isValidId(form.jeu_id)) return toast.error('Jeu invalide')
  } else {
    if (form.console_id && !isValidId(form.console_id)) return toast.error('Console invalide')
    if (form.joueur_id && !isValidId(form.joueur_id)) return toast.error('Joueur invalide')
    if (form.jeu_id && !isValidId(form.jeu_id)) return toast.error('Jeu invalide')
    if (form.duree_minutes !== null && form.duree_minutes !== undefined && !isValidDuree(form.duree_minutes)) return toast.error('Durée invalide (1-1000)')
    if (form.montant !== null && form.montant !== undefined && !isValidPrix(form.montant)) return toast.error('Montant invalide (1-1000000)')
    if (form.statut && !isValidSessionStatut(form.statut)) return toast.error('Statut invalide')
  }
  try {
    if (editingId.value) {
      await api.put(`/sessions/${editingId.value}`, { console_id: form.console_id, joueur_id: form.joueur_id, jeu_id: form.jeu_id, duree_minutes: form.duree_minutes, montant: form.montant, statut: form.statut })
      toast.success('Session modifiée')
    } else {
      await api.post('/sessions', { console_id: form.console_id, joueur_id: form.joueur_id, jeu_id: form.jeu_id })
      toast.success('Session créée')
    }
    closeForm(); fetchData()
  } catch (e) { toast.error(e.message) }
}

async function viewDetail(s) {
  try { selected.value = await api.get(`/sessions/${s.id}`); showDetail.value = true }
  catch { selected.value = s; showDetail.value = true }
}

async function deleteSession(id) {
  if (!confirm('Supprimer cette session ?')) return
  try { await api.delete(`/sessions/${id}`); toast.success('Session supprimée'); fetchData() }
  catch (e) { toast.error(e.message) }
}

async function pauseSession(s) {
  try { await api.put(`/sessions/${s.id}/pause`); toast.success('Session en pause'); fetchData() }
  catch (e) { toast.error(e.message) }
}

async function resumeSession(s) {
  try { await api.put(`/sessions/${s.id}/reprendre`); toast.success('Session reprise'); fetchData() }
  catch (e) { toast.error(e.message) }
}

async function endSession(s) {
  try {
    const result = await api.put(`/sessions/${s.id}/terminer`)
    toast.success(`Facture ${result.facture?.numero_facture} générée`)
    fetchData()
  } catch (e) { toast.error(e.message) }
}

onMounted(() => { fetchData(); fetchRefs() })
</script>
