// @ts-nocheck
// Best-practice router : guards async, refresh, role-based, offline-first
import { createRouter, createWebHistory, createWebHashHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/LoginView.vue'),
    meta: { requiresAuth: false, guestOnly: true }
  },
  {
    path: '/',
    component: () => import('@/components/layout/AppLayout.vue'),
    meta: { requiresAuth: true },
    children: [
      {
        path: '',
        redirect: to => {
          const auth = useAuthStore()
          return auth.user?.role === 'admin' ? '/admin' : '/dashboard'
        }
      },
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/DashboardView.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'sessions',
        name: 'Sessions',
        component: () => import('@/views/SessionsView.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'joueurs',
        name: 'Joueurs',
        component: () => import('@/views/admin/AdminJoueurs.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'paiements',
        name: 'Paiements',
        component: () => import('@/views/admin/AdminFactures.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'jetons',
        name: 'Jetons',
        component: () => import('@/views/admin/AdminJetons.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'messages',
        name: 'Messages',
        component: () => import('@/views/MessagesView.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'parametres',
        name: 'Parametres',
        component: () => import('@/views/ParametresView.vue'),
        meta: { roles: ['employe', 'admin'] }
      },
      {
        path: 'admin',
        name: 'AdminDashboard',
        component: () => import('@/views/admin/AdminDashboard.vue'),
        meta: { roles: ['admin'] }
      },
      {
        path: 'admin/consoles',
        name: 'AdminConsoles',
        component: () => import('@/views/admin/AdminConsoles.vue'),
        meta: { roles: ['admin'] }
      },
      {
        path: 'admin/jeux',
        name: 'AdminJeux',
        component: () => import('@/views/admin/AdminJeux.vue'),
        meta: { roles: ['admin'] }
      },
      {
        path: 'admin/tarifs',
        name: 'AdminTarifs',
        component: () => import('@/views/admin/AdminTarifs.vue'),
        meta: { roles: ['admin'] }
      },
      {
        path: 'admin/rapports',
        name: 'AdminRapports',
        component: () => import('@/views/admin/AdminRapports.vue'),
        meta: { roles: ['admin'] }
      },
      {
        path: 'admin/parametres',
        name: 'AdminParametres',
        component: () => import('@/views/admin/AdminParametres.vue'),
        meta: { roles: ['admin'] }
      },
      {
        path: 'admin/utilisateurs',
        name: 'AdminUtilisateurs',
        component: () => import('@/views/admin/AdminUtilisateurs.vue'),
        meta: { roles: ['admin'] }
      }
    ]
  }
]

const isFileProtocol = typeof window !== 'undefined' && window.location.protocol === 'file:'
const routerHistory = isFileProtocol ? createWebHashHistory() : createWebHistory()

const router = createRouter({
  history: routerHistory,
  routes
})

let lastBackPress = 0
if (typeof window !== 'undefined') {
  window.addEventListener('android-back-pressed', () => {
    const auth = useAuthStore()
    if (!auth.isAuthenticated) return
    const path = router.currentRoute.value.path
    if (path === '/login' || path === '/') return
    router.back()
  })
  window.addEventListener('popstate', () => {
    const now = Date.now()
    if (now - lastBackPress < 2000) return
    lastBackPress = now
  })
}

// Best-practice guard : offline-first, refresh, role check
router.beforeEach(async (to, from, next) => {
  try {
    const auth = useAuthStore()
    console.log('[ROUTER_NAVIGATION] beforeEach', from.path, '->', to.path, 'isAuth', auth.isAuthenticated, 'role', auth.user?.role, 'source', auth.loginSource)

    // Guest only (login) : si déjà auth, redirige
    if (to.meta.guestOnly && auth.isAuthenticated) {
      console.log('[ROUTER_NAVIGATION] already auth, redirect from /login')
      return next(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
    }

    // Requires auth
    if (to.meta.requiresAuth !== false && to.meta.requiresAuth !== undefined ? to.meta.requiresAuth : to.matched.some(r => r.meta.requiresAuth)) {
      // Vérifie si le path nécessite auth (parent '/' a requiresAuth true)
      const requiresAuth = to.matched.some(r => r.meta.requiresAuth) || to.meta.requiresAuth
      if (requiresAuth && !auth.isAuthenticated) {
        console.log('[ROUTER_NAVIGATION] redirect to /login (not auth)')
        return next('/login')
      }
      // Si token présent mais expiré, tente refresh (best practice)
      if (requiresAuth && auth.isAuthenticated) {
        // Vérifie token expiration côté client (décodage sans vérif)
        try {
          const payload = JSON.parse(atob(auth.token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/')))
          const now = Math.floor(Date.now() / 1000)
          if (payload.exp && payload.exp < now) {
            console.log('[ROUTER_NAVIGATION] token expired, try refresh')
            try {
              await auth.refresh()
              console.log('[ROUTER_NAVIGATION] refresh success')
            } catch {
              console.log('[ROUTER_NAVIGATION] refresh failed, redirect login')
              await auth.logout()
              return next('/login')
            }
          }
        } catch {}
      }
    }

    // Role check
    if (to.meta.roles && !to.meta.roles.includes(auth.user?.role)) {
      console.log('[ROUTER_NAVIGATION] role mismatch', auth.user?.role, 'required', to.meta.roles)
      return next(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
    }

    next()
  } catch (e) {
    console.error('[router] beforeEach failed', e)
    next('/login')
  }
})

router.onError((err) => {
  console.error('[router] navigation error (Android WebView)', err)
})
router.afterEach((to, from) => {
  console.log('[ROUTER_NAVIGATION] afterEach', from.path, '->', to.path)
})

export default router
