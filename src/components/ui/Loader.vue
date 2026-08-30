<template>
  <div class="flex flex-col items-center justify-center gap-3" :class="containerClass">
    <motion.div
      v-if="variant === 'spinner'"
      :animate="{ rotate: 360 }"
      :transition="{ duration: 1, repeat: Infinity, ease: 'linear' }"
      class="rounded-full border-2 border-white/10 border-t-neon-violet"
      :class="sizeClass"
    />
    <div v-else-if="variant === 'dots'" class="flex gap-2">
      <motion.div
        v-for="i in 3"
        :key="i"
        :animate="{ y: [0, -8, 0], opacity: [1, 0.6, 1] }"
        :transition="{ duration: 0.6, repeat: Infinity, delay: i * 0.15, ease: 'easeInOut' }"
        class="w-2.5 h-2.5 rounded-full bg-neon-violet"
      />
    </div>
    <div v-else-if="variant === 'skeleton'" class="w-full space-y-3">
      <div v-for="i in lines" :key="i" class="h-4 bg-bg-surface rounded-lg animate-pulse" :style="{ width: `${70 + Math.random() * 30}%` }"></div>
    </div>
    <motion.div
      v-if="variant === 'neon'"
      :animate="{ rotate: 360, scale: [1, 1.05, 1] }"
      :transition="{ rotate: { duration: 1.2, repeat: Infinity, ease: 'linear' }, scale: { duration: 1.2, repeat: Infinity, ease: 'easeInOut' } }"
      class="relative flex items-center justify-center"
      :class="sizeClass"
    >
      <div class="absolute inset-0 rounded-full border-2 border-neon-violet/20"></div>
      <div class="absolute inset-1 rounded-full border-2 border-t-neon-violet border-r-transparent border-b-transparent border-l-transparent"></div>
      <Gamepad2 class="w-1/2 h-1/2 text-neon-violet" />
    </motion.div>
    <p v-if="text" class="text-sm text-txt-dim font-medium">{{ text }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { motion } from 'motion-v'
import { Gamepad2 } from 'lucide-vue-next'

const props = withDefaults(defineProps<{
  size?: 'sm' | 'md' | 'lg' | 'xl'
  variant?: 'spinner' | 'dots' | 'skeleton' | 'neon'
  text?: string
  lines?: number
}>(), {
  size: 'md',
  variant: 'spinner',
  text: '',
  lines: 3,
})

const sizeClass = computed(() => {
  const map = { sm: 'w-6 h-6', md: 'w-10 h-10', lg: 'w-14 h-14', xl: 'w-20 h-20' }
  return map[props.size]
})

const containerClass = computed(() => {
  if (props.variant === 'skeleton') return 'w-full'
  return 'py-8'
})
</script>
