<template>
  <div class="space-y-6 w-full max-w-full min-w-0 overflow-hidden">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 w-full max-w-full min-w-0">
      <h3 class="font-gaming text-lg font-bold">Rapports & Statistiques</h3>
      <div class="flex gap-2">
        <button @click="refresh" class="btn-neon-outline flex items-center gap-2 text-sm">
          <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': loading }" />
        </button>
        <button @click="exportCsv" class="btn-neon-violet flex items-center gap-2 text-sm">
          <Download class="w-4 h-4" /> Exporter CSV
        </button>
      </div>
    </div>

    <div v-if="loading" class="card w-full max-w-full min-w-0 overflow-hidden">
      <Loader variant="neon" size="lg" text="Chargement des rapports..." />
    </div>

    <template v-else>
      <div class="grid grid-cols-2 lg:grid-cols-4 gap-2 sm:gap-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="stat-card">
          <span class="stat-value text-neon-green">{{ formatCurrency(stats.total_revenus || 0) }}</span>
          <span class="stat-label">Revenus totaux</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-blue">{{ stats.total_sessions || 0 }}</span>
          <span class="stat-label">Sessions totales</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-violet">{{ stats.total_joueurs || 0 }}</span>
          <span class="stat-label">Joueurs</span>
        </div>
        <div class="stat-card">
          <span class="stat-value text-neon-yellow">{{ formatCurrency(stats.revenus_aujourd_hui || 0) }}</span>
          <span class="stat-label">Aujourd'hui</span>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="card w-full max-w-full min-w-0 overflow-hidden">
          <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
            <BarChart3 class="w-4 h-4 text-neon-violet" />
            <span>Jeux les plus joués</span>
          </h4>
          <div class="relative" style="height: 280px">
            <Bar v-if="topJeuxChartData" :data="topJeuxChartData" :options="barOptions" />
            <p v-else class="text-txt-dim text-sm text-center py-12">Aucune donnée</p>
          </div>
        </div>

        <div class="card w-full max-w-full min-w-0 overflow-hidden">
          <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
            <Monitor class="w-4 h-4 text-neon-blue" />
            <span>Répartition par console</span>
          </h4>
          <div class="relative flex items-center justify-center" style="height: 280px">
            <Doughnut v-if="consolesChartData" :data="consolesChartData" :options="doughnutOptions" />
            <p v-else class="text-txt-dim text-sm text-center py-12">Aucune donnée</p>
          </div>
        </div>
      </div>

      <div class="card w-full max-w-full min-w-0 overflow-hidden">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
          <TrendingUp class="w-4 h-4 text-neon-green" />
          <span>Sessions par jour (7 derniers jours)</span>
        </h4>
        <div class="relative" style="height: 260px">
          <Line v-if="sessionsLineData" :data="sessionsLineData" :options="lineOptions" />
          <p v-else class="text-txt-dim text-sm text-center py-12">Aucune donnée</p>
        </div>
      </div>

      <div class="card w-full max-w-full min-w-0 overflow-hidden">
        <h4 class="font-gaming font-bold mb-4 flex items-center gap-2">
          <Euro class="w-4 h-4 text-neon-yellow" />
          <span>Revenus par jour (7 derniers jours)</span>
        </h4>
        <div class="relative" style="height: 260px">
          <Bar v-if="revenusBarData" :data="revenusBarData" :options="revenusBarOptions" />
          <p v-else class="text-txt-dim text-sm text-center py-12">Aucune donnée</p>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 w-full max-w-full min-w-0 overflow-hidden">
        <div class="card w-full max-w-full min-w-0 overflow-hidden">
          <h4 class="font-gaming font-bold mb-4 truncate">Top Jeux</h4>
          <div class="space-y-3 w-full max-w-full min-w-0">
            <div v-for="(j, i) in stats.top_jeux || []" :key="i" class="flex items-center gap-3 w-full">
              <span class="font-gaming font-bold text-txt-dim w-6 shrink-0">{{ i + 1 }}.</span>
              <div class="flex-1 min-w-0">
                <div class="flex justify-between items-center mb-1">
                  <span class="truncate text-sm">{{ j.titre }}</span>
                  <span class="text-xs font-mono text-neon-blue shrink-0 ml-2">{{ j.pct }}%</span>
                </div>
                <div class="w-full bg-bg-surface rounded-full h-2">
                  <div class="h-2 rounded-full bg-gradient-to-r from-neon-violet to-neon-blue transition-all duration-700" :style="{ width: Math.min(j.pct, 100) + '%' }"></div>
                </div>
              </div>
            </div>
            <p v-if="!stats.top_jeux?.length" class="text-txt-dim text-sm text-center py-3">Aucune donnée</p>
          </div>
        </div>

        <div class="card w-full max-w-full min-w-0 overflow-hidden">
          <h4 class="font-gaming font-bold mb-4 truncate">Répartition par console</h4>
          <div class="space-y-3 w-full max-w-full min-w-0">
            <div v-for="(c, i) in stats.repartition_consoles || []" :key="i" class="flex items-center gap-3 w-full">
              <span class="font-gaming font-bold text-txt-dim w-6 shrink-0">{{ i + 1 }}.</span>
              <div class="flex-1 min-w-0">
                <div class="flex justify-between items-center mb-1">
                  <span class="truncate text-sm">{{ c.nom }}</span>
                  <span class="text-xs font-mono text-neon-blue shrink-0 ml-2">{{ c.pct }}%</span>
                </div>
                <div class="w-full bg-bg-surface rounded-full h-2">
                  <div class="h-2 rounded-full bg-gradient-to-r from-neon-blue to-neon-green transition-all duration-700" :style="{ width: Math.min(c.pct, 100) + '%' }"></div>
                </div>
              </div>
            </div>
            <p v-if="!stats.repartition_consoles?.length" class="text-txt-dim text-sm text-center py-3">Aucune donnée</p>
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
import Loader from '@/components/ui/Loader.vue'
import { Download, RefreshCw, BarChart3, Monitor, TrendingUp, Euro } from 'lucide-vue-next'
import { toast } from 'sonner'
import { Bar, Doughnut, Line } from 'vue-chartjs'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  BarElement,
  ArcElement,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler
} from 'chart.js'

ChartJS.register(CategoryScale, LinearScale, BarElement, ArcElement, PointElement, LineElement, Title, Tooltip, Legend, Filler)

const stats = ref<any>({})
const loading = ref(true)

async function refresh() {
  loading.value = true
  try { stats.value = await api.get('/rapports/ca') }
  catch (e: any) { toast.error('Rapports: ' + (e.message || 'erreur')) }
  finally { loading.value = false }
}

const neonColors = [
  'rgba(168, 85, 247, 0.8)',
  'rgba(6, 182, 212, 0.8)',
  'rgba(34, 197, 94, 0.8)',
  'rgba(234, 179, 8, 0.8)',
  'rgba(239, 68, 68, 0.8)',
  'rgba(236, 72, 153, 0.8)',
  'rgba(99, 102, 241, 0.8)',
  'rgba(14, 165, 233, 0.8)'
]
const neonBorders = [
  'rgba(168, 85, 247, 1)',
  'rgba(6, 182, 212, 1)',
  'rgba(34, 197, 94, 1)',
  'rgba(234, 179, 8, 1)',
  'rgba(239, 68, 68, 1)',
  'rgba(236, 72, 153, 1)',
  'rgba(99, 102, 241, 1)',
  'rgba(14, 165, 233, 1)'
]

const gridColor = 'rgba(255,255,255,0.06)'
const tickColor = 'rgba(226,232,240,0.5)'

const topJeuxChartData = computed(() => {
  const jeux = stats.value.top_jeux
  if (!jeux?.length) return null
  return {
    labels: jeux.map((j: any) => j.titre),
    datasets: [{
      label: 'Sessions',
      data: jeux.map((j: any) => j.sessions),
      backgroundColor: neonColors.slice(0, jeux.length),
      borderColor: neonBorders.slice(0, jeux.length),
      borderWidth: 1,
      borderRadius: 6,
      maxBarThickness: 50
    }]
  }
})

const barOptions = {
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

const consolesChartData = computed(() => {
  const consoles = stats.value.repartition_consoles
  if (!consoles?.length) return null
  return {
    labels: consoles.map((c: any) => c.nom),
    datasets: [{
      data: consoles.map((c: any) => c.sessions),
      backgroundColor: neonColors.slice(0, consoles.length),
      borderColor: neonBorders.slice(0, consoles.length),
      borderWidth: 2,
      hoverOffset: 8
    }]
  }
})

const doughnutOptions = {
  responsive: true,
  maintainAspectRatio: false,
  cutout: '55%',
  plugins: {
    legend: {
      position: 'bottom' as const,
      labels: { color: tickColor, font: { size: 11 }, padding: 16, usePointStyle: true, pointStyleWidth: 10 }
    },
    tooltip: {
      backgroundColor: 'rgba(18, 18, 26, 0.95)',
      titleColor: '#e2e8f0',
      bodyColor: '#06b6d4',
      borderColor: 'rgba(6, 182, 212, 0.3)',
      borderWidth: 1,
      cornerRadius: 8,
      padding: 12
    }
  }
}

const sessionsLineData = computed(() => {
  const history = stats.value.sessions_history
  if (!history?.length) return null
  return {
    labels: history.map((d: any) => d.date),
    datasets: [{
      label: 'Sessions',
      data: history.map((d: any) => d.count),
      borderColor: 'rgba(168, 85, 247, 1)',
      backgroundColor: 'rgba(168, 85, 247, 0.15)',
      borderWidth: 2,
      pointRadius: 4,
      pointBackgroundColor: 'rgba(168, 85, 247, 1)',
      pointBorderColor: '#12121a',
      pointBorderWidth: 2,
      tension: 0.4,
      fill: true
    }]
  }
})

const lineOptions = {
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

const revenusBarData = computed(() => {
  const history = stats.value.sessions_history
  if (!history?.length) return null
  return {
    labels: history.map((d: any) => d.date),
    datasets: [{
      label: 'Revenus (FC)',
      data: history.map((d: any) => d.revenus || 0),
      backgroundColor: 'rgba(34, 197, 94, 0.6)',
      borderColor: 'rgba(34, 197, 94, 1)',
      borderWidth: 1,
      borderRadius: 6,
      maxBarThickness: 50
    }]
  }
})

const revenusBarOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: { display: false },
    tooltip: {
      backgroundColor: 'rgba(18, 18, 26, 0.95)',
      titleColor: '#e2e8f0',
      bodyColor: '#22c55e',
      borderColor: 'rgba(34, 197, 94, 0.3)',
      borderWidth: 1,
      cornerRadius: 8,
      padding: 12,
      callbacks: {
        label: (ctx: any) => formatCurrency(ctx.raw)
      }
    }
  },
  scales: {
    x: { grid: { color: gridColor }, ticks: { color: tickColor, font: { size: 10 } } },
    y: { grid: { color: gridColor }, ticks: { color: tickColor, font: { size: 10 }, callback: (v: any) => formatCurrency(v) }, beginAtZero: true }
  }
}

function exportCsv() {
  try {
    const s: any = stats.value
    const rows = [
      ['Métrique', 'Valeur'],
      ['Revenus totaux', s.total_revenus || 0],
      ['Sessions totales', s.total_sessions || 0],
      ['Total joueurs', s.total_joueurs || 0],
      ["Revenus aujourd'hui", s.revenus_aujourd_hui || 0],
      ['Sessions aujourd\'hui', s.sessions_aujourd_hui || 0],
      ['Joueurs actifs', s.joueurs_actifs || 0],
      ['Jetons attribués', s.jetons_attribues || 0],
      [],
      ['Top Jeux', 'Sessions', 'Pourcentage'],
      ...(s.top_jeux || []).map((j: any) => [j.titre, j.sessions, j.pct + '%']),
      [],
      ['Répartition consoles', 'Sessions', 'Pourcentage'],
      ...(s.repartition_consoles || []).map((c: any) => [c.nom, c.sessions, c.pct + '%']),
    ]
    const csv = rows.map(r => r.map(v => `"${String(v).replace(/"/g, '""')}"`).join(',')).join('\n')
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a'); a.href = url; a.download = `rapports-${new Date().toISOString().slice(0,10)}.csv`; a.click()
    URL.revokeObjectURL(url)
    toast.success('Rapport exporté')
  } catch (e) { toast.error('Erreur export') }
}

onMounted(refresh)
</script>
