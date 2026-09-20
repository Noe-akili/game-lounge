<template>
  <motion.div
    :initial="{ opacity: 0, y: 12 }"
    :animate="{ opacity: 1, y: 0 }"
    :whileHover="{ y: -4, scale: 1.01 }"
    :transition="{ duration: 0.25, ease: 'easeOut' }"
    class="card-hover relative isolate overflow-hidden group cursor-pointer w-full max-w-full min-w-0 box-border"
  >
    <motion.div class="absolute top-0 left-0 w-full h-1" :class="statusBarColor" :animate="depasse ? { opacity: [1, 0.5, 1] } : {}" :transition="{ duration: 1.5, repeat: Infinity }" />

    <!-- Image de couverture (item 6) : visuel principal en arrière-plan, texte
         lisible par-dessus via un dégradé ; fallback icône si absente/invalide. -->
    <div v-if="imageSrc && !imgFailed" class="absolute inset-0 z-0">
      <img :src="imageSrc" alt="" referrerpolicy="no-referrer" loading="lazy" decoding="async"
        class="w-full h-full object-cover" @error="imgFailed = true" />
      <div class="absolute inset-0 bg-gradient-to-t from-bg/95 via-bg/80 to-bg/50"></div>
    </div>

    <div class="relative z-10 flex items-center justify-between gap-2 mb-3 min-w-0">
      <div class="flex items-center gap-2 min-w-0 flex-1">
        <Monitor v-if="imgFailed || !imageSrc" class="w-5 h-5 shrink-0" :class="statusIconColor" />
        <h3 class="font-gaming font-bold text-txt truncate min-w-0">{{ poste.nom }}</h3>
      </div>
      <span class="badge shrink-0" :class="statusBadgeClass">{{ statusLabel }}</span>
    </div>

    <!-- Sur une photo, le gris clair devient illisible : on remonte le contraste. -->
    <p class="relative z-10 text-xs mb-3 truncate" :class="imageSrc && !imgFailed ? 'text-txt/90' : 'text-txt-dim'">
      Poste {{ poste.poste_numero }} — {{ poste.type }}
    </p>

    <div v-if="isActive" class="relative z-10 mb-4 space-y-2 min-w-0">
      <div class="flex items-center gap-2 min-w-0">
        <div class="w-8 h-8 rounded-full bg-white/5 flex items-center justify-center shrink-0">
          <User class="w-4 h-4 text-txt-dim" />
        </div>
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium truncate">{{ poste.joueur_nom || 'Joueur' }}</p>
          <p class="text-xs text-txt-dim truncate">{{ poste.jeu_nom || 'Jeu' }}</p>
        </div>
      </div>

      <!-- Temps RÉEL + progression vers la fin de la session : la barre avance
           chaque seconde et se remplit à mesure que le temps alloué s'écoule.
           Temps dépassé = barre et chrono en rouge (+X pulsé) : la session sera
           terminée automatiquement par le backend (notification Android envoyée). -->
      <div class="bg-bg-surface rounded-xl p-3 min-w-0">
        <div class="flex items-center gap-2 min-w-0">
          <Timer class="w-5 h-5 shrink-0" :class="depasse ? 'text-neon-red' : 'text-txt-dim'" />
          <!-- COMPTE À REBOURS : c'est le temps qu'il RESTE sur le tarif payé qui
               est affiché en grand (mode="remaining"). Le temps déjà joué passe en
               seconde ligne. Le chrono n'est plus caché derrière un petit
               « Reste … » : l'employé et le joueur voient la même chose. -->
          <div class="min-w-0 flex-1">
            <LiveSessionTimer
              ref="liveTimer"
              mode="remaining"
              :session-debut="poste.session_debut"
              :duree-allouee="poste.duree_allouee || 0"
              :accum-sec="poste.duree_secondes ?? -1"
              :accumulee-min="poste.duree_minutes || 0"
              :statut="poste.session_statut || 'en_cours'"
              class="text-2xl sm:text-3xl tabular-nums block leading-none"
            />
            <p class="text-[10px] text-txt-dim mt-1 truncate">
              <template v-if="depasse">Temps écoulé — terminaison auto…</template>
              <template v-else-if="hasAllocation">
                restant sur {{ poste.duree_allouee }} min — fin {{ finPrevue }}
              </template>
              <template v-else>aucune durée allouée</template>
            </p>
          </div>
          <span class="shrink-0 text-right text-[10px] text-txt-dim leading-tight">
            joué<br /><span class="font-gaming text-sm text-txt">{{ ecouleAffiche }}</span>
          </span>
        </div>
        <div v-if="hasAllocation" class="mt-2 h-1.5 rounded-full bg-white/5 overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-1000"
            :class="depasse ? 'bg-neon-red' : restant < 5 * 60 ? 'bg-neon-yellow' : 'bg-neon-violet/70'"
            :style="{ width: progressPct + '%' }"
          />
        </div>
      </div>

      <div v-if="isOccupied" class="relative z-10 flex items-center gap-2 text-sm text-txt-dim min-w-0">
        <TrendingUp class="w-4 h-4 shrink-0" />
        <!-- Le tarif est un FORFAIT (prix pour la durée allouée), pas un prix
             horaire : afficher « /h » sur un forfait de 5 min induisait en erreur. -->
        <span class="truncate">
          Forfait : {{ formatCurrency(poste.tarif_prix || 0) }}<template v-if="hasAllocation"> / {{ poste.duree_allouee }} min</template>
        </span>
        <span class="ml-auto font-semibold text-txt shrink-0">{{ formatCurrency(montantActuel) }}</span>
      </div>
    </div>

    <div class="relative z-10 flex gap-2 flex-wrap sm:flex-nowrap min-w-0">
      <button v-if="isFree" @click="$emit('start', poste)"
        class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
        <Play class="w-5 h-5" />
        <span>Démarrer</span>
      </button>

      <template v-if="isOccupied">
        <button @click="$emit('pause', poste)"
          class="btn-neon flex-1 bg-bg-surface text-txt-dim border border-white/10 hover:bg-bg-hover flex items-center justify-center gap-2">
          <Pause class="w-5 h-5" />
          <span>Pause</span>
        </button>
        <button @click="$emit('end', poste)"
          class="btn-neon-red flex-1 flex items-center justify-center gap-2">
          <Square class="w-5 h-5" />
          <span>Terminer</span>
        </button>
      </template>

      <button v-if="isPaused" @click="$emit('resume', poste)"
        class="btn-neon-violet flex-1 flex items-center justify-center gap-2">
        <Play class="w-5 h-5" />
        <span>Reprendre</span>
      </button>
    </div>
  </motion.div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { motion } from 'motion-v'
import { Monitor, User, Timer, TrendingUp, Play, Pause, Square } from 'lucide-vue-next'
import { formatCurrency, formatDuration, normalizeImageUrl } from '@/utils/helpers'
import LiveSessionTimer from './LiveSessionTimer.vue'

const props = defineProps({
  console: { type: Object, required: true },
})

defineEmits(['start', 'pause', 'resume', 'end'])

// IMPORTANT : dans un template Vue, `console` est résolu comme l'objet GLOBAL
// console (liste des globaux du compilateur), jamais comme la prop -> le nom, le
// numéro de poste, le joueur et le jeu s'affichaient vides. On expose donc un
// alias `poste` utilisé par tout le template.
const poste = computed(() => props.console || {})

// Chrono temps réel partagé (LiveSessionTimer expose elapsedSeconds/restant).
const liveTimer = ref(null)
// Image de couverture : si l'URL échoue à charger, on retire l'img du DOM
// (fallback propre -> carte classique avec icône).
const imgFailed = ref(false)
// Lien d'image corrigé à l'affichage (schéma manquant, espaces, //cdn…).
const imageSrc = computed(() => normalizeImageUrl(props.console?.image_url))
// Un changement de console (ou de lien) doit redonner une chance à l'image.
watch(imageSrc, () => { imgFailed.value = false })
const elapsed = computed(() => liveTimer.value?.elapsedSeconds ?? 0)
const restant = computed(() => liveTimer.value?.restant ?? 0)
const depasse = computed(() => liveTimer.value?.depasse ?? false)
const hasAllocation = computed(() => (props.console.duree_allouee || 0) > 0)
const progressPct = computed(() => {
  if (!hasAllocation.value) return 0
  return Math.min(100, Math.max(0, (elapsed.value / (props.console.duree_allouee * 60)) * 100))
})
// Temps déjà joué (affiché en petit à droite du compte à rebours).
const ecouleAffiche = computed(() => formatDuration(Math.max(0, elapsed.value)))
// Heure de fin prévue = début + durée du tarif (affichage local 24 h).
const finPrevue = computed(() => {
  if (!hasAllocation.value || !props.console?.session_debut) return '—'
  const debut = new Date(props.console.session_debut).getTime()
  if (Number.isNaN(debut)) return '—'
  const accum = (props.console.duree_secondes ?? 0) * 1000
  const fin = new Date(debut + props.console.duree_allouee * 60000 - accum)
  return fin.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })
})

const hasActiveSession = computed(() => !!props.console.session_id && (props.console.session_statut === 'en_cours' || props.console.session_statut === 'pause'))
const isActive = computed(() => hasActiveSession.value)
const isOccupied = computed(() => hasActiveSession.value && props.console.session_statut === 'en_cours')
const isPaused = computed(() => hasActiveSession.value && props.console.session_statut === 'pause')
const isFree = computed(() => !hasActiveSession.value && props.console.etat !== 'maintenance' && props.console.etat !== 'hors_service')

const montantActuel = computed(() => {
  // MÊME règle que le backend (compute_montant) : forfait choisi + dépassement
  // prorata sur le taux du forfait. Avant : montant "horaire" qui démarrait à
  // 0 FC et grimpait -> contre-sens pour un forfait déjà payé d'avance.
  const tarif = props.console.tarif_prix || 2000
  const allouee = props.console.duree_allouee || 0
  if (allouee <= 0) return Math.ceil(elapsed.value / 3600) * tarif
  const joueeMin = Math.ceil(elapsed.value / 60)
  if (joueeMin <= allouee) return tarif
  const depassement = joueeMin - allouee
  return tarif + Math.ceil((depassement * tarif) / allouee)
})

const statusLabel = computed(() => {
  if (props.console.session_statut === 'en_cours' || props.console.etat === 'occupee') return 'Occupé'
  if (props.console.session_statut === 'pause' || props.console.etat === 'pause') return 'En pause'
  if (props.console.etat === 'maintenance') return 'Maintenance'
  if (props.console.etat === 'hors_service') return 'Hors service'
  return 'Libre'
})

// PALETTE SOBRE : la couleur ne sert plus qu'à signaler ce qui demande une
// action. Une console occupée est un état NORMAL (neutre/bleu), le jaune est
// réservé à la pause, et le rouge au temps dépassé — avant, tout était coloré
// (rouge pour « occupé », vert, violet, jaune) et l'écran devenait illisible.
const statusBadgeClass = computed(() => {
  if (depasse.value) return 'badge-red'
  if (isPaused.value) return 'badge-yellow'
  if (isOccupied.value) return 'badge-blue'
  return 'badge-gray'
})

const statusBarColor = computed(() => {
  if (depasse.value) return 'bg-neon-red'
  if (isPaused.value) return 'bg-neon-yellow'
  if (isOccupied.value) return 'bg-neon-blue/60'
  return 'bg-white/10'
})

const statusIconColor = computed(() => {
  if (depasse.value) return 'text-neon-red'
  if (isPaused.value) return 'text-neon-yellow'
  return 'text-txt-dim'
})
</script>
