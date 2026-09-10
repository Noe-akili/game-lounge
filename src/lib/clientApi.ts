// @ts-nocheck
import { queryAll, queryOne, insert, update, remove, scheduleSave } from './clientDb'

const JWT_SECRET = 'game-lounge-secret-key-2024'

function sanitizeInput(input: string, maxLength = 500): string {
  if (typeof input !== 'string') return ''
  return input.replace(/[<>]/g, '').trim().slice(0, maxLength)
}

function verifyToken(token: string): any {
  try {
    const decoded = JSON.parse(atob(token))
    if (decoded.exp && decoded.exp < Date.now()) return null
    return decoded
  } catch { return null }
}

function signToken(payload: any): string {
  const data = { ...payload, exp: Date.now() + 24 * 60 * 60 * 1000 }
  return btoa(JSON.stringify(data))
}

function error(status: number, message: string): { status: number; body: any } {
  return { status, body: { message } }
}

function ok(body: any): { status: number; body: any } {
  return { status: 200, body }
}

function created(body: any): { status: number; body: any } {
  return { status: 201, body }
}

type RouteResult = { status: number; body: any }

type Handler = (params: { path: string; method: string; body: any; query: any; user: any }) => RouteResult | Promise<RouteResult>

const routes: { pattern: RegExp; method: string; handler: Handler; auth?: boolean; admin?: boolean }[] = []

function route(method: string, pattern: string, handler: Handler, auth = true, admin = false) {
  const regex = new RegExp('^' + pattern.replace(/:(\w+)/g, '(?<$1>[^/]+)') + '$')
  routes.push({ pattern: regex, method, handler, auth, admin })
}

function parseQuery(url: string): any {
  const idx = url.indexOf('?')
  if (idx === -1) return {}
  const params = new URLSearchParams(url.slice(idx + 1))
  const q: any = {}
  for (const [k, v] of params) q[k] = v
  return q
}

function getPath(url: string): string {
  return url.split('?')[0]
}

// ===== AUTH =====
route('GET', '/api/health', () => ok({ status: 'ok', timestamp: Date.now() }), false)

route('POST', '/api/auth/login', async ({ body }) => {
  const { email, password } = body || {}
  if (!email || !password) return error(400, 'Email et mot de passe requis')

  const bcrypt = await require_bcrypt()
  const user = queryOne('users', (u: any) => u.email === email)
  if (!user || !bcrypt.compareSync(password, user.password_hash)) {
    return error(401, 'Identifiants incorrects')
  }

  const token = signToken({ id: user.id, email: user.email, role: user.role, nom: user.nom })
  return ok({ token, user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
}, false)

route('POST', '/api/auth/logout', () => ok({ success: true }))

route('GET', '/api/auth/me', ({ user }) => {
  const u = queryOne('users', (x: any) => x.id === user.id)
  if (!u) return error(404, 'Utilisateur non trouvé')
  return ok({ user: { id: u.id, email: u.email, role: u.role, nom: u.nom } })
})

// ===== CONSOLES =====
route('GET', '/api/consoles', () => {
  const consoles = queryAll('consoles')
  const sessions = queryAll('sessions_jeu').filter((s: any) => s.statut === 'en_cours' || s.statut === 'pause')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')

  const result = consoles.map((c: any) => {
    const session = sessions.find((s: any) => s.console_id === c.id)
    const joueur = session ? joueurs.find((j: any) => j.id === session.joueur_id) : null
    const jeu = session ? jeux.find((j: any) => j.id === session.jeu_id) : null
    return {
      ...c, session_id: session?.id || null, session_statut: session?.statut || null,
      session_debut: session?.debut || null, joueur_id: session?.joueur_id || null,
      jeu_id: session?.jeu_id || null, tarif_prix: session?.tarif_prix || null,
      joueur_nom: joueur?.nom || null, jeu_nom: jeu?.titre || null,
    }
  }).sort((a: any, b: any) => a.poste_numero - b.poste_numero)
  return ok(result)
})

route('GET', '/api/consoles/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const c = queryOne('consoles', (x: any) => x.id === id)
  if (!c) return error(404, 'Console non trouvée')
  return ok(c)
})

route('POST', '/api/consoles', ({ body }) => {
  const { nom, type, poste_numero, etat } = body
  if (!nom || !type || !poste_numero) return error(400, 'Champs requis manquants')
  const c = insert('consoles', { nom: sanitizeInput(nom, 50), type, poste_numero: Number(poste_numero), etat: etat || 'disponible', created_at: new Date().toISOString() })
  return created(c)
}, true, true)

route('PUT', '/api/consoles/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const { nom, type, poste_numero, etat } = body
  const updates: any = {}
  if (nom !== undefined) updates.nom = sanitizeInput(nom, 50)
  if (type !== undefined) updates.type = type
  if (poste_numero !== undefined) updates.poste_numero = Number(poste_numero)
  if (etat !== undefined) updates.etat = sanitizeInput(String(etat), 50)
  const c = update('consoles', id, updates)
  if (!c) return error(404, 'Console non trouvée')
  return ok(c)
}, true, true)

route('DELETE', '/api/consoles/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  remove('consoles', id)
  return ok({ success: true })
}, true, true)

// ===== JEUX =====
route('GET', '/api/jeux', ({ query }) => {
  let jeux = queryAll('jeux').filter((j: any) => j.actif !== false && j.actif !== 0)
  if (query.console_id) jeux = jeux.filter((j: any) => j.console_id === Number(query.console_id))
  return ok(jeux)
})

route('GET', '/api/jeux/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const j = queryOne('jeux', (x: any) => x.id === id)
  if (!j) return error(404, 'Jeu non trouvé')
  return ok(j)
})

route('POST', '/api/jeux', ({ body }) => {
  const { titre, genre, console_id, jaquette_url } = body
  if (!titre) return error(400, 'Titre requis')
  const j = insert('jeux', { titre: sanitizeInput(titre, 100), genre: genre ? sanitizeInput(genre, 50) : genre, console_id: console_id ? Number(console_id) : null, jaquette_url: jaquette_url ? sanitizeInput(String(jaquette_url), 500) : jaquette_url, actif: 1, created_at: new Date().toISOString() })
  return created(j)
}, true, true)

route('PUT', '/api/jeux/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const { titre, genre, console_id, jaquette_url } = body
  const updates: any = {}
  if (titre !== undefined) updates.titre = sanitizeInput(titre, 100)
  if (genre !== undefined) updates.genre = genre ? sanitizeInput(genre, 50) : genre
  if (console_id !== undefined) updates.console_id = console_id ? Number(console_id) : null
  if (jaquette_url !== undefined) updates.jaquette_url = jaquette_url ? sanitizeInput(String(jaquette_url), 500) : jaquette_url
  const j = update('jeux', id, updates)
  if (!j) return error(404, 'Jeu non trouvé')
  return ok(j)
}, true, true)

route('DELETE', '/api/jeux/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  remove('jeux', id)
  return ok({ success: true })
}, true, true)

// ===== JOUEURS =====
route('GET', '/api/joueurs', ({ query }) => {
  let joueurs = queryAll('joueurs')
  if (query.search) {
    const q = query.search.toLowerCase()
    joueurs = joueurs.filter((j: any) => j.nom?.toLowerCase().includes(q) || j.telephone?.includes(q) || j.email?.toLowerCase().includes(q))
  }
  return ok(joueurs)
})

route('GET', '/api/joueurs/:id/historique', ({ path }) => {
  const id = Number(getPath(path).split('/')[3])
  const joueur = queryOne('joueurs', (j: any) => j.id === id)
  if (!joueur) return error(404, 'Joueur non trouvé')
  const sessions = queryAll('sessions_jeu').filter((s: any) => s.joueur_id === id).sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || ''))
  const transactions = queryAll('jetons_transactions').filter((t: any) => t.joueur_id === id).sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || ''))
  const factures = queryAll('factures').filter((f: any) => f.joueur_id === id).sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || ''))
  const consoles = queryAll('consoles')
  const jeux = queryAll('jeux')
  const enrichedSessions = sessions.map((s: any) => ({
    ...s, console_nom: consoles.find((c: any) => c.id === s.console_id)?.nom, jeu_nom: jeux.find((j: any) => j.id === s.jeu_id)?.titre,
  }))
  return ok({ joueur, sessions: enrichedSessions, transactions, factures })
})

route('GET', '/api/joueurs/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const j = queryOne('joueurs', (x: any) => x.id === id)
  if (!j) return error(404, 'Joueur non trouvé')
  return ok(j)
})

route('POST', '/api/joueurs', ({ body }) => {
  const { nom, telephone, email } = body
  if (!nom) return error(400, 'Nom requis')
  const j = insert('joueurs', { nom: sanitizeInput(nom, 50), telephone: telephone ? String(telephone).replace(/[\s\-]/g, '') : '', email: email ? sanitizeInput(String(email), 100) : '', jetons_solde: 0, date_inscription: new Date().toISOString(), derniere_visite: null })
  return created(j)
})

route('PUT', '/api/joueurs/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const { nom, telephone, email, jetons_solde } = body
  const updates: any = {}
  if (nom !== undefined) updates.nom = sanitizeInput(nom, 50)
  if (telephone !== undefined) updates.telephone = telephone ? String(telephone).replace(/[\s\-]/g, '') : ''
  if (email !== undefined) updates.email = email ? sanitizeInput(String(email), 100) : ''
  if (jetons_solde !== undefined) updates.jetons_solde = Number(jetons_solde)
  const j = update('joueurs', id, updates)
  if (!j) return error(404, 'Joueur non trouvé')
  return ok(j)
})

route('DELETE', '/api/joueurs/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const j = queryOne('joueurs', (x: any) => x.id === id)
  if (!j) return error(404, 'Joueur non trouvé')
  remove('joueurs', id)
  return ok({ success: true })
})

// ===== SESSIONS =====
route('GET', '/api/sessions', ({ query }) => {
  let sessions = queryAll('sessions_jeu')
  if (query.statut) sessions = sessions.filter((s: any) => s.statut === query.statut)
  sessions.sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || ''))
  const consoles = queryAll('consoles')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')
  const users = queryAll('users')
  const result = sessions.map((s: any) => ({
    ...s,
    console_nom: consoles.find((c: any) => c.id === s.console_id)?.nom,
    console_type: consoles.find((c: any) => c.id === s.console_id)?.type,
    poste_numero: consoles.find((c: any) => c.id === s.console_id)?.poste_numero,
    joueur_nom: joueurs.find((j: any) => j.id === s.joueur_id)?.nom,
    jeu_nom: jeux.find((j: any) => j.id === s.jeu_id)?.titre,
    employe_nom: users.find((u: any) => u.id === s.employe_id)?.nom,
  }))
  return ok(result)
})

route('GET', '/api/sessions/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const s = queryOne('sessions_jeu', (x: any) => x.id === id)
  if (!s) return error(404, 'Session non trouvée')
  const console_ = queryOne('consoles', (c: any) => c.id === s.console_id)
  const joueur = queryOne('joueurs', (j: any) => j.id === s.joueur_id)
  const jeu = queryOne('jeux', (j: any) => j.id === s.jeu_id)
  const dureeSecondes = (s.statut === 'en_cours')
    ? Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000)
    : (s.statut === 'pause' ? (s.duree_minutes || 0) * 60 : Math.floor((new Date(s.fin).getTime() - new Date(s.debut).getTime()) / 1000))
  return ok({ ...s, console_nom: console_?.nom, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, jeu_nom: jeu?.titre, duree_secondes: Math.max(0, dureeSecondes) })
})

route('POST', '/api/sessions', ({ body, user }) => {
  const { console_id, joueur_id, jeu_id, tarif_id } = body
  if (!console_id || !joueur_id || !jeu_id) return error(400, 'Console, joueur et jeu requis')
  const existing = queryOne('sessions_jeu', (s: any) => s.console_id === Number(console_id) && (s.statut === 'en_cours' || s.statut === 'pause'))
  if (existing) return error(400, 'Cette console a déjà une session en cours')

  let tarif_prix = 2000, duree_minutes = 60, tarif_id_val = null
  if (tarif_id) {
    const selectedTarif = queryOne('tarifs', (t: any) => t.id === Number(tarif_id))
    if (selectedTarif) { tarif_prix = selectedTarif.prix || 2000; duree_minutes = selectedTarif.duree_minutes || 60; tarif_id_val = selectedTarif.id }
  }

  const session = insert('sessions_jeu', {
    console_id: Number(console_id), joueur_id: Number(joueur_id), jeu_id: Number(jeu_id),
    employe_id: user.id, tarif_id: tarif_id_val, debut: new Date().toISOString(), fin: null,
    duree_minutes, montant: tarif_prix, tarif_prix, jetons_gagnes: 0, statut: 'en_cours', created_at: new Date().toISOString()
  })
  update('consoles', Number(console_id), { etat: 'occupee' })
  update('joueurs', Number(joueur_id), { derniere_visite: new Date().toISOString() })

  const consoles = queryAll('consoles')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')
  return created({
    ...session,
    console_nom: consoles.find((c: any) => c.id === session.console_id)?.nom,
    joueur_nom: joueurs.find((j: any) => j.id === session.joueur_id)?.nom,
    jeu_nom: jeux.find((j: any) => j.id === session.jeu_id)?.titre,
  })
})

route('PUT', '/api/sessions/:id/pause', ({ path }) => {
  const id = Number(getPath(path).split('/')[3])
  const s = queryOne('sessions_jeu', (x: any) => x.id === id)
  if (!s) return error(404, 'Session non trouvée')
  if (s.statut !== 'en_cours') return error(400, 'Session non en cours')
  const elapsed = Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000)
  const totalDuree = (s.duree_minutes || 0) * 60 + elapsed
  update('sessions_jeu', id, { statut: 'pause', duree_minutes: Math.floor(totalDuree / 60) })
  update('consoles', s.console_id, { etat: 'pause' })
  return ok(queryOne('sessions_jeu', (x: any) => x.id === id))
})

route('PUT', '/api/sessions/:id/reprendre', ({ path }) => {
  const id = Number(getPath(path).split('/')[3])
  const s = queryOne('sessions_jeu', (x: any) => x.id === id)
  if (!s) return error(404, 'Session non trouvée')
  if (s.statut !== 'pause') return error(400, 'Session non en pause')
  update('sessions_jeu', id, { statut: 'en_cours', debut: new Date().toISOString() })
  update('consoles', s.console_id, { etat: 'occupee' })
  return ok(queryOne('sessions_jeu', (x: any) => x.id === id))
})

route('PUT', '/api/sessions/:id/terminer', ({ path }) => {
  const id = Number(getPath(path).split('/')[3])
  const s = queryOne('sessions_jeu', (x: any) => x.id === id)
  if (!s) return error(404, 'Session non trouvée')

  const elapsed = s.statut === 'en_cours' ? Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000) : 0
  const totalDureeSecondes = (s.duree_minutes || 0) * 60 + elapsed
  const dureeMinutes = Math.max(1, Math.ceil(totalDureeSecondes / 60))
  const montant = Math.ceil(dureeMinutes / 60) * (s.tarif_prix || 2000)

  update('sessions_jeu', id, { statut: 'terminee', fin: new Date().toISOString(), duree_minutes: dureeMinutes, montant })
  update('consoles', s.console_id, { etat: 'disponible' })

  const now = new Date()
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, '')
  const random = String(Math.floor(Math.random() * 9999)).padStart(4, '0')
  const numeroFacture = `FAC-${dateStr}-${random}`
  const montantHT = Math.round(montant / 1.2)
  const tauxTva = 20
  const montantTva = montant - montantHT

  const facture = insert('factures', {
    numero_facture: numeroFacture, session_id: id, joueur_id: s.joueur_id,
    montant_ht: montantHT, taux_tva: tauxTva, montant_tva: montantTva, montant_ttc: montant,
    mode_paiement: 'especes', statut: 'payee', date_paiement: now.toISOString(), created_at: now.toISOString()
  })

  const console_ = queryOne('consoles', (c: any) => c.id === s.console_id)
  const jeu = queryOne('jeux', (j: any) => j.id === s.jeu_id)
  insert('lignes_facture', {
    facture_id: facture.id, description: sanitizeInput(`Session ${console_?.nom || ''} - ${jeu?.titre || ''} - ${dureeMinutes}min`, 500),
    quantite: 1, prix_unitaire: montant, total_ligne: montant
  })

  const regle = queryOne('parametres_fidelite', (r: any) => r.actif !== false && r.actif !== 0)
  let jetonsGagnes = 0
  if (regle?.regle_type === 'temps') {
    jetonsGagnes = Math.floor(dureeMinutes / (regle.seuil || 60)) * (regle.jetons_attribues || 1)
  }
  if (jetonsGagnes > 0) {
    const joueur = queryOne('joueurs', (j: any) => j.id === s.joueur_id)
    update('joueurs', s.joueur_id, { jetons_solde: (joueur?.jetons_solde || 0) + jetonsGagnes })
    insert('jetons_transactions', {
      joueur_id: s.joueur_id, type: 'gain', quantite: jetonsGagnes,
      raison: sanitizeInput(`Session ${dureeMinutes}min - ${console_?.nom || ''}`, 500), session_id: id, created_at: now.toISOString()
    })
  }

  const joueur = queryOne('joueurs', (j: any) => j.id === s.joueur_id)
  const enrichedFacture = { ...facture, joueur_nom: joueur?.nom, lignes: queryAll('lignes_facture').filter((l: any) => l.facture_id === facture.id) }
  return ok({ session: queryOne('sessions_jeu', (x: any) => x.id === id), facture: enrichedFacture, montant, jetonsGagnes, dureeMinutes })
})

route('PUT', '/api/sessions/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const s = queryOne('sessions_jeu', (x: any) => x.id === id)
  if (!s) return error(404, 'Session non trouvée')
  const { console_id, joueur_id, jeu_id, statut, duree_minutes, montant } = body
  const updates: any = {}
  if (console_id !== undefined) updates.console_id = Number(console_id)
  if (joueur_id !== undefined) updates.joueur_id = Number(joueur_id)
  if (jeu_id !== undefined) updates.jeu_id = Number(jeu_id)
  if (statut !== undefined) updates.statut = statut
  if (duree_minutes !== undefined) updates.duree_minutes = Number(duree_minutes)
  if (montant !== undefined) updates.montant = Number(montant)
  return ok(update('sessions_jeu', id, updates))
})

route('DELETE', '/api/sessions/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  remove('sessions_jeu', id)
  return ok({ success: true })
}, true, true)

// ===== FACTURES =====
route('GET', '/api/factures', ({ query }) => {
  let factures = queryAll('factures')
  if (query.statut) factures = factures.filter((f: any) => f.statut === query.statut)
  if (query.joueur_id) factures = factures.filter((f: any) => f.joueur_id === Number(query.joueur_id))
  if (query.date_start) factures = factures.filter((f: any) => f.created_at >= query.date_start)
  if (query.date_end) factures = factures.filter((f: any) => f.created_at <= query.date_end + 'T23:59:59')
  factures.sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || ''))
  const joueurs = queryAll('joueurs')
  return ok(factures.map((f: any) => ({ ...f, joueur_nom: joueurs.find((j: any) => j.id === f.joueur_id)?.nom || 'N/A' })))
})

route('GET', '/api/factures/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const f = queryOne('factures', (x: any) => x.id === id)
  if (!f) return error(404, 'Facture non trouvée')
  const joueur = queryOne('joueurs', (j: any) => j.id === f.joueur_id)
  const lignes = queryAll('lignes_facture').filter((l: any) => l.facture_id === id)
  return ok({ ...f, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, lignes })
})

route('POST', '/api/factures', ({ body }) => {
  const { session_id, joueur_id, montant_ht, taux_tva, montant_tva, montant_ttc, mode_paiement, statut } = body
  if (!session_id || !joueur_id || montant_ttc === undefined) return error(400, 'Champs requis manquants')
  const now = new Date()
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, '')
  const random = String(Math.floor(Math.random() * 9999)).padStart(4, '0')
  const numero_facture = `FAC-${dateStr}-${random}`
  const f = insert('factures', {
    numero_facture, session_id: Number(session_id), joueur_id: Number(joueur_id),
    montant_ht: Number(montant_ht) || 0, taux_tva: Number(taux_tva) || 20, montant_tva: Number(montant_tva) || 0, montant_ttc: Number(montant_ttc),
    mode_paiement: mode_paiement || 'especes', statut: statut || 'payee', date_paiement: now.toISOString(), created_at: now.toISOString()
  })
  return created(f)
})

route('PUT', '/api/factures/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const f = queryOne('factures', (x: any) => x.id === id)
  if (!f) return error(404, 'Facture non trouvée')
  const { statut, mode_paiement, montant_ttc } = body
  const updates: any = {}
  if (statut !== undefined) updates.statut = statut
  if (mode_paiement !== undefined) updates.mode_paiement = mode_paiement
  if (montant_ttc !== undefined) updates.montant_ttc = Number(montant_ttc)
  return ok(update('factures', id, updates))
}, true, true)

route('PUT', '/api/factures/:id/annuler', ({ path }) => {
  const id = Number(getPath(path).split('/')[3])
  const f = queryOne('factures', (x: any) => x.id === id)
  if (!f) return error(404, 'Facture non trouvée')
  update('factures', id, { statut: 'annulee' })
  const updated = queryOne('factures', (x: any) => x.id === id)
  const joueur = queryOne('joueurs', (j: any) => j.id === updated.joueur_id)
  return ok({ ...updated, joueur_nom: joueur?.nom })
}, true, true)

route('DELETE', '/api/factures/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  remove('factures', id)
  return ok({ success: true })
}, true, true)

// ===== LIGNES FACTURE =====
route('GET', '/api/lignes_facture', ({ query }) => {
  let lignes = queryAll('lignes_facture')
  if (query.facture_id) lignes = lignes.filter((l: any) => l.facture_id === Number(query.facture_id))
  return ok(lignes)
})

route('POST', '/api/lignes_facture', ({ body }) => {
  const { facture_id, description, quantite, prix_unitaire, total_ligne } = body
  if (!facture_id || !description || quantite === undefined || prix_unitaire === undefined) return error(400, 'Champs requis manquants')
  const l = insert('lignes_facture', { facture_id: Number(facture_id), description: sanitizeInput(String(description), 500), quantite: Number(quantite), prix_unitaire: Number(prix_unitaire), total_ligne: Number(total_ligne) || Number(quantite) * Number(prix_unitaire) })
  return created(l)
})

// ===== TARIFS =====
route('GET', '/api/tarifs', () => ok(queryAll('tarifs')))

route('GET', '/api/tarifs/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  const t = queryOne('tarifs', (x: any) => x.id === id)
  if (!t) return error(404, 'Tarif non trouvé')
  return ok(t)
})

route('POST', '/api/tarifs', ({ body }) => {
  const { type, duree_minutes, prix, description, console_type, jeu } = body
  if (!type || !duree_minutes || !prix) return error(400, 'Champs requis manquants')
  const t = insert('tarifs', { type, duree_minutes: Number(duree_minutes), prix: Number(prix), description: description ? sanitizeInput(String(description), 500) : '', actif: true, console_type: console_type || null, jeu: jeu || null })
  return created(t)
}, true, true)

route('PUT', '/api/tarifs/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const t = queryOne('tarifs', (x: any) => x.id === id)
  if (!t) return error(404, 'Tarif non trouvé')
  const { type, duree_minutes, prix, description, actif, console_type, jeu } = body
  const updates: any = {}
  if (type !== undefined) updates.type = type
  if (duree_minutes !== undefined) updates.duree_minutes = Number(duree_minutes)
  if (prix !== undefined) updates.prix = Number(prix)
  if (description !== undefined) updates.description = description ? sanitizeInput(String(description), 500) : ''
  if (actif !== undefined) updates.actif = actif
  if (console_type !== undefined) updates.console_type = console_type
  if (jeu !== undefined) updates.jeu = jeu
  return ok(update('tarifs', id, updates))
}, true, true)

route('DELETE', '/api/tarifs/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  remove('tarifs', id)
  return ok({ success: true })
}, true, true)

// ===== JETONS =====
route('GET', '/api/jetons', ({ query }) => {
  let transactions = queryAll('jetons_transactions')
  if (query.joueur_id) transactions = transactions.filter((t: any) => t.joueur_id === Number(query.joueur_id))
  transactions.sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || ''))
  const joueurs = queryAll('joueurs')
  return ok(transactions.map((t: any) => ({ ...t, joueur_nom: joueurs.find((j: any) => j.id === t.joueur_id)?.nom })))
})

route('POST', '/api/jetons', ({ body }) => {
  const { joueur_id, type, quantite, raison, session_id } = body
  if (!joueur_id || !type || quantite === undefined) return error(400, 'Champs requis manquants')
  const t = insert('jetons_transactions', { joueur_id: Number(joueur_id), type, quantite: Number(quantite), raison: raison ? sanitizeInput(String(raison), 500) : '', session_id: session_id ? Number(session_id) : null, created_at: new Date().toISOString() })
  if (type === 'gain' || type === 'bonus') {
    const joueur = queryOne('joueurs', (j: any) => j.id === Number(joueur_id))
    if (joueur) update('joueurs', Number(joueur_id), { jetons_solde: (joueur.jetons_solde || 0) + Number(quantite) })
  } else if (type === 'depense') {
    const joueur = queryOne('joueurs', (j: any) => j.id === Number(joueur_id))
    if (joueur) update('joueurs', Number(joueur_id), { jetons_solde: Math.max(0, (joueur.jetons_solde || 0) - Number(quantite)) })
  }
  return created(t)
})

// ===== MESSAGES =====
route('GET', '/api/messages', () => {
  const messages = queryAll('messages')
  return ok(messages.sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || '')))
})

route('POST', '/api/messages', ({ body, user }) => {
  const { titre, contenu } = body
  if (!contenu || !contenu.trim()) return error(400, 'Contenu requis')
  const msg = insert('messages', { titre: titre || null, contenu: contenu.trim().slice(0, 1000), auteur: user?.nom || 'Système', created_at: new Date().toISOString() })
  return created(msg)
})

route('PUT', '/api/messages/:id', ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const { titre, contenu } = body
  const updates: any = {}
  if (titre !== undefined) updates.titre = titre
  if (contenu !== undefined) updates.contenu = contenu.trim().slice(0, 1000)
  return ok(update('messages', id, updates))
})

route('DELETE', '/api/messages/:id', ({ path }) => {
  const id = Number(getPath(path).split('/').pop())
  remove('messages', id)
  return ok({ success: true })
})

// ===== PARAMETRES FIDELITE =====
route('GET', '/api/parametres/fidelite', () => {
  const regle = queryOne('parametres_fidelite', (r: any) => r.actif !== false && r.actif !== 0)
  return ok(regle || { id: 0, regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
})

route('PUT', '/api/parametres/fidelite', ({ body }) => {
  const { regle_type, seuil, jetons_attribues, actif } = body
  const existing = queryOne('parametres_fidelite', () => true)
  if (existing) {
    const updates: any = {}
    if (regle_type !== undefined) updates.regle_type = regle_type
    if (seuil !== undefined) updates.seuil = Number(seuil)
    if (jetons_attribues !== undefined) updates.jetons_attribues = Number(jetons_attribues)
    if (actif !== undefined) updates.actif = !!actif
    update('parametres_fidelite', existing.id, updates)
  } else {
    insert('parametres_fidelite', { regle_type, seuil: Number(seuil), jetons_attribues: Number(jetons_attribues), actif: !!actif })
  }
  return ok(queryOne('parametres_fidelite', () => true))
}, true, true)

// ===== RAPPORTS =====
route('GET', '/api/rapports/ca', () => {
  const factures = queryAll('factures').filter((f: any) => f.statut === 'payee')
  const sessions = queryAll('sessions_jeu')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')
  const consoles = queryAll('consoles')

  const today = new Date()
  const todayStr = today.toISOString().slice(0, 10)
  const todayFactures = factures.filter((f: any) => (f.date_paiement || '').startsWith(todayStr))
  const todaySessions = sessions.filter((s: any) => (s.created_at || '').startsWith(todayStr))
  const totalRevenus = factures.reduce((s: number, f: any) => s + (f.montant_ttc || 0), 0)
  const totalSessions = sessions.length

  const topJeux = jeux.map((j: any) => {
    const count = sessions.filter((s: any) => s.jeu_id === j.id).length
    return { titre: j.titre, sessions: count, pct: totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }
  }).sort((a: any, b: any) => b.sessions - a.sessions).slice(0, 5)

  const repartitionConsoles = consoles.map((c: any) => {
    const count = sessions.filter((s: any) => s.console_id === c.id).length
    return { nom: c.nom, sessions: count, pct: totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }
  }).sort((a: any, b: any) => b.sessions - a.sessions)

  const days7: any[] = []
  for (let i = 6; i >= 0; i--) {
    const d = new Date(today)
    d.setDate(d.getDate() - i)
    const ds = d.toISOString().slice(0, 10)
    days7.push({
      date: ds.slice(5),
      count: sessions.filter((s: any) => (s.created_at || '').startsWith(ds)).length,
      revenus: factures.filter((f: any) => (f.date_paiement || '').startsWith(ds)).reduce((s: number, f: any) => s + (f.montant_ttc || 0), 0)
    })
  }

  return ok({
    revenus_aujourd_hui: todayFactures.reduce((s: number, f: any) => s + (f.montant_ttc || 0), 0),
    sessions_aujourd_hui: todaySessions.length,
    joueurs_actifs: new Set(todaySessions.map((s: any) => s.joueur_id)).size,
    jetons_attribues: queryAll('jetons_transactions').filter((t: any) => (t.created_at || '').startsWith(todayStr) && t.type === 'gain').reduce((s: number, t: any) => s + (t.quantite || 0), 0),
    total_revenus: totalRevenus, total_sessions: totalSessions, total_joueurs: joueurs.length,
    top_jeux: topJeux, repartition_consoles: repartitionConsoles, sessions_history: days7,
  })
})

// ===== USERS (admin) =====
route('GET', '/api/users', () => {
  const users = queryAll('users').map((u: any) => ({ id: u.id, email: u.email, role: u.role, nom: u.nom, created_at: u.created_at }))
  return ok(users)
}, true, true)

route('POST', '/api/users', async ({ body }) => {
  const { email, password, role, nom } = body
  if (!email || !password || !role || !nom) return error(400, 'Nom, email, mot de passe et rôle requis')
  const bcrypt = await require_bcrypt()
  const password_hash = bcrypt.hashSync(password, 10)
  const u = insert('users', { email: sanitizeInput(email, 100), password_hash, nom: sanitizeInput(nom, 50), role, created_at: new Date().toISOString() })
  return created({ id: u.id, email: u.email, role: u.role, nom: u.nom, created_at: u.created_at })
}, true, true)

route('PUT', '/api/users/:id', async ({ path, body }) => {
  const id = Number(getPath(path).split('/').pop())
  const user = queryOne('users', (u: any) => u.id === id)
  if (!user) return error(404, 'Utilisateur non trouvé')
  const { email, role, nom, password } = body
  const updates: any = {}
  if (email !== undefined) updates.email = sanitizeInput(email, 100)
  if (role !== undefined) updates.role = role
  if (nom !== undefined) updates.nom = sanitizeInput(nom, 50)
  if (password) { const bcrypt = await require_bcrypt(); updates.password_hash = bcrypt.hashSync(password, 10) }
  const updated = update('users', id, updates)
  return ok({ id: updated.id, email: updated.email, role: updated.role, nom: updated.nom, created_at: updated.created_at })
}, true, true)

route('DELETE', '/api/users/:id', ({ path, user }) => {
  const id = Number(getPath(path).split('/').pop())
  const u = queryOne('users', (x: any) => x.id === id)
  if (!u) return error(404, 'Utilisateur non trouvé')
  if (u.id === user.id) return error(400, 'Impossible de supprimer votre propre compte')
  remove('users', id)
  return ok({ success: true })
}, true, true)

// ===== SYNC (no-op in browser) =====
route('GET', '/api/sync/status', () => ok({ neonEnabled: false, neonAvailable: false, hasLocalData: true }), true, true)
route('POST', '/api/sync/toggle', () => ok({ enabled: false }), true, true)
route('POST', '/api/sync/run', () => ok({ success: true, message: 'Mode navigateur - pas de synchronisation' }), true, true)
route('GET', '/api/sync/poll', () => ok({ changes: {}, timestamp: new Date().toISOString() }))

// ===== Bcrypt lazy load =====
let _bcryptPromise: Promise<any> | null = null
function require_bcrypt(): any {
  if (!_bcryptPromise) {
    _bcryptPromise = import('bcryptjs')
  }
  return _bcryptPromise
}

// ===== MAIN HANDLER =====
export async function handleClientRequest(path: string, method: string, body?: any, authToken?: string): Promise<{ status: number; body: any }> {
  // Accepte "…/api/consoles" comme "…/consoles" (les apps appellent sans préfixe).
  const rawPath = getPath(path)
  const cleanPath = rawPath.startsWith('/api') ? rawPath : '/api' + rawPath
  const query = parseQuery(path)

  let user: any = null
  if (authToken) {
    user = verifyToken(authToken)
    if (!user) return error(401, 'Token invalide')
  }

  for (const r of routes) {
    if (r.method !== method) continue
    const match = cleanPath.match(r.pattern)
    if (!match) continue

    if (r.auth && !user) return error(401, 'Token manquant')
    if (r.admin && user?.role !== 'admin') return error(403, 'Accès réservé aux administrateurs')

    try {
      return await r.handler({ path, method, body, query, user })
    } catch (e: any) {
      console.error('Client API error:', e)
      return error(500, e.message || 'Erreur interne')
    }
  }

  return error(404, `Route non trouvée: ${method} ${cleanPath}`)
}
