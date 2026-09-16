// @ts-nocheck
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  { path: '/login', name: 'Login', component: () => import('@/views/LoginView.vue') },
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
      { path: 'admin', component: () => import('@/views/admin/AdminDashboard.vue'), meta: { adminOnly: true } },
      { path: 'admin/consoles', component: () => import('@/views/admin/AdminConsoles.vue'), meta: { adminOnly: true } },
      { path: 'admin/jeux', component: () => import('@/views/admin/AdminJeux.vue'), meta: { adminOnly: true } },
      { path: 'admin/tarifs', component: () => import('@/views/admin/AdminTarifs.vue'), meta: { adminOnly: true } },
      { path: 'admin/rapports', component: () => import('@/views/admin/AdminRapports.vue'), meta: { adminOnly: true } },
      { path: 'admin/parametres', component: () => import('@/views/admin/AdminParametres.vue'), meta: { adminOnly: true } },
      { path: 'admin/utilisateurs', component: () => import('@/views/admin/AdminUtilisateurs.vue'), meta: { adminOnly: true } },
      { path: 'admin/developpeur', component: () => import('@/views/admin/DeveloperView.vue'), meta: { adminOnly: true } },
    ]
  }
]

const router = createRouter({
  history: window.location.protocol === 'file:' ? createWebHashHistory() : createWebHistory(),
  routes
})

router.beforeEach((to) => {
  const auth = useAuthStore()

  // Si le token n'est pas encore dans le store, tenter la restauration depuis le stockage du téléphone
  if (!auth.token) {
    auth.restoreSession()
  }

  // Si pas de session valide persistée : accès uniquement à /login
  if (!auth.token) {
    if (to.path === '/login') return true
    return '/login'
  }

  // Si session valide et l'utilisateur se rend sur /login : renvoi direct vers l'écran d'accueil
  if (to.path === '/login') {
    return auth.isAdmin ? '/admin' : '/dashboard'
  }

  // Contrôle des routes protégées admin
  if (to.matched.some(record => record.meta.adminOnly) && !auth.isAdmin) {
    return '/dashboard'
  }

  if (to.path === '/') {
    return auth.isAdmin ? '/admin' : '/dashboard'
  }
})

export default router
