// @ts-nocheck
// Router : LOGIN obligatoire (aucune session automatique).
//
// Flux de boot (mission §2) :
//   App start -> session locale valide -> Dashboard
//             -> aucune session        -> /login
//   Le guard ne lit QUE l'état local (token/user persistés) : il n'attend
//   JAMAIS le réseau. La vérification serveur (/auth/me) tourne en
//   arrière-plan via restoreSession ; un 401 invalide la session et le
//   guard renvoie alors naturellement à /login (aucune boucle : la session
//   est effacée UNE seule fois, ensuite l'écran login s'affiche).
//
// /initialisation est réservée à la VRAIE première synchronisation : elle est
// atteinte uniquement APRÈS un login réussi (décision de LoginView sur le flag
// persisté initial_sync_completed). Elle ne remplace jamais l'écran de login
// et n'est jamais imposée par le guard à cause d'un réseau indisponible.
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  { path: '/login', name: 'Login', component: () => import('@/views/LoginView.vue') },
  // Écran de première synchronisation (progression réelle du SyncEngine Rust) —
  // uniquement quand initial_sync_completed = false (première installation).
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
  // Décision 100% locale (synchrone) : hydrate le store depuis la session
  // persistée. La vérification serveur éventuelle part en arrière-plan.
  auth.restoreSession()

  // L'écran de login est inaccessible quand on est déjà connecté.
  if (to.path === '/login') {
    return auth.isAuthenticated ? (auth.isAdmin ? '/admin' : '/dashboard') : undefined
  }
  // Toute autre page — y compris /initialisation (elle fait partie du flux
  // post-login, elle ne remplace JAMAIS l'écran de connexion) — exige une
  // session locale.
  if (!auth.isAuthenticated) {
    return '/login'
  }
  // Pages admin réservées au rôle admin.
  if (to.matched.some(record => record.meta.adminOnly) && !auth.isAdmin) {
    return '/dashboard'
  }
  if (to.path === '/') {
    return auth.isAdmin ? '/admin' : '/dashboard'
  }
})

export default router
