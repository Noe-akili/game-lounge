// @ts-nocheck
// ============================================================
// SÉLECTION MULTIPLE PAR APPUI LONG (comme la galerie photo)
// ------------------------------------------------------------
// Principe, en mots simples :
//   1. J'appuie longuement (≈ 0,45 s) sur un élément  -> il est sélectionné
//      et le mode sélection s'allume (petite vibration si le téléphone sait).
//   2. Ensuite, un simple appui sur les autres éléments les ajoute ou les
//      retire de la sélection.
//   3. La barre en bas de l'écran indique combien d'éléments sont choisis et
//      propose : Archiver, Restaurer, Supprimer définitivement, Tout, Annuler.
//   4. Quand la sélection est vide, le mode s'éteint tout seul : les appuis
//      simples reprennent leur rôle normal (ouvrir, modifier…).
//
// Ce fichier ne connaît AUCUN écran en particulier : chaque écran lui donne
// ses actions (archiver / restaurer / supprimer) et il s'occupe du reste.
// ============================================================

import { computed, onUnmounted, reactive, ref } from 'vue'
import { toast } from 'vue-sonner'

/** Durée d'appui à partir de laquelle on considère un « appui long ». */
const DUREE_APPUI_LONG = 450
/** Tolérance de déplacement du doigt : au-delà, c'est un défilement, pas un appui. */
const TOLERANCE_PX = 12

export interface OptionsSelection {
  /** Ex. 'facture' — utilisé dans les messages. */
  nomSingulier: string
  /** Ex. 'factures'. */
  nomPluriel: string
  /** Liste actuellement affichée (pour « Tout sélectionner »). */
  liste: () => any[]
  /** L'élément est-il déjà archivé ? (corbeille) */
  estArchive?: (item: any) => boolean
  /** Archivage (suppression douce). */
  archiver?: (item: any) => Promise<any>
  /** Suppression définitive. */
  supprimer?: (item: any) => Promise<any>
  /** Restauration d'un élément archivé. */
  restaurer?: (item: any) => Promise<any>
  /** Rechargement de la liste après une action groupée. */
  apres?: () => any
}

export function useMultiSelect(opts: OptionsSelection) {
  const actif = ref(false)
  const ids = ref<Set<any>>(new Set())
  const enCours = ref(false)

  let minuteur: any = null
  let appuiLongFait = false
  let depart: { x: number; y: number } | null = null

  const count = computed(() => ids.value.size)

  /** Éléments sélectionnés, retrouvés dans la liste affichée. */
  const selection = computed(() => opts.liste().filter((it: any) => ids.value.has(it.id)))

  const toutArchive = computed(() =>
    !!opts.estArchive && count.value > 0 && selection.value.every((it: any) => opts.estArchive!(it))
  )
  const auMoinsUnArchive = computed(() =>
    !!opts.estArchive && selection.value.some((it: any) => opts.estArchive!(it))
  )
  const toutSelectionne = computed(() => {
    const l = opts.liste()
    return l.length > 0 && l.every((it: any) => ids.value.has(it.id))
  })

  function vibrer(ms = 20) {
    try { navigator?.vibrate?.(ms) } catch {}
  }

  function estSelectionne(id: any) {
    return ids.value.has(id)
  }

  function toggle(item: any) {
    const id = item?.id ?? item
    const s = new Set(ids.value)
    if (s.has(id)) s.delete(id)
    else s.add(id)
    ids.value = s
    // Plus rien de sélectionné -> on sort du mode sélection.
    if (s.size === 0) actif.value = false
  }

  function demarrer(item: any) {
    actif.value = true
    const s = new Set(ids.value)
    s.add(item?.id ?? item)
    ids.value = s
    vibrer(25)
  }

  function quitter() {
    actif.value = false
    ids.value = new Set()
  }

  function toutSelectionner() {
    if (toutSelectionne.value) {
      quitter()
      return
    }
    actif.value = true
    ids.value = new Set(opts.liste().map((it: any) => it.id))
  }

  function annulerMinuteur() {
    if (minuteur) { clearTimeout(minuteur); minuteur = null }
    depart = null
  }

  /**
   * À poser sur la carte / ligne de la liste :  v-on="sel.press(item, () => ouvrir(item))"
   * `onTap` est l'action normale d'un appui simple (ouvrir le détail, etc.).
   */
  function press(item: any, onTap?: () => void) {
    return {
      pointerdown(e: PointerEvent) {
        // Clic droit / stylet : on ignore, le menu contextuel gère.
        if (e.button && e.button !== 0) return
        appuiLongFait = false
        depart = { x: e.clientX, y: e.clientY }
        annulerMinuteurSeul()
        minuteur = setTimeout(() => {
          minuteur = null
          appuiLongFait = true
          demarrer(item)
        }, DUREE_APPUI_LONG)
      },
      pointermove(e: PointerEvent) {
        // Le doigt glisse (défilement) -> ce n'est plus un appui long.
        if (!depart || !minuteur) return
        if (Math.abs(e.clientX - depart.x) > TOLERANCE_PX || Math.abs(e.clientY - depart.y) > TOLERANCE_PX) {
          annulerMinuteur()
        }
      },
      pointerup: annulerMinuteur,
      pointercancel() { annulerMinuteur(); appuiLongFait = false },
      pointerleave: annulerMinuteur,
      contextmenu(e: Event) {
        // Empêche le menu « copier / partager » du navigateur pendant l'appui long.
        if (actif.value || appuiLongFait) e.preventDefault()
      },
      click(e: MouseEvent) {
        // Le clic qui suit immédiatement un appui long ne doit rien déclencher.
        if (appuiLongFait) {
          appuiLongFait = false
          e.preventDefault()
          e.stopPropagation()
          return
        }
        if (actif.value) {
          e.preventDefault()
          e.stopPropagation()
          toggle(item)
          return
        }
        onTap?.()
      },
    }
  }

  function annulerMinuteurSeul() {
    if (minuteur) { clearTimeout(minuteur); minuteur = null }
  }

  /** Exécute une action sur chaque élément sélectionné, un par un. */
  async function executer(action: (item: any) => Promise<any>, verbe: string) {
    const cibles = selection.value.slice()
    if (!cibles.length) return
    enCours.value = true
    let ok = 0
    const echecs: string[] = []
    for (const item of cibles) {
      try { await action(item); ok++ }
      catch (e: any) { echecs.push(e?.message || `#${item.id}`) }
    }
    enCours.value = false
    const mot = ok > 1 ? opts.nomPluriel : opts.nomSingulier
    if (ok) toast.success(`${ok} ${mot} ${verbe}`)
    if (echecs.length) toast.error(`${echecs.length} échec(s) : ${echecs[0]}`)
    quitter()
    await opts.apres?.()
  }

  async function archiverSelection() {
    if (!opts.archiver) return
    if (!confirm(`Archiver ${count.value} ${count.value > 1 ? opts.nomPluriel : opts.nomSingulier} ?`)) return
    await executer(opts.archiver, count.value > 1 ? 'archivés' : 'archivé')
  }

  async function supprimerSelection() {
    if (!opts.supprimer) return
    if (!confirm(`ATTENTION : supprimer DÉFINITIVEMENT ${count.value} ${count.value > 1 ? opts.nomPluriel : opts.nomSingulier} ? Cette action est irréversible.`)) return
    await executer(opts.supprimer, count.value > 1 ? 'supprimés' : 'supprimé')
  }

  async function restaurerSelection() {
    if (!opts.restaurer) return
    await executer(opts.restaurer, count.value > 1 ? 'restaurés' : 'restauré')
  }

  onUnmounted(annulerMinuteur)

  // `reactive` : les refs sont automatiquement déballées, donc dans les
  // templates on écrit simplement `sel.actif`, `sel.count`… sans `.value`.
  return reactive({
    actif, ids, count, enCours, selection,
    toutArchive, auMoinsUnArchive, toutSelectionne,
    estSelectionne, toggle, press, quitter, toutSelectionner,
    archiverSelection, supprimerSelection, restaurerSelection,
    // Métadonnées utilisées par la barre de sélection.
    nomSingulier: opts.nomSingulier,
    nomPluriel: opts.nomPluriel,
    peutArchiver: !!opts.archiver,
    peutSupprimer: !!opts.supprimer,
    peutRestaurer: !!opts.restaurer,
  })
}
