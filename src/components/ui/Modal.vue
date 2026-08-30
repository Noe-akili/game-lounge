<template>
  <Transition name="modal">
    <div v-if="open" class="fixed inset-0 z-50 flex items-end sm:items-center justify-center p-0 sm:p-4">
      <motion.div
        :initial="{ opacity: 0 }"
        :animate="{ opacity: 1 }"
        :exit="{ opacity: 0 }"
        :transition="{ duration: 0.2 }"
        class="absolute inset-0 bg-black/70 backdrop-blur-sm"
        @click="$emit('close')"
      />
      <motion.div
        :initial="{ opacity: 0, y: 40, scale: 0.95 }"
        :animate="{ opacity: 1, y: 0, scale: 1 }"
        :exit="{ opacity: 0, y: 40, scale: 0.95 }"
        :transition="{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }"
        class="relative w-full bg-bg-card sm:rounded-3xl border-t sm:border border-white/10 shadow-2xl sm:max-h-[90vh] flex flex-col"
        :class="[sizeClass, 'max-h-[92vh] sm:max-h-[90vh]']"
      >
        <button @click="$emit('close')"
          class="absolute top-3 right-3 sm:top-4 sm:right-4 p-2 rounded-xl hover:bg-bg-hover text-txt-dim hover:text-txt transition-colors z-10">
          <X class="w-5 h-5" />
        </button>
        <div class="flex-1 overflow-y-auto">
          <slot />
        </div>
      </motion.div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { motion } from 'motion-v'
import { X } from 'lucide-vue-next'

const props = defineProps({
  open: { type: Boolean, default: false },
  size: { type: String, default: 'md' },
})
defineEmits(['close'])
const sizeClass = computed(() => {
  const map = { sm: 'sm:max-w-md', md: 'sm:max-w-lg', lg: 'sm:max-w-2xl', xl: 'sm:max-w-4xl' }
  return map[props.size] || 'sm:max-w-lg'
})
</script>

<style scoped>
.modal-enter-active, .modal-leave-active { transition: all 0.3s ease; }
.modal-enter-from, .modal-leave-to { opacity: 0; }
.modal-enter-from > div:last-child { transform: translateY(100%); }
.modal-leave-to > div:last-child { transform: translateY(100%); }
</style>
