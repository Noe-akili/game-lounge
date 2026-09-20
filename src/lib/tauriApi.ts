// @ts-nocheck
// Pont frontend ↔ backend Rust/Tauri.
// Traduit les appels HTTP du vrai frontend Vue (api.get/post/put/delete)
// en commandes Tauri, avec les mêmes routes/sémantiques que server/ (Express)
// et que le mode navigateur (src/lib/clientApi.ts).

declare global {
  interface Window {
    __TAURI__?: any
    __TAURI_INTERNALS__?: any
  }
}

export function isTauriRuntime() {
  if (typeof window === 'undefined') return false
  const w: any = window as any
  return !!(w.__TAURI__?.core?.invoke || w.__TAURI_INTERNALS__?.invoke || w.__TAURI_IPC__)
}

function invoke(cmd: string, args: Record<string, unknown> = {}): Promise<any> {
  const w: any = window as any
  const fn = w.__TAURI__?.core?.invoke || w.__TAURI_INTERNALS__?.invoke
  if (!fn) return Promise.reject({ message: 'Tauri non disponible', status: 500 })
  console.log('[TAURI_INVOKE_START]', cmd, JSON.stringify(args).slice(0,200))
  // auth_login est LOCAL-FIRST : le compte présent dans SQLite doit répondre
  // rapidement. Le cloud n'est utilisé que pour un compte absent/localement
  // non valide. Le timeout IPC ne doit donc plus masquer un blocage de 75s.
  const timeoutMs = cmd.startsWith('users_') ? 8000 : ((cmd === 'auth_login' || cmd === 'auth_refresh') ? 35000 : 15000)
  return Promise.race([
    Promise.resolve(fn.call(w.__TAURI__?.core || w.__TAURI_INTERNALS__, cmd, args)).then((r:any)=>{ console.log('[TAURI_INVOKE_SUCCESS]', cmd); return r; }).catch((e:any)=>{ console.warn('[TAURI_INVOKE_ERROR]', cmd, e); throw e; }),
    new Promise((_, reject) => setTimeout(() => { console.warn('[TAURI_INVOKE_ERROR] timeout', cmd); reject({ message: 'Timeout IPC (Android WebView)', status: 504 }) }, timeoutMs))
  ])
}


function getPath(url: string): string {
  return url.split('?')[0]
}

function parseQuery(url: string): Record<string, string> {
  const idx = url.indexOf('?')
  if (idx === -1) return {}
  const q: Record<string, string> = {}
  for (const [k, v] of new URLSearchParams(url.slice(idx + 1))) q[k] = v
  return q
}

function num(v: any): number | undefined {
  if (v === undefined || v === null || v === '') return undefined
  const n = Number(v)
  return Number.isFinite(n) ? n : undefined
}

function bval(v: any): boolean | undefined {
  if (v === undefined || v === null || v === '') return undefined
  return v === true || v === 1 || v === '1' || v === 'true'
}

function val(body: any, key: string): any {
  return body ? body[key] : undefined
}

type Ctx = {
  segs: Record<string, string>
  query: Record<string, string>
  body: any
  token: string | null
}

type RouteDef = {
  m: string
  p: string
  f: (ctx: Ctx) => { cmd: string; args: Record<string, unknown> }
}

const ROUTES: RouteDef[] = [

  // ===== AUTH =====
  { m: 'POST', p: '/auth/login', f: ({ body }) => ({ cmd: 'auth_login', args: { email: val(body, 'email'), password: val(body, 'password') } }) },
  { m: 'POST', p: '/auth/bootstrap', f: () => ({ cmd: 'auth_bootstrap_admin', args: {} }) },
  { m: 'POST', p: '/auth/test-supabase', f: () => ({ cmd: 'auth_test_supabase', args: {} }) },
  { m: 'POST', p: '/auth/refresh', f: ({ body }) => ({ cmd: 'auth_refresh', args: { refresh_token: val(body, 'refresh_token') } }) },
  { m: 'POST', p: '/auth/logout', f: ({ token }) => ({ cmd: 'auth_logout', args: { token } }) },
  { m: 'POST', p: '/auth/business-ready', f: () => ({ cmd: 'auth_business_ready', args: {} }) },
  { m: 'POST', p: '/auth/business-suspend', f: () => ({ cmd: 'auth_business_suspend', args: {} }) },
  { m: 'GET', p: '/auth/me', f: ({ token }) => ({ cmd: 'auth_me', args: { token } }) },

  // ===== CONSOLES =====
  { m: 'GET', p: '/consoles', f: ({ token, query }) => ({ cmd: 'consoles_list', args: { token, includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/consoles/:id', f: ({ token, segs }) => ({ cmd: 'consoles_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/consoles', f: ({ token, body }) => ({ cmd: 'consoles_create', args: { token, nom: val(body, 'nom'), type: val(body, 'type'), posteNumero: num(val(body, 'poste_numero')), etat: val(body, 'etat'), imageUrl: val(body, 'image_url') } }) },
  { m: 'PUT', p: '/consoles/:id', f: ({ token, segs, body }) => ({ cmd: 'consoles_update', args: { token, id: num(segs.id), nom: val(body, 'nom'), type: val(body, 'type'), posteNumero: num(val(body, 'poste_numero')), etat: val(body, 'etat'), imageUrl: val(body, 'image_url') } }) },
  { m: 'DELETE', p: '/consoles/:id', f: ({ token, segs }) => ({ cmd: 'consoles_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/consoles/:id/restore', f: ({ token, segs }) => ({ cmd: 'consoles_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/consoles/:id/permanent', f: ({ token, segs }) => ({ cmd: 'consoles_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== JEUX =====
  { m: 'GET', p: '/jeux', f: ({ token, query }) => ({ cmd: 'jeux_list', args: { token, consoleId: num(query.console_id), includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/jeux/:id', f: ({ token, segs }) => ({ cmd: 'jeux_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/jeux', f: ({ token, body }) => ({ cmd: 'jeux_create', args: { token, titre: val(body, 'titre'), genre: val(body, 'genre'), consoleId: num(val(body, 'console_id')), jaquetteUrl: val(body, 'jaquette_url') } }) },
  { m: 'PUT', p: '/jeux/:id', f: ({ token, segs, body }) => ({ cmd: 'jeux_update', args: { token, id: num(segs.id), titre: val(body, 'titre'), genre: val(body, 'genre'), consoleId: val(body, 'console_id'), jaquetteUrl: val(body, 'jaquette_url') } }) },
  { m: 'DELETE', p: '/jeux/:id', f: ({ token, segs }) => ({ cmd: 'jeux_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/jeux/:id/restore', f: ({ token, segs }) => ({ cmd: 'jeux_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/jeux/:id/permanent', f: ({ token, segs }) => ({ cmd: 'jeux_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== JOUEURS =====
  { m: 'GET', p: '/joueurs', f: ({ token, query }) => ({ cmd: 'joueurs_list', args: { token, search: query.search, includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/joueurs/:id/historique', f: ({ token, segs }) => ({ cmd: 'joueurs_historique', args: { token, id: num(segs.id) } }) },
  { m: 'GET', p: '/joueurs/:id', f: ({ token, segs }) => ({ cmd: 'joueurs_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/joueurs', f: ({ token, body }) => ({ cmd: 'joueurs_create', args: { token, nom: val(body, 'nom'), telephone: val(body, 'telephone'), email: val(body, 'email'), sticker: val(body, 'sticker') } }) },
  { m: 'PUT', p: '/joueurs/:id', f: ({ token, segs, body }) => ({ cmd: 'joueurs_update', args: { token, id: num(segs.id), nom: val(body, 'nom'), telephone: val(body, 'telephone'), email: val(body, 'email'), jetonsSolde: num(val(body, 'jetons_solde')), sticker: val(body, 'sticker') } }) },
  { m: 'DELETE', p: '/joueurs/:id', f: ({ token, segs }) => ({ cmd: 'joueurs_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/joueurs/:id/restore', f: ({ token, segs }) => ({ cmd: 'joueurs_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/joueurs/:id/permanent', f: ({ token, segs }) => ({ cmd: 'joueurs_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== SESSIONS =====
  { m: 'GET', p: '/sessions', f: ({ token, query }) => ({ cmd: 'sessions_list', args: { token, statut: query.statut, includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/sessions/:id', f: ({ token, segs }) => ({ cmd: 'sessions_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/sessions', f: ({ token, body }) => ({ cmd: 'sessions_create', args: { token, consoleId: num(val(body, 'console_id')), joueurId: num(val(body, 'joueur_id')), jeuId: num(val(body, 'jeu_id')), tarifId: num(val(body, 'tarif_id')) } }) },
  { m: 'PUT', p: '/sessions/:id/pause', f: ({ token, segs }) => ({ cmd: 'sessions_pause', args: { token, id: num(segs.id) } }) },
  { m: 'PUT', p: '/sessions/:id/reprendre', f: ({ token, segs }) => ({ cmd: 'sessions_reprendre', args: { token, id: num(segs.id) } }) },
  { m: 'PUT', p: '/sessions/:id/terminer', f: ({ token, segs, body }) => ({ cmd: 'sessions_terminer', args: { token, id: num(segs.id), modePaiement: val(body, 'mode_paiement') } }) },
  { m: 'PUT', p: '/sessions/:id', f: ({ token, segs, body }) => ({ cmd: 'sessions_update', args: { token, id: num(segs.id), consoleId: num(val(body, 'console_id')), joueurId: num(val(body, 'joueur_id')), jeuId: num(val(body, 'jeu_id')), statut: val(body, 'statut'), dureeMinutes: num(val(body, 'duree_minutes')), montant: num(val(body, 'montant')) } }) },
  { m: 'DELETE', p: '/sessions/:id', f: ({ token, segs }) => ({ cmd: 'sessions_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/sessions/:id/restore', f: ({ token, segs }) => ({ cmd: 'sessions_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/sessions/:id/permanent', f: ({ token, segs }) => ({ cmd: 'sessions_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== FACTURES =====
  { m: 'GET', p: '/factures', f: ({ token, query }) => ({ cmd: 'factures_list', args: { token, statut: query.statut, joueurId: num(query.joueur_id), dateStart: query.date_start, dateEnd: query.date_end, includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/factures/:id/pdf', f: ({ token, segs }) => ({ cmd: 'factures_pdf', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/factures/:id/save-pdf', f: ({ token, segs, body }) => ({ cmd: 'factures_save_pdf', args: { token, id: num(segs.id), folder: val(body, 'folder'), filename: val(body, 'filename') } }) },
  { m: 'GET', p: '/factures/:id', f: ({ token, segs }) => ({ cmd: 'factures_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/factures', f: ({ token, body }) => ({ cmd: 'factures_create', args: { token, sessionId: num(val(body, 'session_id')), joueurId: num(val(body, 'joueur_id')), montantHt: num(val(body, 'montant_ht')), tauxTva: num(val(body, 'taux_tva')), montantTva: num(val(body, 'montant_tva')), montantTtc: num(val(body, 'montant_ttc')), modePaiement: val(body, 'mode_paiement'), statut: val(body, 'statut') } }) },
  { m: 'PUT', p: '/factures/:id/annuler', f: ({ token, segs }) => ({ cmd: 'factures_annuler', args: { token, id: num(segs.id) } }) },
  { m: 'PUT', p: '/factures/:id', f: ({ token, segs, body }) => ({ cmd: 'factures_update', args: { token, id: num(segs.id), statut: val(body, 'statut'), modePaiement: val(body, 'mode_paiement'), montantTtc: num(val(body, 'montant_ttc')) } }) },
  { m: 'DELETE', p: '/factures/:id', f: ({ token, segs }) => ({ cmd: 'factures_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/factures/:id/restore', f: ({ token, segs }) => ({ cmd: 'factures_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/factures/:id/permanent', f: ({ token, segs }) => ({ cmd: 'factures_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== LIGNES FACTURE =====
  { m: 'GET', p: '/lignes_facture', f: ({ token, query }) => ({ cmd: 'lignes_list', args: { token, factureId: num(query.facture_id) } }) },
  { m: 'GET', p: '/lignes_facture/:id', f: ({ token, segs }) => ({ cmd: 'lignes_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/lignes_facture', f: ({ token, body }) => ({ cmd: 'lignes_create', args: { token, factureId: num(val(body, 'facture_id')), description: val(body, 'description'), quantite: num(val(body, 'quantite')), prixUnitaire: num(val(body, 'prix_unitaire')), totalLigne: num(val(body, 'total_ligne')) } }) },
  { m: 'PUT', p: '/lignes_facture/:id', f: ({ token, segs, body }) => ({ cmd: 'lignes_update', args: { token, id: num(segs.id), description: val(body, 'description'), quantite: num(val(body, 'quantite')), prixUnitaire: num(val(body, 'prix_unitaire')), totalLigne: num(val(body, 'total_ligne')) } }) },
  { m: 'DELETE', p: '/lignes_facture/:id', f: ({ token, segs }) => ({ cmd: 'lignes_delete', args: { token, id: num(segs.id) } }) },

  // ===== TARIFS =====
  { m: 'GET', p: '/tarifs', f: ({ token, query }) => ({ cmd: 'tarifs_list', args: { token, includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/tarifs/:id', f: ({ token, segs }) => ({ cmd: 'tarifs_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/tarifs', f: ({ token, body }) => ({ cmd: 'tarifs_create', args: { token, type: val(body, 'type'), dureeMinutes: num(val(body, 'duree_minutes')), prix: num(val(body, 'prix')), description: val(body, 'description'), consoleType: val(body, 'console_type'), jeu: val(body, 'jeu') } }) },
  { m: 'PUT', p: '/tarifs/:id', f: ({ token, segs, body }) => ({ cmd: 'tarifs_update', args: { token, id: num(segs.id), type: val(body, 'type'), dureeMinutes: num(val(body, 'duree_minutes')), prix: num(val(body, 'prix')), description: val(body, 'description'), actif: bval(val(body, 'actif')), consoleType: val(body, 'console_type'), jeu: val(body, 'jeu') } }) },
  { m: 'DELETE', p: '/tarifs/:id', f: ({ token, segs }) => ({ cmd: 'tarifs_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/tarifs/:id/restore', f: ({ token, segs }) => ({ cmd: 'tarifs_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/tarifs/:id/permanent', f: ({ token, segs }) => ({ cmd: 'tarifs_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== JETONS =====
  { m: 'GET', p: '/jetons', f: ({ token, query }) => ({ cmd: 'jetons_list', args: { token, joueurId: num(query.joueur_id) } }) },
  { m: 'GET', p: '/jetons/:id', f: ({ token, segs }) => ({ cmd: 'jetons_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/jetons', f: ({ token, body }) => ({ cmd: 'jetons_create', args: { token, joueurId: num(val(body, 'joueur_id')), type: val(body, 'type'), quantite: num(val(body, 'quantite')), raison: val(body, 'raison'), sessionId: num(val(body, 'session_id')) } }) },
  { m: 'PUT', p: '/jetons/:id', f: ({ token, segs, body }) => ({ cmd: 'jetons_update', args: { token, id: num(segs.id), type: val(body, 'type'), quantite: num(val(body, 'quantite')), raison: val(body, 'raison') } }) },
  { m: 'DELETE', p: '/jetons/:id', f: ({ token, segs }) => ({ cmd: 'jetons_delete', args: { token, id: num(segs.id) } }) },

  // ===== MESSAGES =====
  { m: 'GET', p: '/messages', f: ({ token }) => ({ cmd: 'messages_list', args: { token } }) },
  { m: 'POST', p: '/messages', f: ({ token, body }) => ({ cmd: 'messages_create', args: { token, titre: val(body, 'titre'), contenu: val(body, 'contenu') } }) },
  { m: 'PUT', p: '/messages/:id', f: ({ token, segs, body }) => ({ cmd: 'messages_update', args: { token, id: num(segs.id), titre: val(body, 'titre'), contenu: val(body, 'contenu') } }) },
  { m: 'DELETE', p: '/messages/:id', f: ({ token, segs }) => ({ cmd: 'messages_delete', args: { token, id: num(segs.id) } }) },

  // ===== PARAMÈTRES FIDÉLITÉ =====
  { m: 'GET', p: '/parametres/fidelite', f: ({ token }) => ({ cmd: 'fidelite_get', args: { token } }) },
  { m: 'GET', p: '/parametres/fidelite/:id', f: ({ token, segs }) => ({ cmd: 'fidelite_get_by_id', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/parametres/fidelite', f: ({ token, body }) => ({ cmd: 'fidelite_create', args: { token, regleType: val(body, 'regle_type'), seuil: num(val(body, 'seuil')), jetonsAttribues: num(val(body, 'jetons_attribues')), valeurJeton: num(val(body, 'valeur_jeton')), actif: bval(val(body, 'actif')) } }) },
  { m: 'PUT', p: '/parametres/fidelite', f: ({ token, body }) => ({ cmd: 'fidelite_put', args: { token, regleType: val(body, 'regle_type'), seuil: num(val(body, 'seuil')), jetonsAttribues: num(val(body, 'jetons_attribues')), valeurJeton: num(val(body, 'valeur_jeton')), actif: bval(val(body, 'actif')) } }) },
  { m: 'DELETE', p: '/parametres/fidelite/:id', f: ({ token, segs }) => ({ cmd: 'fidelite_delete', args: { token, id: num(segs.id) } }) },

  // ===== RAPPORTS =====
  { m: 'GET', p: '/rapports/ca', f: ({ token }) => ({ cmd: 'rapports_ca', args: { token } }) },

  // ===== USERS (admin) =====
  { m: 'GET', p: '/users', f: ({ token, query }) => ({ cmd: 'users_list', args: { token, includeDeleted: bval(query.include_deleted) || false } }) },
  { m: 'GET', p: '/users/:id', f: ({ token, segs }) => ({ cmd: 'users_get', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/users', f: ({ token, body }) => ({ cmd: 'users_create', args: { token, email: val(body, 'email'), password: val(body, 'password'), role: val(body, 'role'), nom: val(body, 'nom') } }) },
  { m: 'PUT', p: '/users/:id', f: ({ token, segs, body }) => ({ cmd: 'users_update', args: { token, id: num(segs.id), email: val(body, 'email'), role: val(body, 'role'), nom: val(body, 'nom'), password: val(body, 'password') } }) },
  { m: 'DELETE', p: '/users/:id', f: ({ token, segs }) => ({ cmd: 'users_delete', args: { token, id: num(segs.id) } }) },
  { m: 'POST', p: '/users/:id/restore', f: ({ token, segs }) => ({ cmd: 'users_restore', args: { token, id: num(segs.id) } }) },
  { m: 'DELETE', p: '/users/:id/permanent', f: ({ token, segs }) => ({ cmd: 'users_permanent_delete', args: { token, id: num(segs.id) } }) },

  // ===== SYNC =====
  { m: 'GET', p: '/sync/status', f: ({ token }) => ({ cmd: 'sync_status', args: { token } }) },
  { m: 'POST', p: '/sync/toggle', f: ({ token, body }) => ({ cmd: 'sync_toggle', args: { token, enabled: !!val(body, 'enabled') } }) },
  { m: 'POST', p: '/sync/run', f: ({ token }) => ({ cmd: 'sync_run', args: { token } }) },
  { m: 'POST', p: '/sync/pull', f: ({ token }) => ({ cmd: 'sync_run', args: { token } }) },
  { m: 'POST', p: '/sync/push', f: ({ token }) => ({ cmd: 'sync_run', args: { token } }) },
  { m: 'GET', p: '/sync/poll', f: ({ token }) => ({ cmd: 'sync_poll', args: { token } }) },
  // État de l'initial (première sync faite ? quelles tables ?) — mission §5
  { m: 'GET', p: '/sync/initial-status', f: ({ token }) => ({ cmd: 'sync_initial_status', args: { token } }) },
  // BYPASS écran d'initialisation (hors ligne) : marque l'init comme faite,
  // travaille en local ; la reprise cloud se fera au retour du réseau.
  { m: 'POST', p: '/sync/initial-skip', f: ({ token }) => ({ cmd: 'sync_initial_skip', args: { token } }) },
]

function compile(pattern: string): RegExp {
  const src = pattern.replace(/:(\w+)/g, '(?<$1>[^/]+)')
  return new RegExp('^' + src + '$')
}

export async function handleTauriRequest(
  path: string,
  method: string,
  body?: any,
  authToken?: string
): Promise<{ status: number; body: any }> {
  const clean = getPath(path)
  const ready = clean.startsWith('/api') ? clean.slice(4) : clean
  const query = parseQuery(path)

  for (const r of ROUTES) {
    if (r.m !== method) continue
    const m = ready.match(compile(r.p))
    if (!m) continue

    const segs: Record<string, string> = {}
    for (const [k, v] of Object.entries(m.groups || {})) segs[k] = v

    const { cmd, args } = r.f({ segs, query, body, token: authToken || null })

    try {
      const result = await invoke(cmd, args)
      return { status: 200, body: result }
    } catch (e: any) {
      // Tauri peut rejeter avec string, objet {message,status}, ou {message:"..."}
      // Sur Android, e peut être null si WebView est détruit (rotation) -> on évite crash
      if (e == null) {
        console.warn(`[tauriApi] ${cmd} failed: null error (WebView destroyed)`)
        return { status: 503, body: { message: 'Service temporairement indisponible, réessayez' } }
      }
      let status = 500
      let message = 'Erreur interne'
      if (typeof e === 'string') {
        message = e
        try { const parsed = JSON.parse(e); if (parsed.status) status = parsed.status; if (parsed.message) message = parsed.message } catch {}
      } else if (e && typeof e === 'object') {
        status = typeof e.status === 'number' ? e.status : (typeof e.code === 'number' ? e.code : 500)
        // 504 = timeout IPC Android, on le mappe en 503 pour retry
        if (status === 504) status = 503
        message = e.message || e.error || (typeof e.toString === 'function' ? e.toString() : message)
        if (typeof message === 'string' && message.startsWith('{')) {
          try { const p = JSON.parse(message); if (p.message) message = p.message; if (p.status) status = p.status } catch {}
        }
        // Android WebView parfois renvoie {error: "database is locked"}
        if (message.includes('database is locked') || message.includes('busy')) {
          status = 503
          message = 'Base temporairement verrouillée, réessayez'
        }
      }
      // Ne JAMAIS detruire la session persistee si la requete etait partie
      // sans token (race au demarrage) : ca ne peut pas etre un token invalide.
      if (status === 401 && authToken) {
        try { localStorage.removeItem('gl_token'); localStorage.removeItem('gl_user'); localStorage.removeItem('gl_refresh_token'); window.dispatchEvent(new Event('gl:unauthorized')) } catch {}
      }
      console.warn(`[tauriApi] ${cmd} failed:`, { status, message, raw: e })
      return { status, body: { message } }
    }
  }

  return { status: 404, body: { message: `Route non trouvée: ${method} ${ready}` } }
}