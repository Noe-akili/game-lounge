<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div v-if="loading" class="card">
      <Loader variant="neon" size="lg" text="Chargement..." />
    </div>
    <AdminParametres v-else />
  </div>
</template>

<script setup lang="ts">
// L'application est mono-utilisateur admin : l'ancienne vue "Paramètres" employé
// (préférences + déconnexion) n'a plus de raison d'être. On redirige vers les
// paramètres admin, l'interface étant désormais unique.
import { onMounted, ref } from 'vue'
import Loader from '@/components/ui/Loader.vue'
import AdminParametres from '@/views/admin/AdminParametres.vue'

const loading = ref(true)
onMounted(() => setTimeout(() => (loading.value = false), 150))
</script>
