<!--
  BARRE DE SÉLECTION MULTIPLE
  ---------------------------
  Apparaît en bas de l'écran dès qu'un élément est sélectionné par appui long.
  Elle indique combien d'éléments sont choisis et propose les actions groupées
  avec des BOUTONS ÉCRITS (Supprimer / Archiver / Restaurer), pas seulement des
  icônes : sur téléphone, on doit comprendre sans deviner.

  Important : la barre passe AU-DESSUS du menu du bas (z-50 contre z-40).
  Avant, les deux étaient au même niveau et le menu recouvrait les boutons
  d'action : la sélection marchait mais on ne pouvait rien supprimer.
-->
<template>
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-4"
    leave-active-class="transition duration-150 ease-in"
    leave-to-class="opacity-0 translate-y-4"
  >
    <div
      v-if="sel.actif && sel.count > 0"
      class="fixed left-0 right-0 bottom-0 z-50 px-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] pt-3 bg-bg-card/98 backdrop-blur-xl border-t border-neon-violet/30 shadow-[0_-8px_24px_rgba(0,0,0,0.45)]"
    >
      <div class="mx-auto w-full max-w-3xl space-y-2.5">
        <!-- Ligne 1 : ce qui est sélectionné + annuler / tout sélectionner -->
        <div class="flex items-center gap-2 min-w-0">
          <button
            @click="sel.quitter()"
            class="p-2 rounded-xl bg-bg-surface hover:bg-bg-hover text-txt-dim shrink-0"
            aria-label="Annuler la sélection"
          >
            <X class="w-5 h-5" />
          </button>

          <div class="min-w-0 flex-1">
            <p class="font-gaming font-bold text-sm truncate">
              {{ sel.count }} {{ sel.count > 1 ? sel.nomPluriel : sel.nomSingulier }} sélectionné{{ sel.count > 1 ? 's' : '' }}
            </p>
            <p class="text-[11px] text-txt-dim truncate">
              Appuyez sur d'autres éléments pour les ajouter
            </p>
          </div>

          <button
            @click="sel.toutSelectionner()"
            class="px-3 py-2 rounded-xl text-xs font-semibold bg-bg-surface text-txt-dim hover:text-txt shrink-0 flex items-center gap-1.5"
          >
            <CheckCheck class="w-4 h-4" />
            <span>{{ sel.toutSelectionne ? 'Rien' : 'Tout' }}</span>
          </button>
        </div>

        <!-- Ligne 2 : les actions, écrites en clair -->
        <div class="flex items-center gap-2">
          <button
            v-if="sel.peutRestaurer && sel.auMoinsUnArchive"
            :disabled="sel.enCours"
            @click="sel.restaurerSelection()"
            class="flex-1 min-w-0 flex items-center justify-center gap-2 px-3 py-2.5 rounded-xl font-semibold text-sm bg-neon-green/15 text-neon-green border border-neon-green/30 active:scale-[0.98] disabled:opacity-50"
          >
            <RotateCcw class="w-4 h-4 shrink-0" />
            <span class="truncate">Restaurer</span>
          </button>

          <button
            v-if="sel.peutArchiver && !sel.toutArchive"
            :disabled="sel.enCours"
            @click="sel.archiverSelection()"
            class="flex-1 min-w-0 flex items-center justify-center gap-2 px-3 py-2.5 rounded-xl font-semibold text-sm bg-neon-yellow/15 text-neon-yellow border border-neon-yellow/30 active:scale-[0.98] disabled:opacity-50"
          >
            <Archive class="w-4 h-4 shrink-0" />
            <span class="truncate">Archiver</span>
          </button>

          <button
            v-if="sel.peutSupprimer"
            :disabled="sel.enCours"
            @click="sel.supprimerSelection()"
            class="flex-1 min-w-0 flex items-center justify-center gap-2 px-3 py-2.5 rounded-xl font-semibold text-sm bg-neon-red/15 text-neon-red border border-neon-red/30 active:scale-[0.98] disabled:opacity-50"
          >
            <Loader2 v-if="sel.enCours" class="w-4 h-4 shrink-0 animate-spin" />
            <Trash2 v-else class="w-4 h-4 shrink-0" />
            <span class="truncate">Supprimer</span>
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
// @ts-nocheck
import { X, Trash2, Archive, RotateCcw, CheckCheck, Loader2 } from 'lucide-vue-next'

const props = defineProps<{ sel: any }>()
const sel = props.sel
</script>
