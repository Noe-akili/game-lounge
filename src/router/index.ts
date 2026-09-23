// @ts-nocheck
import { createRouter, createWebHashHistory } from 'vue-router'
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
      { path: 'documentation', component: () => import('@/views/DocumentationView.vue') },
      { path: 'admin', component: () => import('@/views/admin/AdminDashboard.vue'), meta: { adminOnly: true } },
      { path: 'admin/consoles', component: () => import('@/views/admin/AdminConsoles.vue'), meta: { adminOnly: true } },
      { path: 'admin/jeux', component: () => import('@/views/admin/AdminJeux.vue') },
      { path: 'jeux', component: () => import('@/views/admin/AdminJeux.vue') },
      { path: 'admin/tarifs', component: () => import('@/views/admin/AdminTarifs.vue'), meta: { adminOnly: true } },
      { path: 'admin/rapports', component: () => import('@/views/admin/AdminRapports.vue'), meta: { adminOnly: true } },
      { path: 'admin/parametres', component: () => import('@/views/admin/AdminParametres.vue'), meta: { adminOnly: true } },
      { path: 'admin/synchronisation', component: () => import('@/views/admin/AdminSync.vue'), meta: { adminOnly: true } },
      { path: 'admin/utilisateurs', component: () => import('@/views/admin/AdminUtilisateurs.vue'), meta: { adminOnly: true } },
    ]
  },
  // Catch-all : si l'URL démarre sur /index.html ou n'importe quel chemin inconnu au boot
  { path: '/:pathMatch(.*)*', redirect: '/login' }
]

// Sur Android Tauri WebView, createWebHashHistory garantit un affichage instantané et fiable
// sans dépendre du serveur HTTP local ni du routage d'URL HTML5
const router = createRouter({
  history: createWebHashHistory(),
  routes
})

let sessionRestoredOnce = false

router.beforeEach(async (to) => {
  const auth = useAuthStore()

  // Restauration de session unique et protégée par timeout au premier chargement
  if (!sessionRestoredOnce && !auth.token) {
    sessionRestoredOnce = true
    try {
      await auth.restoreSession()
    } catch {
      // Ignorer : si la session ne peut pas être restaurée, on va sur /login
    }
  }

  // Si pas de session valide persistée : accès uniquement à /login
  if (!auth.token) {
    if (to.path === '/login') return true
    return '/login'
  }

  // Si session valide et l'utilisateur se rend sur /login : renvoi direct vers l'accueil
  if (to.path === '/login') {
    return auth.isAdmin ? '/admin' : '/dashboard'
  }

  // Contrôle des routes protégées admin
  if (to.matched.some(record => record.meta?.adminOnly) && !auth.isAdmin) {
    return '/dashboard'
  }

  if (to.path === '/') {
    return auth.isAdmin ? '/admin' : '/dashboard'
  }
})

export default router
