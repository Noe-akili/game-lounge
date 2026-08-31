<template>
  <Modal :open="open" size="lg" @close="$emit('close')">
    <div class="p-4 sm:p-6">
      <h2 class="font-gaming text-xl sm:text-2xl font-bold mb-4 sm:mb-6 flex items-center gap-3">
        <div class="w-10 h-10 rounded-xl bg-neon-green/20 flex items-center justify-center shrink-0">
          <Play class="w-5 h-5 text-neon-green" />
        </div>
        Démarrer une session
      </h2>

      <div class="flex gap-1 sm:gap-2 mb-4 sm:mb-6 overflow-x-auto pb-2">
        <div v-for="(s, i) in steps" :key="i"
          class="flex items-center gap-1 sm:gap-2 px-3 sm:px-4 py-2 rounded-full text-xs sm:text-sm font-medium transition-all shrink-0"
          :class="step > i ? 'bg-neon-green/15 text-neon-green' : step === i ? 'bg-neon-violet/15 text-neon-violet border border-neon-violet/30' : 'bg-bg-surface text-txt-dim'">
          <span class="w-5 h-5 sm:w-6 sm:h-6 rounded-full flex items-center justify-center text-[10px] sm:text-xs font-bold shrink-0"
            :class="step > i ? 'bg-neon-green text-white' : step === i ? 'bg-neon-violet text-white' : 'bg-bg-hover text-txt-dim'">
            <Check v-if="step > i" class="w-3 h-3" /><span v-else>{{ i + 1 }}</span>
          </span>
          <span class="hidden sm:inline">{{ s }}</span>
        </div>
      </div>

      <div class="max-h-[50vh] overflow-y-auto pr-1">
        <!-- ÉTAPE 0: Joueur -->
        <div v-if="step === 0">
          <label class="block text-sm font-medium text-txt-muted mb-2">Rechercher un joueur</label>
          <div class="relative mb-3">
            <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-txt-dim" />
            <input v-model="searchQuery" placeholder="Nom ou téléphone..." class="input-field pl-10" @input="onSearch" />
          </div>
          <div class="space-y-2 max-h-40 overflow-y-auto mb-3">
            <button v-for="j in filteredJoueurs" :key="j.id"
              @click="selectedJoueur = j"
              class="w-full flex items-center gap-3 p-3 rounded-xl border transition-all text-left"
              :class="selectedJoueur?.id === j.id ? 'border-neon-violet bg-neon-violet/10' : 'border-white/5 hover:border-white/20 hover:bg-bg-hover'">
              <div class="w-10 h-10 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet font-bold text-sm shrink-0">
                {{ j.nom?.charAt(0) }}
              </div>
              <div class="flex-1 min-w-0">
                <p class="font-medium truncate">{{ j.nom }}</p>
                <p class="text-xs text-txt-dim truncate">{{ j.telephone }} — {{ j.jetons_solde || 0 }} jetons</p>
              </div>
              <ChevronRight class="w-4 h-4 text-txt-dim shrink-0" />
            </button>
            <p v-if="filteredJoueurs.length === 0 && searchQuery" class="text-center text-txt-dim py-4 text-sm">Aucun joueur trouvé</p>
          </div>
          <button @click="showNewPlayer = true" class="btn-neon-outline w-full flex items-center justify-center gap-2">
            <UserPlus class="w-4 h-4" /> Nouveau joueur
          </button>
        </div>

        <!-- ÉTAPE 1: Console -->
        <div v-if="step === 1">
          <p class="text-sm text-txt-dim mb-3">Consoles disponibles :</p>
          <div class="space-y-2">
            <button v-for="c in availableConsoles" :key="c.id"
              @click="selectedConsole = c"
              class="w-full flex items-center gap-3 p-3 sm:p-4 rounded-xl border transition-all text-left"
              :class="selectedConsole?.id === c.id ? 'border-neon-green bg-neon-green/10' : 'border-white/5 hover:border-white/20'">
              <Monitor class="w-5 h-5 sm:w-6 sm:h-6 shrink-0" :class="selectedConsole?.id === c.id ? 'text-neon-green' : 'text-txt-dim'" />
              <div class="flex-1 min-w-0">
                <p class="font-medium truncate">{{ c.nom }}</p>
                <p class="text-xs text-txt-dim">{{ c.type }} — Poste {{ c.poste_numero }}</p>
              </div>
              <span class="badge-green shrink-0">Libre</span>
            </button>
            <p v-if="availableConsoles.length === 0" class="text-center text-txt-dim py-6">Aucune console disponible</p>
          </div>
        </div>

        <!-- ÉTAPE 2: Jeu -->
        <div v-if="step === 2">
          <p class="text-sm text-txt-dim mb-3">Choisir un jeu <span class="text-neon-violet font-medium">{{ selectedConsole?.type }}</span> :</p>
          <div class="grid grid-cols-2 gap-2 sm:gap-3">
            <button v-for="j in jeux" :key="j.id"
              @click="selectedJeu = j"
              class="p-2 sm:p-3 rounded-xl border transition-all text-center"
              :class="selectedJeu?.id === j.id ? 'border-neon-violet bg-neon-violet/10' : 'border-white/5 hover:border-white/20'">
              <div class="w-12 h-12 sm:w-16 sm:h-16 mx-auto rounded-xl bg-bg-surface flex items-center justify-center mb-2">
                <Gamepad2 class="w-6 h-6 sm:w-8 sm:h-8" :class="selectedJeu?.id === j.id ? 'text-neon-violet' : 'text-txt-dim'" />
              </div>
              <p class="text-xs sm:text-sm font-medium truncate">{{ j.titre }}</p>
              <p class="text-[10px] sm:text-xs text-txt-dim">{{ j.genre }}</p>
            </button>
            <p v-if="jeux.length === 0" class="col-span-2 text-center text-txt-dim py-6">Aucun jeu disponible</p>
          </div>
        </div>

        <!-- ÉTAPE 3: Tarif -->
        <div v-if="step === 3">
          <p class="text-sm text-txt-dim mb-1">Session : <span class="text-neon-violet font-medium">{{ selectedJeu?.titre }}</span> sur <span class="text-neon-green font-medium">{{ selectedConsole?.nom }}</span></p>
          <p class="text-sm text-txt-dim mb-3">Choisir un tarif :</p>
          <div v-if="filteredTarifs.length === 0" class="text-center py-8 text-txt-dim">
            <p class="text-sm">Aucun tarif pour ce jeu</p>
            <p class="text-xs mt-1">Créez un tarif dans Paramètres → Tarifs</p>
          </div>
          <div v-else class="space-y-2">
            <button v-for="t in filteredTarifs" :key="t.id"
              @click="selectedTarif = t"
              class="w-full flex items-center justify-between p-3 sm:p-4 rounded-xl border transition-all"
              :class="selectedTarif?.id === t.id ? 'border-neon-blue bg-neon-blue/10' : 'border-white/5 hover:border-white/20'">
              <div class="min-w-0 text-left flex-1">
                <p class="font-medium text-sm truncate">{{ t.description || `${t.type} ${t.jeu || ''}` }}</p>
                <div class="flex items-center gap-2 mt-1">
                  <span class="text-xs px-2 py-0.5 rounded-full" :class="t.duree_minutes <= 15 ? 'bg-neon-yellow/15 text-neon-yellow' : t.duree_minutes <= 30 ? 'bg-neon-blue/15 text-neon-blue' : 'bg-neon-green/15 text-neon-green'">
                    {{ t.duree_minutes }}min
                  </span>
                  <span v-if="t.console_type" class="text-xs text-txt-dim">{{ t.console_type }}</span>
                </div>
              </div>
              <span class="font-gaming font-bold text-neon-green shrink-0 ml-3 text-lg">{{ formatCurrency(t.prix) }}</span>
            </button>
          </div>
        </div>
      </div>

      <div class="flex gap-3 mt-4 sm:mt-6 pt-4 border-t border-white/5">
        <button v-if="step > 0" @click="prevStep" class="btn-neon-outline flex-1">Retour</button>
        <button v-if="step < 3" @click="nextStep" :disabled="!canProceed"
          class="btn-neon-violet flex-1">Suivant</button>
        <button v-if="step === 3" @click="startSession" :disabled="!selectedTarif || loading"
          class="btn-neon-green flex-1 flex items-center justify-center gap-2">
          <Loader2 v-if="loading" class="w-5 h-5 animate-spin" />
          <Play v-else class="w-5 h-5" />
          Démarrer
        </button>
      </div>
    </div>
  </Modal>

  <Modal :open="showNewPlayer" @close="showNewPlayer = false">
    <div class="p-6">
      <h3 class="font-gaming text-xl font-bold mb-4">Nouveau joueur</h3>
      <div class="space-y-4">
        <input v-model="newPlayer.nom" placeholder="Nom complet" class="input-field" />
        <input v-model="newPlayer.telephone" placeholder="Téléphone" class="input-field" />
        <input v-model="newPlayer.email" placeholder="Email (optionnel)" class="input-field" />
        <div class="flex gap-3">
          <button @click="showNewPlayer = false" class="btn-neon-outline flex-1">Annuler</button>
          <button @click="createPlayer" :disabled="!newPlayer.nom || !newPlayer.telephone"
            class="btn-neon-violet flex-1">Créer</button>
        </div>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'
import { api } from '@/utils/api'
import { formatCurrency } from '@/utils/helpers'
import { toast } from 'sonner'
import Modal from './Modal.vue'
import { Play, Search, Check, ChevronRight, UserPlus, Monitor, Gamepad2, Loader2 } from 'lucide-vue-next'
import { isValidNom, isValidPhone, isValidEmail, sanitizeInput, isValidId } from '@/utils/validators'

const props = defineProps({ open: Boolean, console: Object })
const emit = defineEmits(['close', 'started'])

const steps = ['Joueur', 'Console', 'Jeu', 'Tarif']
const step = ref(0)
const loading = ref(false)
const searchQuery = ref('')
const joueurs = ref([])
const allConsoles = ref([])
const selectedJoueur = ref(null)
const selectedConsole = ref(null)
const selectedJeu = ref(null)
const selectedTarif = ref(null)
const jeux = ref([])
const tarifs = ref([])
const showNewPlayer = ref(false)
const newPlayer = reactive({ nom: '', telephone: '', email: '' })

const availableConsoles = computed(() => {
  return allConsoles.value.filter(c => c.etat === 'disponible' && !c.session_id)
})

const filteredJoueurs = computed(() => {
  if (!searchQuery.value) return joueurs.value.slice(0, 10)
  const q = searchQuery.value.toLowerCase()
  return joueurs.value.filter(j => j.nom?.toLowerCase().includes(q) || j.telephone?.includes(q))
})

const filteredTarifs = computed(() => {
  if (!selectedJeu.value || !selectedConsole.value) return []
  const consoleType = selectedConsole.value.type
  const jeuTitre = selectedJeu.value.titre
  return tarifs.value.filter(t => {
    const matchConsole = !t.console_type || t.console_type === consoleType
    const matchJeu = !t.jeu || t.jeu === jeuTitre
    return matchConsole && matchJeu
  })
})

const canProceed = computed(() => {
  if (step.value === 0) return !!selectedJoueur.value
  if (step.value === 1) return !!selectedConsole.value
  if (step.value === 2) return !!selectedJeu.value
  return true
})

watch(() => props.open, (v) => {
  if (v) {
    step.value = 0
    selectedJoueur.value = null
    selectedConsole.value = props.console || null
    selectedJeu.value = null
    selectedTarif.value = null
    searchQuery.value = ''
    loadData()
    if (props.console) {
      step.value = 2
      loadJeux(props.console.id)
    }
  }
})

async function loadData() {
  try {
    const [joueursData, consolesData, tarifsData] = await Promise.all([
      api.get('/joueurs'),
      api.get('/consoles'),
      api.get('/tarifs')
    ])
    joueurs.value = joueursData
    allConsoles.value = consolesData
    tarifs.value = tarifsData
  } catch (e: any) {
    toast.error('Erreur chargement données: ' + (e.message || 'Serveur indisponible'))
  }
}

async function onSearch() {
  if (searchQuery.value.length >= 2) {
    try { joueurs.value = await api.get(`/joueurs?search=${searchQuery.value}`) }
    catch (e: any) { toast.error('Recherche: ' + (e.message || 'erreur')) }
  } else {
    try { joueurs.value = await api.get('/joueurs') }
    catch (e: any) { toast.error('Joueurs: ' + (e.message || 'erreur')) }
  }
}

function nextStep() {
  if (step.value === 1 && selectedConsole.value) {
    loadJeux(selectedConsole.value.id)
    selectedJeu.value = null
    selectedTarif.value = null
  }
  if (step.value === 2 && selectedJeu.value) {
    selectedTarif.value = null
  }
  step.value++
}

function prevStep() {
  step.value--
}

async function loadJeux(consoleId) {
  try { jeux.value = await api.get(`/jeux?console_id=${consoleId}`) }
  catch (e: any) { toast.error('Jeux: ' + (e.message || 'erreur')) }
}

async function createPlayer() {
  if (!isValidNom(newPlayer.nom)) return toast.error('Nom invalide (2-50 caractères)')
  if (!isValidPhone(newPlayer.telephone)) return toast.error('Téléphone invalide (8-15 chiffres)')
  if (newPlayer.email && !isValidEmail(newPlayer.email)) return toast.error('Email invalide')
  newPlayer.nom = sanitizeInput(newPlayer.nom, 50)
  if (newPlayer.email) newPlayer.email = sanitizeInput(newPlayer.email, 100)
  try {
    const player = await api.post('/joueurs', newPlayer)
    selectedJoueur.value = player
    joueurs.value.push(player)
    showNewPlayer.value = false
    toast.success('Joueur créé !')
    Object.assign(newPlayer, { nom: '', telephone: '', email: '' })
  } catch (e) { toast.error(e.message) }
}

async function startSession() {
  if (!isValidId(selectedConsole.value?.id)) return toast.error('Console invalide')
  if (!isValidId(selectedJoueur.value?.id)) return toast.error('Joueur invalide')
  if (!isValidId(selectedJeu.value?.id)) return toast.error('Jeu invalide')
  if (!selectedTarif.value) return toast.error('Veuillez choisir un tarif')
  loading.value = true
  try {
    await api.post('/sessions', {
      console_id: selectedConsole.value.id,
      joueur_id: selectedJoueur.value.id,
      jeu_id: selectedJeu.value.id,
      tarif_id: selectedTarif.value.id,
    })
    emit('started')
    emit('close')
    toast.success('Session démarrée !')
  } catch (e: any) {
    toast.error(e.message)
  } finally {
    loading.value = false
  }
}
</script>
