<template>
  <div class="fixed inset-0 pointer-events-none z-0 overflow-hidden transition-all duration-500" aria-hidden="true">
    <!-- 1. Mode Étoiles Animées -->
    <template v-if="settings.bgMode === 'stars'">
      <!-- Voile de contraste sous les étoiles -->
      <div
        class="absolute inset-0 transition-colors duration-500"
        :class="settings.themeMode === 'light'
          ? 'bg-gradient-to-b from-sky-50/60 via-indigo-50/40 to-purple-50/60'
          : 'bg-gradient-to-b from-[#06060c] via-[#090915] to-[#0f0c22]'"
      ></div>
      <!-- Toile d'étoiles lumineuses bien visible au-dessus du voile -->
      <canvas ref="canvasRef" class="absolute inset-0 w-full h-full z-10 pointer-events-none"></canvas>
    </template>

    <!-- 2. Mode Photo Personnalisée -->
    <template v-else-if="settings.bgMode === 'custom' && settings.bgCustomImage">
      <div
        class="absolute inset-0 bg-cover bg-center bg-no-repeat transition-all duration-700 scale-105"
        :style="{ backgroundImage: `url(${settings.bgCustomImage})` }"
      ></div>
      <!-- Filtre adaptatif de cohérence thématique (teinte, contraste et lisibilité UI) -->
      <div
        class="absolute inset-0 transition-all duration-500"
        :class="settings.themeMode === 'light'
          ? 'bg-[#f8fafc]/88 backdrop-blur-[3px] mix-blend-soft-light'
          : 'bg-[#0a0a12]/85 backdrop-blur-[3px]'"
      ></div>
      <!-- Accentuation gaming douce aux coins -->
      <div
        class="absolute inset-0 pointer-events-none"
        :class="settings.themeMode === 'light'
          ? 'bg-radial-gradient from-transparent via-white/40 to-indigo-100/60'
          : 'bg-radial-gradient from-transparent via-[#0e0e18]/60 to-[#07070d]/90'"
      ></div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useSettingsStore } from '@/stores/settings'

const settings = useSettingsStore()
const canvasRef = ref<HTMLCanvasElement | null>(null)
let animationFrameId: number | null = null

interface Star {
  x: number
  y: number
  size: number
  opacity: number
  speed: number
  twinkleSpeed: number
  color: string
}

function initStars() {
  const canvas = canvasRef.value
  if (!canvas) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  let width = (canvas.width = window.innerWidth)
  let height = (canvas.height = window.innerHeight)

  const handleResize = () => {
    if (!canvas) return
    width = canvas.width = window.innerWidth
    height = canvas.height = window.innerHeight
  }
  window.addEventListener('resize', handleResize)

  const starCount = Math.min(120, Math.floor((width * height) / 9000))
  const isLight = settings.themeMode === 'light'
  const colors = isLight
    ? ['#6366f1', '#8b5cf6', '#3b82f6', '#a855f7']
    : ['#ffffff', '#a855f7', '#00f0ff', '#f43f5e', '#e0e7ff']

  const stars: Star[] = []
  for (let i = 0; i < starCount; i++) {
    stars.push({
      x: Math.random() * width,
      y: Math.random() * height,
      size: Math.random() * 1.8 + 0.6,
      opacity: Math.random() * 0.8 + 0.2,
      speed: Math.random() * 0.15 + 0.05,
      twinkleSpeed: (Math.random() * 0.02 + 0.008) * (Math.random() > 0.5 ? 1 : -1),
      color: colors[Math.floor(Math.random() * colors.length)],
    })
  }

  function render() {
    if (!ctx) return
    ctx.clearRect(0, 0, width, height)

    for (const star of stars) {
      star.y -= star.speed
      if (star.y < 0) {
        star.y = height
        star.x = Math.random() * width
      }

      star.opacity += star.twinkleSpeed
      if (star.opacity > 0.95 || star.opacity < 0.2) {
        star.twinkleSpeed = -star.twinkleSpeed
      }

      ctx.beginPath()
      ctx.arc(star.x, star.y, star.size, 0, Math.PI * 2)
      ctx.fillStyle = star.color
      ctx.globalAlpha = Math.max(0.1, Math.min(1, star.opacity))
      ctx.shadowBlur = star.size * 2
      ctx.shadowColor = star.color
      ctx.fill()
    }

    ctx.globalAlpha = 1
    ctx.shadowBlur = 0
    animationFrameId = requestAnimationFrame(render)
  }

  render()

  return () => {
    window.removeEventListener('resize', handleResize)
    if (animationFrameId !== null) {
      cancelAnimationFrame(animationFrameId)
    }
  }
}

let cleanup: (() => void) | undefined

watch(
  () => [settings.bgMode, settings.themeMode],
  ([mode]) => {
    if (cleanup) {
      cleanup()
      cleanup = undefined
    }
    if (mode === 'stars') {
      nextTick(() => {
        setTimeout(() => {
          cleanup = initStars()
        }, 50)
      })
    }
  },
  { immediate: true }
)

onUnmounted(() => {
  if (cleanup) cleanup()
})
</script>
