<template>
  <span :class="cls">{{ display }}</span>
</template>

<script setup lang="ts">
// Chronomètre de session EN TEMPS RÉEL (s'écoule à la seconde, pas gelé entre
// deux rafraîchissements). Le temps restant est calculé sur duree_allouee
// (temps alloué par le tarif) : quand il atteint zéro, le backend termine la
// session automatiquement et une notification Android est envoyée.
// @ts-nocheck
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { formatDuration } from '@/utils/helpers'

const props = defineProps({
  sessionDebut: { type: String, default: '' },
  dureeAllouee: { type: Number, default: 0 },
  accumulee: { type: Number, default: 0 },
  statut: { type: String, default: 'en_cours' },
  // 'elapsed' = temps écoulé (+dépassé en rouge) ; 'remaining' = compte à rebours.
  mode: { type: String, default: 'elapsed' },
})

const now = ref(Date.now())
let timer = null

onMounted(() => {
  timer = setInterval(() => { now.value = Date.now() }, 1000)
})
onUnmounted(() => { if (timer) clearInterval(timer) })

const elapsedSeconds = computed(() => {
  if (props.statut === 'pause') return (props.accumulee || 0) * 60
  if (!props.sessionDebut) return (props.accumulee || 0) * 60
  const start = new Date(props.sessionDebut).getTime()
  if (Number.isNaN(start)) return (props.accumulee || 0) * 60
  return (props.accumulee || 0) * 60 + Math.max(0, Math.floor((now.value - start) / 1000))
})

const restant = computed(() =>
  props.dureeAllouee > 0 ? props.dureeAllouee * 60 - elapsedSeconds.value : 0
)
const depasse = computed(() => props.dureeAllouee > 0 && restant.value < 0)

const display = computed(() => {
  if (props.mode === 'remaining') {
    if (props.dureeAllouee <= 0) return '—'
    return formatDuration(Math.max(0, restant.value))
  }
  if (props.dureeAllouee > 0 && depasse.value) return `+${formatDuration(-restant.value)}`
  return formatDuration(elapsedSeconds.value)
})

const cls = computed(() => {
  const base = 'font-gaming font-bold tabular-nums'
  if (props.mode === 'remaining') {
    if (depasse.value) return `${base} text-neon-red`
    if (props.dureeAllouee > 0 && restant.value < 5 * 60) return `${base} text-neon-yellow`
    return `${base} text-txt-dim`
  }
  if (depasse.value) return `${base} text-neon-red animate-pulse`
  if (props.statut === 'pause') return `${base} text-neon-yellow`
  if (props.dureeAllouee > 0 && restant.value < 5 * 60) return `${base} text-neon-yellow`
  return `${base} text-neon-blue`
})

defineExpose({ elapsedSeconds, restant, depasse })
</script>
