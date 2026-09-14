// @ts-nocheck
// Router mono-utilisateur admin : PAS de login, PAS de vue employé.
//
// Guard NON BLOQUANT (mission §7) : le guard ne vérifie que l'ÉTAT LOCAL
// minimal nécessaire au routage. Aucune requête réseau, aucun IPC bloquant :
//   navigation -> état local -> afficher page (le réseau tourne en background)
//
// /initialisation est réservée au VRAI premier lancement : le flag
// 'initial_sync_done' est persisté côté Rust (SQLite) et lu UNE fois en
// arrière-plan. Réseau indisponible ≠ premier lancement (mission §7).
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { SessionStorage } from '@/lib/sessionStorage'

const routes = [
  // Écran de première synchronisation (progression réelle du SyncEngine Rust) —
  // réservé au premier lancement / installation nécessitant une configuration.
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

// Flag premier lancement : cache mémoire + lecture IPC UNE SEULE FOIS,
// en arrière-plan (jamais dans le chemin critique du guard).
let firstRun: boolean | null = null
let firstRunProbe: Promise<boolean> | null = null

function probeFirstRun(): Promise<boolean> {
  if (firstRun !== null) return Promise.resolve(firstRun)
  if (firstRunProbe) return firstRunProbe
  firstRunProbe = (async () => {
    try {
      const { api } = await import('@/utils/api')
      // Heuristique persistée : la table users est vide = l'installation
      // n'a encore rien synchronisé/créé -> VRAI premier lancement.
      const users = await api.get('/users')
      firstRun = Array.isArray(users) && users.length === 0
    } catch {
      // Réseau/IPC indisponible : on NE considère PAS ça comme un premier
      // lancement (mission §7). L'app s'ouvre sur l'admin normalement.
      firstRun = false
    }
    firstRunProbe = null
    return firstRun
  })()
  return firstRunProbe
}

router.beforeEach(async (to) => {
  const auth = useAuthStore()
  const { api } = await import('@/utils/api')

  // === Guard = ÉTAT LOCAL uniquement (mission §7) ===
  // 1. Session locale présente (token+user synchrones) -> navigation directe.
  if (auth.token && auth.user) {
    // La vérification serveur (/auth/me) a déjà été lancée par restoreSession
    // en arrière-plan : on ne l'attend PAS ici.
  } else if (SessionStorage.getSessionSync().token) {
    // Session persistée non encore chargée dans le store : hydrate le store
    // SANS réseau, puis laisse filer la navigation.
    const s = SessionStorage.getSessionSync()
    auth.token = s.token
    auth.user = s.user
  } else if (to.path !== '/initialisation') {
    // 2. Aucune session locale : PREMIER LANCEMENT possible. La décision
    //    /initialisation est prise sur le flag persisté, en arrière-plan —
    //    on navigue d'abord (bootstrap lui-même lancé en background plus bas).
    probeFirstRun().then((isFirst) => {
      if (isFirst && router.currentRoute.value.path !== '/initialisation') {
        router.replace('/initialisation')
      }
    })
  }

  // 3. Bootstrap / restauration : lancés SANS bloquer la navigation.
  //    restoreSession est idempotente + concurrent-safe (Promise partagée).
  if (!auth.token || !auth.user) {
    auth.restoreSession().catch(() => {})
  }

  // 4. Redirection '/' -> '/admin' (local, instantané).
  if (to.path === '/') return '/admin'
})

export default router
