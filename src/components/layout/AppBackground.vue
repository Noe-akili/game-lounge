<template>
  <div class="fixed inset-0 pointer-events-none z-0 overflow-hidden transition-all duration-500" aria-hidden="true">
    <!-- 1. Mode Étoiles Animées Enrichi -->
    <template v-if="settings.bgMode === 'stars'">
      <!-- Fond spatial profond -->
      <div
        class="absolute inset-0 transition-colors duration-700"
        :class="settings.themeMode === 'light'
          ? 'bg-gradient-to-b from-sky-100/60 via-indigo-50/40 to-purple-100/60'
          : 'bg-gradient-to-b from-[#04040a] via-[#070614] to-[#0c091d]'"
      ></div>
      <!-- Nébuleuses galactiques lumineuses -->
      <div
        class="absolute -top-32 -left-32 w-[28rem] h-[28rem] rounded-full blur-[100px] pointer-events-none transition-opacity duration-700 opacity-40 animate-pulse-neon"
        :class="settings.themeMode === 'light' ? 'bg-indigo-300/30' : 'bg-purple-600/25'"
      ></div>
      <div
        class="absolute top-1/2 -right-32 w-[32rem] h-[32rem] rounded-full blur-[120px] pointer-events-none transition-opacity duration-700 opacity-35"
        :class="settings.themeMode === 'light' ? 'bg-cyan-300/30' : 'bg-cyan-500/20'"
      ></div>
      <!-- Toile d'étoiles multicouches, scintillantes et étoiles filantes -->
      <canvas ref="canvasRef" class="absolute inset-0 w-full h-full z-10 pointer-events-none"></canvas>
    </template>

    <!-- 2. Mode Photo Personnalisée -->
    <template v-else-if="settings.bgMode === 'custom' && settings.bgCustomImage">
      <div
        class="absolute inset-0 bg-cover bg-center bg-no-repeat transition-all duration-700 scale-105"
        :style="{ backgroundImage: `url(${settings.bgCustomImage})` }"
      ></div>
      <!-- Filtre adaptatif de cohérence thématique -->
      <div
        class="absolute inset-0 transition-all duration-500"
        :class="settings.themeMode === 'light'
          ? 'bg-[#f8fafc]/75 backdrop-blur-[2px] mix-blend-soft-light'
          : 'bg-[#0a0a12]/75 backdrop-blur-[2px]'"
      ></div>
      <!-- Accentuation gaming douce aux coins -->
      <div
        class="absolute inset-0 pointer-events-none"
        :class="settings.themeMode === 'light'
          ? 'bg-radial-gradient from-transparent via-white/30 to-indigo-100/50'
          : 'bg-radial-gradient from-transparent via-[#0e0e18]/40 to-[#07070d]/80'"
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
  baseOpacity: number
  opacity: number
  speed: number
  twinkleSpeed: number
  color: string
  isCross: boolean
}

interface Meteor {
  x: number
  y: number
  len: number
  speed: number
  size: number
  angle: number
  opacity: number
  active: boolean
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

  const isLight = settings.themeMode === 'light'
  const starCount = Math.min(160, Math.floor((width * height) / 7500))

  const colors = isLight
    ? ['#6366f1', '#8b5cf6', '#3b82f6', '#a855f7', '#0284c7']
    : ['#ffffff', '#ffffff', '#e0e7ff', '#a855f7', '#00f0ff', '#f43f5e', '#38bdf8']

  const stars: Star[] = []
  for (let i = 0; i < starCount; i++) {
    const size = Math.random() < 0.15 ? Math.random() * 2 + 1.8 : Math.random() * 1.5 + 0.5
    stars.push({
      x: Math.random() * width,
      y: Math.random() * height,
      size,
      baseOpacity: Math.random() * 0.7 + 0.3,
      opacity: Math.random() * 0.8 + 0.2,
      speed: Math.random() * 0.25 + 0.05,
      twinkleSpeed: (Math.random() * 0.025 + 0.008) * (Math.random() > 0.5 ? 1 : -1),
      color: colors[Math.floor(Math.random() * colors.length)],
      isCross: size > 2.5 && Math.random() > 0.4,
    })
  }

  // Étoiles filantes (météores)
  const meteors: Meteor[] = []
  function spawnMeteor() {
    meteors.push({
      x: Math.random() * width * 1.2 - width * 0.2,
      y: Math.random() * height * 0.4,
      len: Math.random() * 80 + 50,
      speed: Math.random() * 8 + 7,
      size: Math.random() * 1.5 + 1,
      angle: Math.PI / 4 + (Math.random() * 0.2 - 0.1),
      opacity: 1,
      active: true,
    })
  }

  let meteorTimer = 0

  function render() {
    if (!ctx) return
    ctx.clearRect(0, 0, width, height)

    // Étoiles
    for (const star of stars) {
      star.y -= star.speed
      if (star.y < 0) {
        star.y = height
        star.x = Math.random() * width
      }

      star.opacity += star.twinkleSpeed
      if (star.opacity > 1 || star.opacity < 0.2) {
        star.twinkleSpeed = -star.twinkleSpeed
      }

      ctx.save()
      ctx.beginPath()
      ctx.arc(star.x, star.y, star.size, 0, Math.PI * 2)
      ctx.fillStyle = star.color
      ctx.globalAlpha = Math.max(0.1, Math.min(1, star.opacity))
      ctx.shadowBlur = star.size * 3
      ctx.shadowColor = star.color
      ctx.fill()

      // Scintillement en croix pour les plus grandes étoiles
      if (star.isCross && star.opacity > 0.7) {
        ctx.strokeStyle = star.color
        ctx.lineWidth = 0.8
        const crossLen = star.size * 2.5
        ctx.beginPath()
        ctx.moveTo(star.x - crossLen, star.y)
        ctx.lineTo(star.x + crossLen, star.y)
        ctx.moveTo(star.x, star.y - crossLen)
        ctx.lineTo(star.x, star.y + crossLen)
        ctx.stroke()
      }
      ctx.restore()
    }

    // Gestion des météores
    meteorTimer++
    if (meteorTimer > 180 && Math.random() < 0.03 && meteors.length < 3) {
      spawnMeteor()
      meteorTimer = 0
    }

    for (let i = meteors.length - 1; i >= 0; i--) {
      const m = meteors[i]
      if (!m.active) {
        meteors.splice(i, 1)
        continue
      }
      const dx = Math.cos(m.angle) * m.speed
      const dy = Math.sin(m.angle) * m.speed
      m.x += dx
      m.y += dy
      m.opacity -= 0.015

      if (m.opacity <= 0 || m.x > width + 100 || m.y > height + 100) {
        m.active = false
        meteors.splice(i, 1)
        continue
      }

      ctx.save()
      const grad = ctx.createLinearGradient(
        m.x,
        m.y,
        m.x - Math.cos(m.angle) * m.len,
        m.y - Math.sin(m.angle) * m.len
      )
      const meteorColor = isLight ? '#4f46e5' : '#00f0ff'
      grad.addColorStop(0, meteorColor)
      grad.addColorStop(1, 'transparent')

      ctx.strokeStyle = grad
      ctx.lineWidth = m.size
      ctx.globalAlpha = Math.max(0, m.opacity)
      ctx.beginPath()
      ctx.moveTo(m.x, m.y)
      ctx.lineTo(m.x - Math.cos(m.angle) * m.len, m.y - Math.sin(m.angle) * m.len)
      ctx.stroke()
      ctx.restore()
    }

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
    if (mode && mode !== 'default') {
      document.documentElement.classList.add('has-custom-bg')
    } else {
      document.documentElement.classList.remove('has-custom-bg')
    }
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
  document.documentElement.classList.remove('has-custom-bg')
})
</script>
