// @ts-nocheck
// Router minimal propre
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  { path: '/login', name: 'Login', component: () => import('@/views/LoginView.vue') },
  // Écran de première synchronisation (progression réelle du SyncEngine Rust) —
  // affiché une seule fois, entre le login et le dashboard (mission §6).
  { path: '/initialisation', name: 'Initialisation', component: () => import('@/views/InitialisationView.vue') },
  {
    path: '/',
    component: () => import('@/components/layout/AppLayout.vue'),
    children: [
      { path: '', redirect: () => { const auth = useAuthStore(); return auth.user?.role === 'admin' ? '/admin' : '/dashboard' } },
      { path: 'dashboard', component: () => import('@/views/DashboardView.vue') },
      { path: 'sessions', component: () => import('@/views/SessionsView.vue') },
      { path: 'joueurs', component: () => import('@/views/admin/AdminJoueurs.vue') },
      { path: 'paiements', component: () => import('@/views/admin/AdminFactures.vue') },
      { path: 'jetons', component: () => import('@/views/admin/AdminJetons.vue') },
      { path: 'messages', component: () => import('@/views/MessagesView.vue') },
      { path: 'parametres', component: () => import('@/views/ParametresView.vue') },
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
  if (to.path !== '/login' && !auth.isAuthenticated) return '/login'
  if (to.path === '/login' && auth.isAuthenticated) return auth.user?.role === 'admin' ? '/admin' : '/dashboard'
})

export default router
