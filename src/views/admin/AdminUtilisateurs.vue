<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full max-w-full min-w-0">
      <div>
        <h3 class="font-gaming text-lg font-bold">Utilisateurs</h3>
        <p class="text-xs text-txt-dim">Gestion centralisée des comptes sur Supabase</p>
      </div>

      <div class="flex items-center gap-3 flex-wrap">
        <label class="flex items-center gap-2 cursor-pointer text-xs text-txt-dim bg-bg-card px-3 py-1.5 rounded-lg border border-border/50 hover:border-neon-violet/30 transition-colors">
          <input type="checkbox" v-model="showArchived" @change="fetchUsers" class="rounded accent-neon-violet cursor-pointer" />
          <span>Afficher archivés</span>
        </label>

        <button @click="openAdd" class="btn-neon-violet flex items-center gap-2 text-sm">
          <UserPlus class="w-4 h-4" /> Ajouter un utilisateur
        </button>
      </div>
    </div>

    <div v-if="listLoading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des utilisateurs..." />
    </div>

    <div v-else-if="users.length === 0" class="card w-full max-w-full min-w-0 overflow-hidden text-center py-12">
      <p class="text-txt-dim">Aucun utilisateur trouvé</p>
    </div>

    <div v-else class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
      <div v-for="u in users" :key="u.id" 
        class="card flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden flex-wrap transition-colors cursor-pointer"
        :class="u.deleted ? 'opacity-60 border-dashed border-border/70 hover:border-txt-dim' : 'hover:border-neon-violet/20'"
        @click="viewUser(u)">
        <div class="w-11 h-11 rounded-full flex items-center justify-center font-bold shrink-0"
          :class="u.deleted ? 'bg-txt-dim/20 text-txt-dim' : (u.role === 'admin' ? 'bg-neon-violet/20 text-neon-violet' : 'bg-neon-blue/20 text-neon-blue')">
          {{ u.nom?.charAt(0) }}
        </div>
        <div class="flex-1 min-w-0 overflow-hidden">
          <div class="flex items-center gap-2">
            <p class="font-medium truncate" :class="{ 'line-through text-txt-dim': u.deleted }">{{ u.nom }}</p>
            <span v-if="u.deleted" class="text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded bg-neon-red/10 text-neon-red border border-neon-red/20">Archivé</span>
          </div>
          <p class="text-xs text-txt-dim truncate">{{ u.email }}</p>
        </div>
        
        <span class="badge shrink-0 max-w-full truncate" :class="u.deleted ? 'badge-gray' : (u.role === 'admin' ? 'badge-violet' : 'badge-blue')">
          {{ u.role === 'admin' ? 'Admin' : 'Employé' }}
        </span>

        <div class="flex gap-1 shrink-0 flex-wrap" @click.stop>
          <template v-if="u.deleted">
            <button @click="restoreUser(u.id)" class="p-2 rounded-lg hover:bg-neon-green/10 text-txt-dim hover:text-neon-green transition-colors" title="Restaurer le compte">
              <RotateCcw class="w-4 h-4" />
            </button>
            <button @click="permanentDeleteUser(u.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-txt-dim hover:text-neon-red transition-colors" title="Supprimer définitivement de Supabase">
              <Trash2 class="w-4 h-4 text-neon-red" />
            </button>
          </template>
          <template v-else>
            <button @click="editUser(u)" class="p-2 rounded-lg hover:bg-bg-hover text-txt-dim transition-colors" title="Modifier">
              <Pencil class="w-4 h-4" />
            </button>
            <button v-if="u.id !== currentUserId" @click="deleteUser(u.id)" class="p-2 rounded-lg hover:bg-neon-red/10 text-txt-dim hover:text-neon-red transition-colors" title="Archiver">
              <Archive class="w-4 h-4" />
            </button>
          </template>
        </div>
      </div>
    </div>

    <!-- Modal Formulaire Création / Modification -->
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

    <!-- Modal Détail Utilisateur -->
    <Modal :open="showDetail" @close="showDetail = false">
      <div class="p-6" v-if="selected">
        <div class="flex items-center justify-between mb-4">
          <h3 class="font-gaming text-xl font-bold">Détails utilisateur</h3>
          <span v-if="selected.deleted" class="text-xs uppercase font-bold tracking-wider px-2 py-0.5 rounded bg-neon-red/10 text-neon-red border border-neon-red/20">Archivé</span>
        </div>
        <div class="space-y-3">
          <div class="flex justify-between"><span class="text-txt-dim">Nom</span><span class="font-medium">{{ selected.nom }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Email</span><span class="font-mono text-sm">{{ selected.email }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Rôle</span><span class="badge" :class="selected.role === 'admin' ? 'badge-violet' : 'badge-blue'">{{ selected.role }}</span></div>
          <div class="flex justify-between"><span class="text-txt-dim">Créé le</span><span class="text-sm">{{ formatDate(selected.created_at) }}</span></div>
        </div>
        
        <div class="flex gap-3 mt-6">
          <button @click="showDetail = false" class="btn-neon-outline flex-1">Fermer</button>
          <template v-if="selected.deleted">
            <button @click="restoreUser(selected.id); showDetail = false" class="btn-neon-green flex-1 flex items-center justify-center gap-2">
              <RotateCcw class="w-4 h-4" /> Restaurer
            </button>
            <button @click="permanentDeleteUser(selected.id); showDetail = false" class="btn-neon-red flex-1 flex items-center justify-center gap-2">
              <Trash2 class="w-4 h-4" /> Supprimer définitif
            </button>
          </template>
          <template v-else>
            <button @click="editUser(selected); showDetail = false" class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
              <Pencil class="w-4 h-4" /> Modifier
            </button>
          </template>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed } from 'vue'
import { api } from '@/utils/api'
import { useAuthStore } from '@/stores/auth'
import { toast } from 'vue-sonner'
import Modal from '@/components/ui/Modal.vue'
import { UserPlus, Trash2, Loader2, Pencil, RotateCcw, Archive } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { formatDate } from '@/utils/helpers'
import { isValidEmail, isValidPassword, isValidNom, isValidRole, sanitizeInput } from '@/utils/validators'

const users = ref<any[]>([])
const showForm = ref(false)
const showDetail = ref(false)
const selected = ref<any>(null)
const editingId = ref<number | null>(null)
const loading = ref(false)
const listLoading = ref(true)
const showArchived = ref(false)
const auth = useAuthStore()
const currentUserId = computed(() => auth.user?.id)

const form = reactive({ nom: '', email: '', password: '', role: 'employe' })

async function fetchUsers() {
  listLoading.value = true
  try {
    const query = showArchived.value ? '?include_deleted=1' : ''
    users.value = await api.get(`/users${query}`)
  } catch (e: any) {
    toast.error('Erreur chargement utilisateurs: ' + (e.message || 'Connectez-vous à internet'))
  } finally {
    listLoading.value = false
  }
}

function openAdd() {
  editingId.value = null
  Object.assign(form, { nom: '', email: '', password: '', role: 'employe' })
  showForm.value = true
}

function editUser(u: any) {
  editingId.value = u.id
  Object.assign(form, { nom: u.nom, email: u.email, password: '', role: u.role })
  showForm.value = true
}

function closeForm() {
  showForm.value = false
  editingId.value = null
}

async function viewUser(u: any) {
  // Ouvrir immédiatement avec la ligne locale. Aucun appel réseau/IPC ne doit
  // bloquer l'ouverture du détail.
  selected.value = u
  showDetail.value = true
  try {
    selected.value = await api.get(`/users/${u.id}`)
  } catch {
    // La donnée locale reste affichée.
  }
}

async function saveUser() {
  if (!isValidNom(form.nom)) return toast.error('Nom invalide (2-50 caractères)')
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
      toast.success('Utilisateur mis à jour !')
    } else {
      await api.post('/users', form)
      toast.success('Utilisateur créé avec succès !')
    }
    closeForm()
    Object.assign(form, { nom: '', email: '', password: '', role: 'employe' })
    await fetchUsers()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Opération impossible'))
  } finally {
    loading.value = false
  }
}

async function deleteUser(id: number) {
  if (!confirm('Archiver cet utilisateur ?')) return
  try {
    await api.delete(`/users/${id}`)
    toast.success('Utilisateur archivé localement')
    await fetchUsers()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Échec'))
  }
}

async function restoreUser(id: number) {
  try {
    await api.post(`/users/${id}/restore`, {})
    toast.success('Utilisateur restauré localement !')
    await fetchUsers()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de restaurer'))
  }
}

async function permanentDeleteUser(id: number) {
  if (!confirm('ATTENTION : Supprimer DÉFINITIVEMENT cet utilisateur de Supabase ? Cette action est irréversible.')) return
  try {
    await api.delete(`/users/${id}/permanent`)
    toast.success('Utilisateur supprimé définitivement')
    await fetchUsers()
  } catch (e: any) {
    toast.error('Erreur: ' + (e.message || 'Impossible de supprimer'))
  }
}

onMounted(fetchUsers)
</script>
