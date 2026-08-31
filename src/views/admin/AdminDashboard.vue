<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement du tableau de bord..." />
    </div>
    <template v-else>
      <div class="grid grid-cols-2 lg:grid-cols-4 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="stat-card">
          <span class="stat-value text-neon-green">{{ formatCurrency(stats.revenus_aujourd_hui || 0) }}</span>
          <span class="stat-label">Revenus aujourd'hui</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-blue">{{ stats.sessions_aujourd_hui || 0 }}</span>
          <span class="stat-label">Sessions aujourd'hui</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-violet">{{ stats.joueurs_actifs || 0 }}</span>
          <span class="stat-label">Joueurs actifs</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-yellow">{{ stats.jetons_attribues || 0 }}</span>
          <span class="stat-label">Jetons attribués</span>
        </div>
      </div>

      <div class="card w-full max-w-full min-w-0 overflow-hidden">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2 w-full max-w-full min-w-0">
          <BarChart3 class="w-4 h-4 text-neon-violet shrink-0" />
          <span class="truncate">Activité 7 jours</span>
        </h4>
        <div class="relative" style="height: 200px">
          <Bar v-if="chartData" :data="chartData" :options="chartOptions" />
          <p v-else class="text-txt-dim text-sm text-center py-6">Aucune donnée</p>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="card w-full max-w-full min-w-0 overflow-hidden">
          <h4 class="font-gaming font-bold mb-4 flex items-center gap-2 w-full max-w-full min-w-0">
            <BarChart3 class="w-4 h-4 text-neon-violet shrink-0" />
            <span class="truncate">Sessions en cours</span>
          </h4>
          <div class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
            <div v-for="s in activeSessions" :key="s.id" class="flex items-center gap-2 sm:gap-3 p-2 rounded-lg bg-bg-surface text-sm w-full max-w-full min-w-0 overflow-hidden flex-wrap">
              <div class="w-2 h-2 rounded-full bg-neon-green animate-pulse shrink-0"></div>
              <span class="flex-1 min-w-0 truncate">{{ s.console_nom }}</span>
              <span class="text-txt-dim truncate min-w-0 flex-1">{{ s.joueur_nom }}</span>
              <span class="font-mono text-neon-blue shrink-0 truncate">{{ s.jeu_nom }}</span>
            </div>
            <p v-if="activeSessions.length === 0" class="text-sm text-txt-dim text-center py-3">Aucune session active</p>
          </div>
        </div>

        <div class="card w-full max-w-full min-w-0 overflow-hidden">
          <h4 class="font-gaming font-bold mb-4 flex items-center gap-2 w-full max-w-full min-w-0">
            <Users class="w-4 h-4 text-neon-blue shrink-0" />
            <span class="truncate">Derniers joueurs</span>
          </h4>
          <div class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
            <div v-for="j in joueurs" :key="j.id" class="flex items-center gap-2 sm:gap-3 p-2 rounded-lg bg-bg-surface text-sm w-full max-w-full min-w-0 overflow-hidden flex-wrap">
              <div class="w-8 h-8 rounded-full bg-neon-violet/20 flex items-center justify-center text-neon-violet text-xs font-bold shrink-0">
                {{ j.nom?.charAt(0) }}
              </div>
              <span class="flex-1 min-w-0 truncate">{{ j.nom }}</span>
              <span class="text-txt-dim shrink-0 truncate">{{ j.jetons_solde || 0 }} jetons</span>
            </div>
          </div>
        </div>
      </div>

      <div class="card w-full max-w-full min-w-0 overflow-hidden">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2 w-full max-w-full min-w-0">
          <Trophy class="w-4 h-4 text-neon-yellow shrink-0" />
          <span class="truncate">Top jeux</span>
        </h4>
        <div class="space-y-2 w-full max-w-full min-w-0 overflow-hidden">
          <div v-for="(j, i) in stats.top_jeux || []" :key="i" class="flex items-center gap-2 sm:gap-3 text-sm w-full max-w-full min-w-0 overflow-hidden flex-wrap">
            <span class="font-gaming font-bold text-txt-dim w-6 shrink-0">{{ i + 1 }}.</span>
            <span class="flex-1 min-w-0 truncate">{{ j.titre }}</span>
            <span class="text-txt-dim shrink-0 truncate">{{ j.sessions }} sessions</span>
            <span class="font-mono text-neon-blue w-12 text-right shrink-0 truncate">{{ j.pct }}%</span>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { api } from '@/utils/api'
import { formatCurrency } from '@/utils/helpers'
import { BarChart3, Users, Trophy } from 'lucide-vue-next'
import Loader from '@/components/ui/Loader.vue'
import { Bar } from 'vue-chartjs'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  BarElement,
  Title,
  Tooltip,
  Legend
} from 'chart.js'

ChartJS.register(CategoryScale, LinearScale, BarElement, Title, Tooltip, Legend)

const stats = ref<any>({})
const activeSessions = ref<any[]>([])
const joueurs = ref<any[]>([])
const loading = ref(true)

const gridColor = 'rgba(255,255,255,0.06)'
const tickColor = 'rgba(226,232,240,0.5)'

const chartData = computed(() => {
  const history = stats.value.sessions_history
  if (!history?.length) return null
  return {
    labels: history.map((d: any) => d.date),
    datasets: [{
      label: 'Sessions',
      data: history.map((d: any) => d.count),
      backgroundColor: history.map((_: any, i: number) => i === history.length - 1 ? 'rgba(168, 85, 247, 0.8)' : 'rgba(168, 85, 247, 0.35)'),
      borderColor: 'rgba(168, 85, 247, 1)',
      borderWidth: 1,
      borderRadius: 6,
      maxBarThickness: 40
    }]
  }
})

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: { display: false },
    tooltip: {
      backgroundColor: 'rgba(18, 18, 26, 0.95)',
      titleColor: '#e2e8f0',
      bodyColor: '#a855f7',
      borderColor: 'rgba(168, 85, 247, 0.3)',
      borderWidth: 1,
      cornerRadius: 8,
      padding: 12
    }
  },
  scales: {
    x: { grid: { color: gridColor }, ticks: { color: tickColor, font: { size: 10 } } },
    y: { grid: { color: gridColor }, ticks: { color: tickColor, font: { size: 10 } }, beginAtZero: true }
  }
}

onMounted(async () => {
  loading.value = true
  try { stats.value = await api.get('/rapports/ca') }
  catch (e: any) { toast.error('Stats: ' + (e.message || 'erreur')) }
  try { activeSessions.value = (await api.get('/sessions?statut=en_cours')).slice(0, 5) }
  catch (e: any) { toast.error('Sessions: ' + (e.message || 'erreur')) }
  try { joueurs.value = (await api.get('/joueurs')).slice(0, 5) }
  catch (e: any) { toast.error('Joueurs: ' + (e.message || 'erreur')) }
  finally { loading.value = false }
})
</script>
