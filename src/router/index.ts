// @ts-nocheck
// Router minimal propre
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  { path: '/login', name: 'Login', component: () => import('@/views/LoginView.vue') },
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
    ]
  }
]

const router = createRouter({
  history: window.location.protocol === 'file:' ? createWebHashHistory() : createWebHistory(),
  routes
})

router.beforeEach((to, _from, next) => {
  const auth = useAuthStore()
  if (to.path !== '/login' && !auth.isAuthenticated) return next('/login')
  if (to.path === '/login' && auth.isAuthenticated) return next(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
  next()
})

export default router
