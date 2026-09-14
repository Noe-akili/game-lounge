// @ts-nocheck
// Router : LOGIN obligatoire (aucune session automatique).
//
// Flux imposé (mission §6-7) :
//   App start -> écran LOGIN (aucun processus métier, pas de sync, pas de
//   SQLite business, pas de notifications, pas de watcher).
//   Login OK -> token/user sauvegardés -> redirection accueil
//   processus métier démarrent APRÈS affichage de l'écran d'accueil (App.vue).
//   Login ERREUR -> "Identifiants incorrects" -> rester sur login.
//
// Le guard n'effectue AUCUN appel réseau, AUCUN check SQLite, AUCUNES
// side effects. Il se contente de vérifier la présence locale du token.
// Si pas de token -> écran login. Si token présent -> user a déjà validé
// ses identifiants par le passé, on laisse passer (la session a été
// sauvegardée explicitement lors du login Supabase).
//
// /initialisation n'est plus gérée par le guard : elle est atteinte uniquement
// APRÈS un login réussi via le flux LoginView -> home -> App.vue.
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  { path: '/login', name: 'Login', component: () => import('@/views/LoginView.vue') },
  // Écran de première synchronisation (progression réelle du SyncEngine Rust) —
  // uniquement quand initial_sync_completed = false (première installation).
  // Atteint APRÈS login réussi, jamais imposé par le guard.
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
  // Vérification ABSOLUMENT locale : aucun appel API, aucune side effect.
  // Pas de token (jamais connecté ou session effacée) -> écran login.
  if (!auth.token) {
    // La page login reste accessible si on n'est pas connecté (après logout
    // ou expiration) : on ne redirige QUE si une session existe.
    if (to.path === '/login') return auth.isAuthenticated ? '/dashboard' : undefined
    return '/login'
  }
  // Token présent -> l'utilisateur a déjà validé ses identifiants.
  // On laisse passer vers l'accueil.
  // Les vérifications admin se font via le meta sur les routes.
  if (to.matched.some(record => record.meta.adminOnly) && !auth.isAdmin) {
    return '/dashboard'
  }
  if (to.path === '/') {
    return auth.isAdmin ? '/admin' : '/dashboard'
  }
})

export default router
