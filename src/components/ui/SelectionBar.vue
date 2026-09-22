<!--
  BARRE DE SÉLECTION MULTIPLE
  ---------------------------
  Apparaît en bas de l'écran dès qu'un élément est sélectionné par appui long.
  Elle indique combien d'éléments sont choisis et propose les actions groupées.
  Un seul composant pour tous les écrans : on lui passe l'objet renvoyé par
  useMultiSelect().
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
      class="fixed left-0 right-0 bottom-0 z-40 px-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] pt-3 bg-bg/95 backdrop-blur-xl border-t border-neon-violet/25"
    >
      <div class="mx-auto w-full max-w-3xl flex items-center gap-2 min-w-0">
        <button
          @click="sel.quitter()"
          class="p-2 rounded-xl hover:bg-bg-hover text-txt-dim shrink-0"
          title="Annuler la sélection"
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
          class="px-2.5 py-2 rounded-xl text-xs font-medium bg-bg-surface text-txt-dim hover:text-txt shrink-0"
          :title="sel.toutSelectionne ? 'Tout désélectionner' : 'Tout sélectionner'"
        >
          <CheckCheck class="w-4 h-4" />
        </button>

        <button
          v-if="sel.peutRestaurer && sel.auMoinsUnArchive"
          :disabled="sel.enCours"
          @click="sel.restaurerSelection()"
          class="p-2 rounded-xl text-neon-green hover:bg-neon-green/10 disabled:opacity-50 shrink-0"
          title="Restaurer la sélection"
        >
          <RotateCcw class="w-5 h-5" />
        </button>

        <button
          v-if="sel.peutArchiver && !sel.toutArchive"
          :disabled="sel.enCours"
          @click="sel.archiverSelection()"
          class="p-2 rounded-xl text-neon-yellow hover:bg-neon-yellow/10 disabled:opacity-50 shrink-0"
          title="Archiver la sélection"
        >
          <Archive class="w-5 h-5" />
        </button>

        <button
          v-if="sel.peutSupprimer"
          :disabled="sel.enCours"
          @click="sel.supprimerSelection()"
          class="p-2 rounded-xl text-neon-red hover:bg-neon-red/10 disabled:opacity-50 shrink-0"
          title="Supprimer définitivement"
        >
          <Loader2 v-if="sel.enCours" class="w-5 h-5 animate-spin" />
          <Trash2 v-else class="w-5 h-5" />
        </button>
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
