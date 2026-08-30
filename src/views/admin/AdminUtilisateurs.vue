<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Utilisateurs</h3>
      <button @click="openAdd" class="btn-neon-violet flex items-center gap-2 text-sm">
        <UserPlus class="w-4 h-4" /> Ajouter un utilisateur
      </button>
    </div>

    <div v-if="listLoading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des utilisateurs..." />
    </div>

    <div v-else-if="users.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <p class="text-txt-dim">Aucun utilisateur</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="u in users" :key="u.id" class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap hover:border-neon-violet/20 transition-colors cursor-pointer" @click="viewUser(u)">
        <div class="w-11 h-11 rounded-full flex items-center justify-center font-bold shrink-0"
          :class="u.role === 'admin' ? 'bg-neon-violet/20 text-neon-violet' : 'bg-neon-blue/20 text-neon-blue'">
          {{ u.nom?.charAt(0) }}
        </div>
        <div class="flex-1 min-w-0 overflow-hidden"><p class="font-medium truncate">{{ u.nom }}</p><p class="text-xs text-txt-dim truncate">{{ u.email }}</p></div>
        <span class="badge shrink-0 max-w-full truncate" :class="u.role === 'admin' ? 'badge-violet' : 'badge-blue'">{{ u.role === 'admin' ? 'Admin' : 'Employé' }}</span>
        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <button @click="editUser(u)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
            <Pencil class="w-4 h-4" />
          </button>
          <button v-if="u.id !== currentUserId" @click="deleteUser(u.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-txt-dim hover:text-neon-red transition-colors" title="Supprimer">
            <Trash2 class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <Modal :open="showForm" @close="closeForm">
      <div class="p-6">
        <h3 class="font-gaming text-xl font-bold mb-4">{{ editingId ? 'Modifier' : 'Nouvel' }} utilisateur</h3>
        <form @submit.prevent="saveUser" class="space-y-4">
          <input v-model="form.nom" placeholder="Nom complet" class="input-field" required />
          <input v-model="form.email" type="email" placeholder="Email" class="input-field" required />
          <input v-model="form.password" type="password" :placeholder="editingId ? 'Nouveau mot de passe (laisser vide pour ne pas changer)' : 'Mot de passe (min 6 caractères)'" class="input-field" :required="!editingId" />
          <select v-model="form.role" class="input-field" required>
            <option value="employe">Employé</option>
            <option value="admin">Administrateur</option>
          </select>
          <div class="flex gap-3 pt-2">
            <button type="button" @click="closeForm" class="btn-neon-outline flex-1">Annuler</button>
            <button type="submit" :disabled="loading" class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
              <Loader2 v-if="loading" class="w-4 h-4 animate-spin" />
              {{ editingId ? 'Modifier' : 'Créer' }}
            </button>
          </div>
        </form>
      </div>
    </Modal>

    <Modal :open="showDetail" @close="showDetail = false">
      <div class="p-6" v-if="selected">
        <h3 class="font-gaming text-xl font-bold mb-4">Détails utilisateur</h3>
        <div class="space-y-3">
          <div class="flex justify-between"><span class="text-txt-dim">Nom</span><span class="font-medium">{{ selected.nom }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Email</span><span class="font-mono text-sm">{{ selected.email }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Rôle</span><span class="badge" :class="selected.role === 'admin' ? 'badge-violet' : 'badge-blue'">{{ selected.role }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Créé le</span><span class="text-sm">{{ formatDate(selected.created_at) }}</span></div>
        </div>
        <div class="flex gap-3 mt-6">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <button @click="editUser(selected); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2"><Pencil class="w-4 h-4" /> Modifier</button>
        </div>
      </div>
    </Modal>
    <!-- responsive table overflow helper: ensures horizontal scroll on mobile -->
    <div class="w-full overflow-x-auto -mx-4 sm:mx-0 hidden" aria-hidden="true"><table class="min-w-[600px] w-full"><tbody><tr><td></td></tr></tbody></table></div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed } from 'vue'
import { api } from '@/utils/api'
import { useAuthStore } from '@/stores/auth'
import { toast } from 'sonner'
import Modal from '@/components/ui/Modal.vue'
import { UserPlus, Trash2, Loader2, Pencil } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { formatDate } from '@/utils/helpers'
import { isValidEmail, isValidPassword, isValidNom, isValidRole, sanitizeInput } from '@/utils/validators'

const users = ref([])
const showForm = ref(false)
const showDetail = ref(false)
const selected = ref(null)
const editingId = ref(null)
const loading = ref(false)
const listLoading = ref(true)
const auth = useAuthStore()
const currentUserId = computed(() => auth.user?.id)

const form = reactive({ nom: '', email: '', password: '', role: 'employe' })

async function fetchUsers() {
  listLoading.value = true
  try { users.value = await api.get('/users') } catch {} finally { listLoading.value = false }
}

function openAdd() {
  editingId.value = null
  Object.assign(form, { nom: '', email: '', password: '', role: 'employe' })
  showForm.value = true
}

function editUser(u) {
  editingId.value = u.id
  Object.assign(form, { nom: u.nom, email: u.email, password: '', role: u.role })
  showForm.value = true
}

function closeForm() {
  showForm.value = false
  editingId.value = null
}

async function viewUser(u) {
  try {
    selected.value = await api.get(`/users/${u.id}`)
    showDetail.value = true
  } catch { selected.value = u; showDetail.value = true }
}

async function saveUser() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères, lettres/chiffres/ -\'&)')
  if (!isValidEmail(form.email)) return toast.error('Email invalide')
  if (!isValidRole(form.role)) return toast.error('Rôle invalide')
  if (!editingId.value && !isValidPassword(form.password)) return toast.error('Mot de passe invalide (min 6 caractères, au moins une lettre)')
  if (editingId.value && form.password && !isValidPassword(form.password)) return toast.error('Mot de passe invalide (min 6 caractères, au moins une lettre)')
  form.nom = sanitizeInput(form.nom, 50)
  loading.value = true
  try {
    if (editingId.value) {
      const payload: any = { nom: form.nom, email: form.email, role: form.role }
      if (form.password) payload.password = form.password
      await api.put(`/users/${editingId.value}`, payload)
      toast.success('Utilisateur modifié !')
    } else {
      await api.post('/users', form)
      toast.success('Utilisateur créé !')
    }
    closeForm()
    Object.assign(form, { nom: '', email: '', password: '', role: 'employe' })
    await fetchUsers()
  } catch (e) { toast.error(e.message) }
  finally { loading.value = false }
}

async function deleteUser(id) {
  if (!confirm('Supprimer cet utilisateur ?')) return
  try {
    await api.delete(`/users/${id}`)
    toast.success('Utilisateur supprimé')
    await fetchUsers()
  } catch (e) { toast.error(e.message) }
}

onMounted(fetchUsers)
</script>
