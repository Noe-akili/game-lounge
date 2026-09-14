// @ts-nocheck
// Router mono-utilisateur admin : PAS de login, PAS de vue employé.
// Tout le monde voit la même interface admin ; l'accès est implicite.
// Seule exception : l'écran d'initialisation lors de la première sync.
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  // Écran de première synchronisation (progression réelle du SyncEngine Rust) —
  // affiché une seule fois, au premier lancement (mission §6).
  { path: '/initialisation', name: 'Initialisation', component: () => import('@/views/InitialisationView.vue') },
  {
    path: '/',
    component: () => import('@/components/layout/AppLayout.vue'),
    children: [
      { path: '', redirect: '/admin' },
      { path: 'sessions', component: () => import('@/views/SessionsView.vue') },
      { path: 'joueurs', component: () => import('@/views/admin/AdminJoueurs.vue') },
      { path: 'paiements', component: () => import('@/views/admin/AdminFactures.vue') },
      { path: 'jetons', component: () => import('@/views/admin/AdminJetons.vue') },
      { path: 'messages', component: () => import('@/views/MessagesView.vue') },
      { path: 'admin', component: () => import('@/views/admin/AdminDashboard.vue') },
      { path: 'admin/consoles', component: () => import('@/views/admin/AdminConsoles.vue') },
      { path: 'admin/jeux', component: () => import('@/views/admin/AdminJeux.vue') },
      { path: 'admin/tarifs', component: () => import('@/views/admin/AdminTarifs.vue') },
      { path: 'admin/rapports', component: () => import('@/views/admin/AdminRapports.vue') },
      { path: 'admin/parametres', component: () => import('@/views/admin/AdminParametres.vue') },
      { path: 'admin/utilisateurs', component: () => import('@/views/admin/AdminUtilisateurs.vue') },
      { path: 'admin/developpeur', component: () => import('@/views/admin/DeveloperView.vue') },
    ]
  }
]

const router = createRouter({
  history: window.location.protocol === 'file:' ? createWebHashHistory() : createWebHistory(),
  routes
})

router.beforeEach(async (to) => {
  const auth = useAuthStore()
  await auth.restoreSession()
  // Première installation : écran d'initialisation (progression réelle) une
  // seule fois ; ensuite tout le monde va directement sur l'interface admin.
  if (to.path !== '/initialisation') {
    try {
      const { useSyncStore } = await import('@/stores/sync')
      const sync = useSyncStore()
      if (sync.initialSyncCompleted === null) await sync.fetchInitialStatus()
      if (sync.initialSyncCompleted === false && to.path !== '/initialisation') return '/initialisation'
    } catch {}
  }
})

export default router
