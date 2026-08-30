// @ts-nocheck
import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/LoginView.vue'),
    meta: { requiresAuth: false }
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

const router = createRouter({
  history: createWebHistory(),
  routes
})

router.beforeEach((to, from, next) => {
  const auth = useAuthStore()

  if (to.meta.requiresAuth !== false && !auth.isAuthenticated) {
    return next('/login')
  }

  if (to.path === '/login' && auth.isAuthenticated) {
    return next(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
  }

  if (to.meta.roles && !to.meta.roles.includes(auth.user?.role)) {
    return next(auth.user?.role === 'admin' ? '/admin' : '/dashboard')
  }

  next()
})

export default router
