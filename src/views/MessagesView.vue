<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex items-center justify-between gap-2">
      <h3 class="font-gaming text-lg font-bold">Messages</h3>
      <button @click="showForm = true" class="btn-neon-violet flex items-center gap-2 text-sm shrink-0">
        <Plus class="w-4 h-4" /> Nouveau
      </button>
    </div>
    <div v-if="loading" class="card">
      <Loader variant="neon" size="lg" text="Chargement des messages..." />
    </div>
    <div v-else-if="messages.length === 0" class="card text-center py-12">
      <MessageSquare class="w-12 h-12 text-txt-dim mx-auto mb-3" />
      <p class="text-txt-dim">Aucun message</p>
      <p class="text-xs text-txt-dim mt-1">Créez un message pour l'équipe</p>
    </div>
    <div v-else class="space-y-2">
      <div v-for="m in messages" :key="m.id"
        class="card flex items-start gap-3 w-full max-w-full min-w-0 overflow-hidden cursor-pointer select-none transition-colors"
        :class="sel.estSelectionne(m.id) ? 'ring-2 ring-neon-violet bg-neon-violet/5 border-neon-violet/40' : ''"
        v-on="sel.press(m, () => editMessage(m))">
        <div v-if="sel.actif" class="w-5 h-5 mt-2 rounded-md border-2 flex items-center justify-center shrink-0"
          :class="sel.estSelectionne(m.id) ? 'bg-neon-violet border-neon-violet' : 'border-txt-dim/50'">
          <Check v-if="sel.estSelectionne(m.id)" class="w-3 h-3 text-white" />
        </div>
        <div class="w-10 h-10 rounded-xl bg-neon-violet/20 flex items-center justify-center shrink-0">
          <MessageSquare class="w-5 h-5 text-neon-violet" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="font-medium truncate">{{ m.titre || 'Message' }}</p>
          <p class="text-sm text-txt-dim break-words">{{ m.contenu }}</p>
          <p class="text-xs text-txt-dim mt-1">{{ formatDate(m.created_at) }} · {{ m.auteur || 'Système' }}</p>
        </div>
        <div v-if="!sel.actif" class="flex gap-1 shrink-0" @click.stop>
          <button @click="editMessage(m)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim"><Pencil class="w-4 h-4" /></button>
          <button @click="deleteMessage(m.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-neon-red"><Trash2 class="w-4 h-4" /></button>
        </div>
      </div>
    </div>

    <SelectionBar :sel="sel" />

    <Modal :open="showForm" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Nouveau' }} message</h3>
        <div class="space-y-4">
          <input v-model="form.titre" placeholder="Titre" class="input-field" />
          <textarea v-model="form.contenu" placeholder="Contenu du message..." rows="4" class="input-field resize-none"></textarea>
          <div class="flex gap-3">
            <button @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button @click="saveMessage" :disabled="!form.contenu" class="btn-neon-violet flex-1">{{ editingId ? 'Modifier' : 'Envoyer' }}</button>
          </div>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted } from 'vue'
import { api } from '@/utils/api'
import { toast } from 'vue-sonner'
import { formatDate } from '@/utils/helpers'
import Modal from '@/components/ui/Modal.vue'
import Loader from '@/components/ui/Loader.vue'
import { MessageSquare, Plus, Pencil, Trash2, Check } from 'lucide-vue-next'
import SelectionBar from '@/components/ui/SelectionBar.vue'
import { useMultiSelect } from '@/composables/useMultiSelect'
import { isValidTitre, isValidContenu, sanitizeInput } from '@/utils/validators'

const loading = ref(true)
const messages = ref<any[]>([])
const showForm = ref(false)
const editingId = ref<number | null>(null)
const form = reactive({ titre: '', contenu: '' })

// SÉLECTION MULTIPLE : appui long sur un message puis appuis simples.
// Les messages n'ont pas de corbeille : la suppression est directe.
const sel = useMultiSelect({
  nomSingulier: 'message',
  nomPluriel: 'messages',
  liste: () => messages.value,
  supprimer: (m: any) => api.delete(`/messages/${m.id}/permanent`),
  apres: fetchData,
})

async function fetchData() {
  loading.value = true
  try {
    // Messages are stored locally if no backend endpoint, fallback to localStorage
    try {
      const data = await api.get('/messages')
      messages.value = Array.isArray(data) ? data : []
    } catch {
      const local = localStorage.getItem('gl_messages')
      messages.value = local ? JSON.parse(local) : []
    }
  } finally { loading.value = false }
}

function saveLocal() {
  localStorage.setItem('gl_messages', JSON.stringify(messages.value))
}

function closeForm() { showForm.value = false; editingId.value = null; Object.assign(form, { titre: '', contenu: '' }) }

async function saveMessage() {
  if (!isValidContenu(form.contenu)) return toast.error('Contenu invalide (1-1000 caractères, < > interdits)')
  if (form.titre && !isValidTitre(form.titre)) return toast.error('Titre invalide (2-100 caractères)')
  form.titre = sanitizeInput(form.titre, 100)
  form.contenu = sanitizeInput(form.contenu, 1000)
  try {
    if (editingId.value) {
      try {
        await api.put(`/messages/${editingId.value}`, form)
      } catch (e: any) {
        toast.error('Erreur modification: ' + (e.message || 'inconnue'))
      }
      const idx = messages.value.findIndex((m: any) => m.id === editingId.value)
      if (idx !== -1) messages.value[idx] = { ...messages.value[idx], ...form }
    } else {
      try {
        const res = await api.post('/messages', form)
        messages.value.unshift(res)
      } catch (e: any) {
        toast.error('Erreur envoi: ' + (e.message || 'Serveur indisponible'))
        const newMsg: any = { id: Date.now(), titre: form.titre || 'Message', contenu: form.contenu, auteur: 'Moi', created_at: new Date().toISOString() }
        messages.value.unshift(newMsg)
      }
    }
    saveLocal()
    toast.success(editingId.value ? 'Message modifié' : 'Message envoyé')
    closeForm()
  } catch (e: any) { toast.error(e.message) }
}

function editMessage(m: any) {
  editingId.value = m.id
  Object.assign(form, { titre: m.titre || '', contenu: m.contenu || '' })
  showForm.value = true
}

async function deleteMessage(id: number) {
  if (!confirm('Supprimer ce message ?')) return
  try {
    await api.delete(`/messages/${id}/permanent`)
    messages.value = messages.value.filter((m: any) => m.id !== id)
    saveLocal()
    toast.success('Message supprimé')
  } catch (e: any) {
    toast.error('Erreur suppression: ' + (e.message || 'inconnue'))
  }
}

onMounted(() => {
  fetchData()
  // Auto-refresh when sync poll detects new messages
  window.addEventListener('sync-poll', onSyncPoll)
})
onUnmounted(() => window.removeEventListener('sync-poll', onSyncPoll))

function onSyncPoll(e: Event) {
  const detail = (e as CustomEvent).detail
  if (detail?.changes?.messages) {
    // Refresh messages from server
    api.get('/messages').then(data => {
      if (Array.isArray(data)) messages.value = data
    }).catch(() => {})
  }
}
</script>
