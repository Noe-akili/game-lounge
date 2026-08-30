// @ts-nocheck
import express from 'express'
import cors from 'cors'
import helmet from 'helmet'
import rateLimit from 'express-rate-limit'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import { existsSync } from 'node:fs'
import { queryAll, queryOne, insert, update, remove, logError, getSyncStatus, setSyncEnabled, runFullSync, isNeonEnabled } from './db.ts'
import bcrypt from 'bcryptjs'
import jwt from 'jsonwebtoken'
import {
  isValidEmail,
  isValidPassword,
  isValidNom,
  isValidTitre,
  isValidGenre,
  isValidRole,
  isValidConsoleType,
  isValidTarifType,
  isValidRegleType,
  isValidJetonType,
  isValidPhone,
  isValidPosteNumero,
  isValidDuree,
  isValidPrix,
  isValidSeuil,
  isValidJetonsAttribues,
  isValidQuantite,
  isValidId,
  isValidSessionStatut,
  isValidFactureStatut,
  isValidModePaiement,
  isValidContenu,
  sanitizeInput,
} from './utils/validators.ts'

const __dirname = dirname(fileURLToPath(import.meta.url))
const JWT_SECRET: string = process.env.JWT_SECRET || 'game-lounge-secret-2024'
const app = express()
const PORT: number = Number(process.env.PORT) || 3001

app.use(helmet())
app.use(cors())
app.use(express.json())

// Rate limiter for login - prevent brute force
const loginLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 20,
  standardHeaders: true,
  legacyHeaders: false,
  message: { message: 'Trop de tentatives, réessayez plus tard' },
})

interface JwtUser {
  id: number
  email: string
  role: 'admin' | 'employe'
  nom: string
}

function authMiddleware(req: any, res: any, next: any) {
  const header: string | undefined = req.headers.authorization
  if (!header || !header.startsWith('Bearer ')) return res.status(401).json({ message: 'Token manquant' })
  try {
    req.user = jwt.verify(header.split(' ')[1], JWT_SECRET) as JwtUser
    next()
  } catch { return res.status(401).json({ message: 'Token invalide' }) }
}

function adminOnly(req: any, res: any, next: any) {
  if (req.user?.role !== 'admin') return res.status(403).json({ message: 'Accès réservé aux administrateurs' })
  next()
}

// ===== AUTH =====
app.post('/api/auth/login', loginLimiter, async (req: any, res: any) => {
  const { email, password } = req.body as { email: string; password: string }
  if (!email || !password) return res.status(400).json({ message: 'Email et mot de passe requis' })
  if (!isValidEmail(email)) return res.status(400).json({ message: 'Email invalide' })
  if (!isValidPassword(password)) return res.status(400).json({ message: 'Mot de passe invalide (min 6 caractères, au moins une lettre)' })
  const user: any = await queryOne('users', u => u.email === email)
  if (!user || !bcrypt.compareSync(password, user.password_hash)) return res.status(401).json({ message: 'Identifiants incorrects' })
  const token = jwt.sign({ id: user.id, email: user.email, role: user.role, nom: user.nom }, JWT_SECRET, { expiresIn: '24h' })
  res.json({ token, user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
})

app.get('/api/auth/me', authMiddleware, async (req: any, res: any) => {
  const user: any = await queryOne('users', u => u.id === req.user.id)
  if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
  res.json({ user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
})

// ===== CONSOLES =====
app.get('/api/consoles', authMiddleware, async (req: any, res: any) => {
  const consoles: any[] = await queryAll('consoles')
  const sessions: any[] = (await queryAll('sessions_jeu')).filter(s => s.statut === 'en_cours' || s.statut === 'pause')
  const joueurs: any[] = await queryAll('joueurs')
  const jeux: any[] = await queryAll('jeux')

  const result = consoles.map(c => {
    const session = sessions.find(s => s.console_id === c.id)
    const joueur = session ? joueurs.find(j => j.id === session.joueur_id) : null
    const jeu = session ? jeux.find(j => j.id === session.jeu_id) : null
    return {
      ...c,
      session_id: session?.id || null,
      session_statut: session?.statut || null,
      session_debut: session?.debut || null,
      joueur_id: session?.joueur_id || null,
      jeu_id: session?.jeu_id || null,
      tarif_prix: session?.tarif_prix || null,
      joueur_nom: joueur?.nom || null,
      jeu_nom: jeu?.titre || null,
    }
  }).sort((a: any, b: any) => a.poste_numero - b.poste_numero)
  res.json(result)
})

app.post('/api/consoles', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { nom, type, poste_numero, etat } = req.body
  if (!nom || !type || !poste_numero) return res.status(400).json({ message: 'Champs requis manquants' })
  if (!isValidNom(nom)) return res.status(400).json({ message: 'Nom invalide (2-50 caractères)' })
  if (!isValidConsoleType(type)) return res.status(400).json({ message: 'Type de console invalide' })
  if (!isValidPosteNumero(poste_numero)) return res.status(400).json({ message: 'Numéro de poste invalide (1-100)' })
  const sanitizedNom = sanitizeInput(nom, 50)
  const c = await insert('consoles', { nom: sanitizedNom, type, poste_numero: Number(poste_numero), etat: etat || 'disponible', created_at: new Date().toISOString() })
  res.status(201).json(c)
})

app.put('/api/consoles/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const { nom, type, poste_numero, etat } = req.body
  const updates: Record<string, unknown> = {}
  if (nom !== undefined) {
    if (!isValidNom(nom)) return res.status(400).json({ message: 'Nom invalide (2-50 caractères)' })
    updates.nom = sanitizeInput(nom, 50)
  }
  if (type !== undefined) {
    if (!isValidConsoleType(type)) return res.status(400).json({ message: 'Type de console invalide' })
    updates.type = type
  }
  if (poste_numero !== undefined) {
    if (!isValidPosteNumero(poste_numero)) return res.status(400).json({ message: 'Numéro de poste invalide (1-100)' })
    updates.poste_numero = Number(poste_numero)
  }
  if (etat !== undefined) updates.etat = sanitizeInput(String(etat), 50)
  const c = await update('consoles', id, updates)
  if (!c) return res.status(404).json({ message: 'Console non trouvée' })
  res.json(c)
})

app.delete('/api/consoles/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  await remove('consoles', Number(req.params.id))
  res.json({ success: true })
})

// ===== JEUX =====
app.get('/api/jeux', authMiddleware, async (req: any, res: any) => {
  const { console_id } = req.query as { console_id?: string }
  let jeux: any[] = (await queryAll('jeux')).filter(j => j.actif !== false)
  if (console_id) jeux = jeux.filter(j => j.console_id === Number(console_id))
  res.json(jeux)
})

app.post('/api/jeux', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { titre, genre, console_id, jaquette_url } = req.body
  if (!titre) return res.status(400).json({ message: 'Titre requis' })
  if (!isValidTitre(titre)) return res.status(400).json({ message: 'Titre invalide (2-100 caractères)' })
  if (genre && !isValidGenre(genre)) return res.status(400).json({ message: 'Genre invalide (2-50 caractères)' })
  if (console_id !== undefined && console_id !== null && console_id !== '' && !isValidId(console_id)) return res.status(400).json({ message: 'Console ID invalide' })
  const sanitizedTitre = sanitizeInput(titre, 100)
  const sanitizedGenre = genre ? sanitizeInput(genre, 50) : genre
  const sanitizedJaquette = jaquette_url ? sanitizeInput(String(jaquette_url), 500) : jaquette_url
  const j = await insert('jeux', { titre: sanitizedTitre, genre: sanitizedGenre, console_id: console_id ? Number(console_id) : null, jaquette_url: sanitizedJaquette, actif: true, created_at: new Date().toISOString() })
  res.status(201).json(j)
})

app.put('/api/jeux/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const { titre, genre, console_id, jaquette_url } = req.body
  const updates: Record<string, unknown> = {}
  if (titre !== undefined) {
    if (!isValidTitre(titre)) return res.status(400).json({ message: 'Titre invalide (2-100 caractères)' })
    updates.titre = sanitizeInput(titre, 100)
  }
  if (genre !== undefined) {
    if (genre && !isValidGenre(genre)) return res.status(400).json({ message: 'Genre invalide' })
    updates.genre = genre ? sanitizeInput(genre, 50) : genre
  }
  if (console_id !== undefined) {
    if (console_id !== null && console_id !== '' && !isValidId(console_id)) return res.status(400).json({ message: 'Console ID invalide' })
    updates.console_id = console_id ? Number(console_id) : null
  }
  if (jaquette_url !== undefined) updates.jaquette_url = jaquette_url ? sanitizeInput(String(jaquette_url), 500) : jaquette_url
  const j = await update('jeux', id, updates)
  if (!j) return res.status(404).json({ message: 'Jeu non trouvé' })
  res.json(j)
})

app.delete('/api/jeux/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  await remove('jeux', Number(req.params.id))
  res.json({ success: true })
})

// ===== JOUEURS =====
app.get('/api/joueurs', authMiddleware, async (req: any, res: any) => {
  const { search } = req.query as { search?: string }
  let joueurs: any[] = await queryAll('joueurs')
  if (search) {
    const q = search.toLowerCase()
    joueurs = joueurs.filter(j => j.nom?.toLowerCase().includes(q) || j.telephone?.includes(q) || j.email?.toLowerCase().includes(q))
  }
  res.json(joueurs)
})

app.post('/api/joueurs', authMiddleware, async (req: any, res: any) => {
  const { nom, telephone, email } = req.body
  if (!nom) return res.status(400).json({ message: 'Nom requis' })
  if (!isValidNom(nom)) return res.status(400).json({ message: 'Nom invalide (2-50 caractères)' })
  if (telephone && !isValidPhone(String(telephone))) return res.status(400).json({ message: 'Téléphone invalide (8-15 chiffres, ex: +243...)' })
  if (email && !isValidEmail(String(email))) return res.status(400).json({ message: 'Email invalide' })
  const sanitizedNom = sanitizeInput(nom, 50)
  const sanitizedEmail = email ? sanitizeInput(String(email), 100) : ''
  const j = await insert('joueurs', { nom: sanitizedNom, telephone: telephone ? String(telephone).replace(/[\s\-]/g, '') : '', email: sanitizedEmail, jetons_solde: 0, date_inscription: new Date().toISOString(), derniere_visite: null })
  res.status(201).json(j)
})

app.put('/api/joueurs/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const { nom, telephone, email, jetons_solde } = req.body
  const updates: Record<string, unknown> = {}
  if (nom !== undefined) {
    if (!isValidNom(nom)) return res.status(400).json({ message: 'Nom invalide (2-50 caractères)' })
    updates.nom = sanitizeInput(nom, 50)
  }
  if (telephone !== undefined) {
    if (telephone && !isValidPhone(String(telephone))) return res.status(400).json({ message: 'Téléphone invalide (8-15 chiffres, ex: +243...)' })
    updates.telephone = telephone ? String(telephone).replace(/[\s\-]/g, '') : ''
  }
  if (email !== undefined) {
    if (email && !isValidEmail(String(email))) return res.status(400).json({ message: 'Email invalide' })
    updates.email = email ? sanitizeInput(String(email), 100) : ''
  }
  if (jetons_solde !== undefined) {
    if (!Number.isInteger(Number(jetons_solde)) || Number(jetons_solde) < 0) return res.status(400).json({ message: 'Jetons invalides' })
    updates.jetons_solde = Number(jetons_solde)
  }
  const j = await update('joueurs', id, updates)
  if (!j) return res.status(404).json({ message: 'Joueur non trouvé' })
  res.json(j)
})

app.get('/api/joueurs/:id/historique', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  const joueur: any = await queryOne('joueurs', j => j.id === id)
  if (!joueur) return res.status(404).json({ message: 'Joueur non trouvé' })

  const sessions: any[] = (await queryAll('sessions_jeu')).filter(s => s.joueur_id === id).sort((a: any, b: any) => new Date(b.created_at as string).getTime() - new Date(a.created_at as string).getTime())
  const transactions: any[] = (await queryAll('jetons_transactions')).filter(t => t.joueur_id === id).sort((a: any, b: any) => new Date(b.created_at as string).getTime() - new Date(a.created_at as string).getTime())
  const factures: any[] = (await queryAll('factures')).filter(f => f.joueur_id === id).sort((a: any, b: any) => new Date(b.created_at as string).getTime() - new Date(a.created_at as string).getTime())

  const consoles: any[] = await queryAll('consoles')
  const jeux: any[] = await queryAll('jeux')

  const enrichedSessions = sessions.map(s => ({
    ...s,
    console_nom: consoles.find(c => c.id === s.console_id)?.nom,
    jeu_nom: jeux.find(j => j.id === s.jeu_id)?.titre,
  }))

  res.json({ joueur, sessions: enrichedSessions, transactions, factures })
})

// ===== MESSAGES =====
app.get('/api/messages', authMiddleware, async (req: any, res: any) => {
  const messages = await queryAll('messages')
  res.json(messages.sort((a: any, b: any) => (b.created_at || '').localeCompare(a.created_at || '')))
})

app.post('/api/messages', authMiddleware, async (req: any, res: any) => {
  const { titre, contenu } = req.body
  if (!contenu || !contenu.trim()) return res.status(400).json({ message: 'Contenu requis' })
  const msg = await insert('messages', {
    titre: titre || null,
    contenu: contenu.trim().slice(0, 1000),
    auteur: req.user?.nom || 'Système',
    created_at: new Date().toISOString()
  })
  res.status(201).json(msg)
})

app.put('/api/messages/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const { titre, contenu } = req.body
  const updates: any = {}
  if (titre !== undefined) updates.titre = titre
  if (contenu !== undefined) updates.contenu = contenu.trim().slice(0, 1000)
  const msg = await update('messages', id, updates)
  res.json(msg)
})

app.delete('/api/messages/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  await remove('messages', id)
  res.json({ success: true })
})

// ===== TARIFS =====
app.get('/api/tarifs', authMiddleware, async (req: any, res: any) => {
  res.json(await queryAll('tarifs'))
})

app.post('/api/tarifs', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { type, duree_minutes, prix, description, console_type, jeu } = req.body
  if (!type || !duree_minutes || !prix) return res.status(400).json({ message: 'Champs requis manquants' })
  if (!isValidTarifType(type)) return res.status(400).json({ message: 'Type de tarif invalide' })
  if (!isValidDuree(duree_minutes)) return res.status(400).json({ message: 'Durée invalide (1-1000)' })
  if (!isValidPrix(prix)) return res.status(400).json({ message: 'Prix invalide (1-1000000)' })
  const sanitizedDesc = description ? sanitizeInput(String(description), 500) : ''
  const t = await insert('tarifs', { type, duree_minutes: Number(duree_minutes), prix: Number(prix), description: sanitizedDesc, actif: true, console_type: console_type || null, jeu: jeu || null })
  res.status(201).json(t)
})

app.put('/api/tarifs/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const { type, duree_minutes, prix, description, actif, console_type, jeu } = req.body
  const updates: Record<string, unknown> = {}
  if (type !== undefined) {
    if (!isValidTarifType(type)) return res.status(400).json({ message: 'Type de tarif invalide' })
    updates.type = type
  }
  if (duree_minutes !== undefined) {
    if (!isValidDuree(duree_minutes)) return res.status(400).json({ message: 'Durée invalide (1-1000)' })
    updates.duree_minutes = Number(duree_minutes)
  }
  if (prix !== undefined) {
    if (!isValidPrix(prix)) return res.status(400).json({ message: 'Prix invalide (1-1000000)' })
    updates.prix = Number(prix)
  }
  if (description !== undefined) updates.description = description ? sanitizeInput(String(description), 500) : ''
  if (actif !== undefined) updates.actif = actif
  if (console_type !== undefined) updates.console_type = console_type
  if (jeu !== undefined) updates.jeu = jeu
  const t = await update('tarifs', id, updates)
  if (!t) return res.status(404).json({ message: 'Tarif non trouvé' })
  res.json(t)
})

app.delete('/api/tarifs/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  await remove('tarifs', Number(req.params.id))
  res.json({ success: true })
})

app.get('/api/parametres/fidelite', authMiddleware, async (req: any, res: any) => {
  const regle: any = await queryOne('parametres_fidelite', r => r.actif !== false)
  res.json(regle || { id: 0, regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
})

app.put('/api/parametres/fidelite', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { regle_type, seuil, jetons_attribues, actif } = req.body
  if (regle_type !== undefined && !isValidRegleType(regle_type)) return res.status(400).json({ message: 'Type de règle invalide' })
  if (seuil !== undefined && !isValidSeuil(seuil)) return res.status(400).json({ message: 'Seuil invalide (1-10000)' })
  if (jetons_attribues !== undefined && !isValidJetonsAttribues(jetons_attribues)) return res.status(400).json({ message: 'Jetons attribués invalides (1-1000)' })
  const existing: any = await queryOne('parametres_fidelite', () => true)
  if (existing) {
    const updates: Record<string, unknown> = {}
    if (regle_type !== undefined) updates.regle_type = regle_type
    if (seuil !== undefined) updates.seuil = Number(seuil)
    if (jetons_attribues !== undefined) updates.jetons_attribues = Number(jetons_attribues)
    if (actif !== undefined) updates.actif = !!actif
    await update('parametres_fidelite', existing.id, updates)
  } else {
    if (!regle_type || seuil === undefined || jetons_attribues === undefined) return res.status(400).json({ message: 'Champs requis manquants' })
    await insert('parametres_fidelite', { regle_type, seuil: Number(seuil), jetons_attribues: Number(jetons_attribues), actif: !!actif })
  }
  res.json(await queryOne('parametres_fidelite', () => true))
})

// ===== SESSIONS =====
app.get('/api/sessions', authMiddleware, async (req: any, res: any) => {
  const { statut } = req.query as { statut?: string }
  let sessions: any[] = await queryAll('sessions_jeu')
  if (statut) sessions = sessions.filter(s => s.statut === statut)
  sessions.sort((a: any, b: any) => new Date(b.created_at as string).getTime() - new Date(a.created_at as string).getTime())

  const consoles: any[] = await queryAll('consoles')
  const joueurs: any[] = await queryAll('joueurs')
  const jeux: any[] = await queryAll('jeux')
  const users: any[] = await queryAll('users')

  const result = sessions.map(s => ({
    ...s,
    console_nom: consoles.find(c => c.id === s.console_id)?.nom,
    console_type: consoles.find(c => c.id === s.console_id)?.type,
    poste_numero: consoles.find(c => c.id === s.console_id)?.poste_numero,
    joueur_nom: joueurs.find(j => j.id === s.joueur_id)?.nom,
    jeu_nom: jeux.find(j => j.id === s.jeu_id)?.titre,
    employe_nom: users.find(u => u.id === s.employe_id)?.nom,
  }))
  res.json(result)
})

app.get('/api/sessions/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const s: any = await queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  const console_: any = await queryOne('consoles', c => c.id === s.console_id)
  const joueur: any = await queryOne('joueurs', j => j.id === s.joueur_id)
  const jeu: any = await queryOne('jeux', j => j.id === s.jeu_id)
  const dureeSecondes = (s.statut === 'en_cours')
    ? Math.floor((Date.now() - new Date(s.debut as string).getTime()) / 1000)
    : (s.statut === 'pause' ? (s.duree_minutes as number) * 60 : Math.floor((new Date(s.fin as string).getTime() - new Date(s.debut as string).getTime()) / 1000))
  res.json({ ...s, console_nom: console_?.nom, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, jeu_nom: jeu?.titre, duree_secondes: Math.max(0, dureeSecondes) })
})

app.post('/api/sessions', authMiddleware, async (req: any, res: any) => {
  const { console_id, joueur_id, jeu_id, tarif_id } = req.body
  if (!console_id || !joueur_id || !jeu_id) return res.status(400).json({ message: 'Console, joueur et jeu requis' })
  if (!isValidId(console_id)) return res.status(400).json({ message: 'Console ID invalide' })
  if (!isValidId(joueur_id)) return res.status(400).json({ message: 'Joueur ID invalide' })
  if (!isValidId(jeu_id)) return res.status(400).json({ message: 'Jeu ID invalide' })

  const existingSession: any = await queryOne('sessions_jeu', s => s.console_id === Number(console_id) && (s.statut === 'en_cours' || s.statut === 'pause'))
  if (existingSession) return res.status(400).json({ message: 'Cette console a déjà une session en cours' })

  let tarif_prix: number = 2000
  let duree_minutes: number = 60
  let tarif_id_val: number | null = null

  if (tarif_id) {
    const selectedTarif: any = await queryOne('tarifs', t => t.id === Number(tarif_id))
    if (selectedTarif) {
      tarif_prix = selectedTarif.prix || 2000
      duree_minutes = selectedTarif.duree_minutes || 60
      tarif_id_val = selectedTarif.id
    }
  }

  const session = await insert('sessions_jeu', {
    console_id: Number(console_id), joueur_id: Number(joueur_id), jeu_id: Number(jeu_id),
    employe_id: req.user.id, tarif_id: tarif_id_val, debut: new Date().toISOString(), fin: null,
    duree_minutes, montant: tarif_prix, tarif_prix, jetons_gagnes: 0,
    statut: 'en_cours', created_at: new Date().toISOString()
  })
  await update('consoles', Number(console_id), { etat: 'occupee' })
  await update('joueurs', Number(joueur_id), { derniere_visite: new Date().toISOString() })

  const consoles: any[] = await queryAll('consoles')
  const joueurs: any[] = await queryAll('joueurs')
  const jeux: any[] = await queryAll('jeux')

  res.status(201).json({
    ...session,
    console_nom: consoles.find(c => c.id === (session as any).console_id)?.nom,
    joueur_nom: joueurs.find(j => j.id === (session as any).joueur_id)?.nom,
    jeu_nom: jeux.find(j => j.id === (session as any).jeu_id)?.titre,
  })
})

app.put('/api/sessions/:id/pause', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const s: any = await queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  if (s.statut !== 'en_cours') return res.status(400).json({ message: 'Session non en cours' })

  const elapsed = Math.floor((Date.now() - new Date(s.debut as string).getTime()) / 1000)
  const totalDuree = (s.duree_minutes as number || 0) * 60 + elapsed

  await update('sessions_jeu', id, { statut: 'pause', duree_minutes: Math.floor(totalDuree / 60) })
  await update('consoles', s.console_id as number, { etat: 'pause' })

  const updated = await queryOne('sessions_jeu', x => x.id === id)
  res.json(updated)
})

app.put('/api/sessions/:id/reprendre', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const s: any = await queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  if (s.statut !== 'pause') return res.status(400).json({ message: 'Session non en pause' })

  await update('sessions_jeu', id, { statut: 'en_cours', debut: new Date().toISOString() })
  await update('consoles', s.console_id as number, { etat: 'occupee' })

  const updated = await queryOne('sessions_jeu', x => x.id === id)
  res.json(updated)
})

app.put('/api/sessions/:id/terminer', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const s: any = await queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })

  const elapsed = s.statut === 'en_cours' ? Math.floor((Date.now() - new Date(s.debut as string).getTime()) / 1000) : 0
  const totalDureeSecondes = (s.duree_minutes as number || 0) * 60 + elapsed
  const dureeMinutes = Math.max(1, Math.ceil(totalDureeSecondes / 60))
  const montant = Math.ceil(dureeMinutes / 60) * (s.tarif_prix as number || 2000)

  await update('sessions_jeu', id, { statut: 'terminee', fin: new Date().toISOString(), duree_minutes: dureeMinutes, montant })
  await update('consoles', s.console_id as number, { etat: 'disponible' })

  const now = new Date()
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, '')
  const random = String(Math.floor(Math.random() * 9999)).padStart(4, '0')
  const numeroFacture = `FAC-${dateStr}-${random}`
  const montantHT = Math.round(montant / 1.2)
  const tauxTva = 20
  const montantTva = montant - montantHT

  const facture = await insert('factures', {
    numero_facture: numeroFacture, session_id: id, joueur_id: s.joueur_id,
    montant_ht: montantHT, taux_tva: tauxTva, montant_tva: montantTva, montant_ttc: montant,
    mode_paiement: 'especes', statut: 'payee', date_paiement: now.toISOString(), created_at: now.toISOString()
  })

  const console_: any = await queryOne('consoles', c => c.id === s.console_id)
  const jeu: any = await queryOne('jeux', j => j.id === s.jeu_id)
  await insert('lignes_facture', {
    facture_id: (facture as any).id,
    description: sanitizeInput(`Session ${console_?.nom || ''} - ${jeu?.titre || ''} - ${dureeMinutes}min`, 500),
    quantite: 1, prix_unitaire: montant, total_ligne: montant
  })

  const regle: any = await queryOne('parametres_fidelite', r => r.actif !== false)
  let jetonsGagnes = 0
  if (regle?.regle_type === 'temps') {
    jetonsGagnes = Math.floor(dureeMinutes / (regle.seuil as number || 60)) * (regle.jetons_attribues as number || 1)
  }
  if (jetonsGagnes > 0) {
    const joueur: any = await queryOne('joueurs', j => j.id === s.joueur_id)
    await update('joueurs', s.joueur_id as number, { jetons_solde: (joueur?.jetons_solde as number || 0) + jetonsGagnes })
    await insert('jetons_transactions', {
      joueur_id: s.joueur_id, type: 'gain', quantite: jetonsGagnes,
      raison: sanitizeInput(`Session ${dureeMinutes}min - ${console_?.nom || ''}`, 500), session_id: id, created_at: now.toISOString()
    })
  }

  const joueur: any = await queryOne('joueurs', j => j.id === s.joueur_id)
  const enrichedFacture = { ...facture, joueur_nom: joueur?.nom, lignes: (await queryAll('lignes_facture')).filter(l => l.facture_id === (facture as any).id) }

  res.json({ session: await queryOne('sessions_jeu', x => x.id === id), facture: enrichedFacture, montant, jetonsGagnes, dureeMinutes })
})

// ===== FACTURES =====
app.get('/api/factures', authMiddleware, async (req: any, res: any) => {
  const { statut, joueur_id, date_start, date_end } = req.query as Record<string, string>
  let factures: any[] = await queryAll('factures')
  if (statut) factures = factures.filter(f => f.statut === statut)
  if (joueur_id) factures = factures.filter(f => f.joueur_id === Number(joueur_id))
  if (date_start) factures = factures.filter(f => (f.created_at as string) >= date_start)
  if (date_end) factures = factures.filter(f => (f.created_at as string) <= date_end + 'T23:59:59')
  factures.sort((a: any, b: any) => new Date(b.created_at as string).getTime() - new Date(a.created_at as string).getTime())

  const joueurs: any[] = await queryAll('joueurs')
  res.json(factures.map(f => ({ ...f, joueur_nom: joueurs.find(j => j.id === f.joueur_id)?.nom || 'N/A' })))
})

app.get('/api/factures/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const f: any = await queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const joueur: any = await queryOne('joueurs', j => j.id === f.joueur_id)
  const lignes = (await queryAll('lignes_facture')).filter(l => l.facture_id === id)
  res.json({ ...f, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, lignes })
})

app.get('/api/factures/:id/pdf', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const f: any = await queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const joueur: any = await queryOne('joueurs', j => j.id === f.joueur_id)
  const lignes: any[] = (await queryAll('lignes_facture')).filter(l => l.facture_id === id)

  try {
    const { jsPDF } = await import('jspdf')
    const doc = new jsPDF()
    doc.setFontSize(22); (doc as any).setFont('helvetica', 'bold'); doc.text('GAME LOUNGE', 105, 18, { align: 'center' })
    doc.setFontSize(10); (doc as any).setFont('helvetica', 'normal'); doc.text('Facture de prestation', 105, 25, { align: 'center' })

    doc.setDrawColor(168, 85, 247); (doc as any).setLineWidth(0.5); doc.line(20, 30, 190, 30)

    doc.setFontSize(11); (doc as any).setFont('helvetica', 'bold')
    doc.text(`N° ${f.numero_facture}`, 20, 40)
    ;(doc as any).setFont('helvetica', 'normal'); doc.setFontSize(10)
    doc.text(`Date : ${new Date(f.created_at as string).toLocaleDateString('fr-FR')}`, 20, 48)
    doc.text(`Joueur : ${joueur?.nom || 'N/A'}`, 20, 56)
    doc.text(`Téléphone : ${joueur?.telephone || 'N/A'}`, 20, 64)

    doc.setFillColor(168, 85, 247); doc.rect(20, 74, 170, 8, 'F')
    doc.setTextColor(255, 255, 255); doc.setFontSize(9); (doc as any).setFont('helvetica', 'bold')
    doc.text('Désignation', 22, 80)
    doc.text('Qté', 120, 80); doc.text('P.U.', 135, 80); doc.text('Total', 165, 80)
    doc.setTextColor(0, 0, 0)

    let y = 90
    ;(doc as any).setFont('helvetica', 'normal'); doc.setFontSize(10)
    lignes.forEach(l => {
      doc.text(l.description as string, 22, y)
      doc.text(String(l.quantite), 120, y)
      doc.text(`${new Intl.NumberFormat('fr-FR').format(l.prix_unitaire as number)} FC`, 135, y)
      doc.text(`${new Intl.NumberFormat('fr-FR').format(l.total_ligne as number)} FC`, 165, y)
      y += 8
    })

    doc.setDrawColor(200, 200, 200); doc.line(120, y, 190, y); y += 8

    doc.setFontSize(10); (doc as any).setFont('helvetica', 'normal')
    doc.text(`Montant HT :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_ht as number)} FC`, 165, y); y += 7
    doc.text(`TVA (${f.taux_tva}%) :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_tva as number)} FC`, 165, y); y += 7

    doc.setDrawColor(168, 85, 247); (doc as any).setLineWidth(0.8); doc.line(120, y, 190, y); y += 8
    doc.setFontSize(14); (doc as any).setFont('helvetica', 'bold')
    doc.text(`TOTAL :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_ttc as number)} FC`, 165, y)

    doc.setDrawColor(168, 85, 247); (doc as any).setLineWidth(0.5); doc.line(20, 270, 190, 270)
    doc.setFontSize(8); (doc as any).setFont('helvetica', 'normal'); doc.setTextColor(128, 128, 128)
    doc.text('Game Lounge — Merci pour votre visite !', 105, 276, { align: 'center' })

    res.setHeader('Content-Type', 'application/pdf')
    res.setHeader('Content-Disposition', `attachment; filename="${f.numero_facture}.pdf"`)
    res.send(Buffer.from(doc.output('arraybuffer') as ArrayBuffer))
  } catch (err: any) {
    logError(err instanceof Error ? err : new Error(err.message || 'Erreur de génération PDF'), '/api/factures/:id/pdf', 'GET').catch(() => {})
    res.status(500).json({ message: 'Erreur génération PDF', error: err.message })
  }
})

app.put('/api/factures/:id/annuler', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const f: any = await queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  await update('factures', id, { statut: 'annulee' })
  const updated: any = await queryOne('factures', x => x.id === id)
  const joueur: any = await queryOne('joueurs', j => j.id === updated.joueur_id)
  res.json({ ...updated, joueur_nom: joueur?.nom })
})

// ===== JETONS =====
app.get('/api/jetons', authMiddleware, async (req: any, res: any) => {
  const { joueur_id } = req.query as Record<string, string>
  let transactions: any[] = await queryAll('jetons_transactions')
  if (joueur_id) transactions = transactions.filter(t => t.joueur_id === Number(joueur_id))
  transactions.sort((a: any, b: any) => new Date(b.created_at as string).getTime() - new Date(a.created_at as string).getTime())

  const joueurs: any[] = await queryAll('joueurs')
  res.json(transactions.map(t => ({ ...t, joueur_nom: joueurs.find(j => j.id === t.joueur_id)?.nom })))
})

// ===== RAPPORTS =====
app.get('/api/rapports/ca', authMiddleware, async (req: any, res: any) => {
  const factures: any[] = (await queryAll('factures')).filter(f => f.statut === 'payee')
  const sessions: any[] = await queryAll('sessions_jeu')
  const joueurs: any[] = await queryAll('joueurs')
  const jeux: any[] = await queryAll('jeux')
  const consoles: any[] = await queryAll('consoles')

  const today = new Date()
  const todayStr = today.toISOString().slice(0, 10)
  const todayFactures = factures.filter(f => (f.date_paiement as string)?.startsWith(todayStr))
  const todaySessions = sessions.filter(s => (s.created_at as string)?.startsWith(todayStr))

  const totalRevenus = factures.reduce((s, f) => s + (f.montant_ttc as number || 0), 0)
  const totalSessions = sessions.length

  const topJeux = jeux.map(j => {
    const count = sessions.filter(s => s.jeu_id === j.id).length
    return { titre: j.titre, sessions: count, pct: totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }
  }).sort((a, b) => b.sessions - a.sessions).slice(0, 5)

  const repartitionConsoles = consoles.map(c => {
    const count = sessions.filter(s => s.console_id === c.id).length
    return { nom: c.nom, sessions: count, pct: totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }
  }).sort((a, b) => b.sessions - a.sessions)

  const days7: { date: string; count: number; revenus: number }[] = []
  for (let i = 6; i >= 0; i--) {
    const d = new Date(today)
    d.setDate(d.getDate() - i)
    const ds = d.toISOString().slice(0, 10)
    days7.push({
      date: ds.slice(5),
      count: sessions.filter(s => (s.created_at as string)?.startsWith(ds)).length,
      revenus: factures.filter(f => (f.date_paiement as string)?.startsWith(ds)).reduce((s, f) => s + (f.montant_ttc as number || 0), 0)
    })
  }

  res.json({
    revenus_aujourd_hui: todayFactures.reduce((s, f) => s + (f.montant_ttc as number || 0), 0),
    sessions_aujourd_hui: todaySessions.length,
    joueurs_actifs: new Set(todaySessions.map(s => s.joueur_id)).size,
    jetons_attribues: (await queryAll('jetons_transactions')).filter(t => (t.created_at as string)?.startsWith(todayStr) && t.type === 'gain').reduce((s, t) => s + (t.quantite as number || 0), 0),
    total_revenus: totalRevenus,
    total_sessions: totalSessions,
    total_joueurs: joueurs.length,
    top_jeux: topJeux,
    repartition_consoles: repartitionConsoles,
    sessions_history: days7,
  })
})

// ===== USERS (admin) =====
app.get('/api/users', authMiddleware, adminOnly, async (req: any, res: any) => {
  const users: any[] = (await queryAll('users')).map(u => ({ id: u.id, email: u.email, role: u.role, nom: u.nom, created_at: u.created_at }))
  res.json(users)
})

app.post('/api/users', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { email, password, role, nom } = req.body
  if (!email || !password || !role || !nom) return res.status(400).json({ message: 'Nom, email, mot de passe et rôle requis' })
  if (!isValidNom(nom)) return res.status(400).json({ message: 'Nom invalide (2-50 caractères)' })
  if (!isValidEmail(email)) return res.status(400).json({ message: 'Email invalide' })
  if (!isValidPassword(password)) return res.status(400).json({ message: 'Mot de passe invalide (min 6 caractères, au moins une lettre)' })
  if (!isValidRole(role)) return res.status(400).json({ message: 'Rôle invalide' })
  if (await queryOne('users', u => u.email === email)) return res.status(400).json({ message: 'Email déjà utilisé' })
  const sanitizedNom = sanitizeInput(nom, 50)
  const sanitizedEmail = sanitizeInput(email, 100)
  const password_hash = bcrypt.hashSync(password, 10)
  const user: any = await insert('users', { email: sanitizedEmail, password_hash, role, nom: sanitizedNom, created_at: new Date().toISOString() })
  res.status(201).json({ id: user.id, email: user.email, role: user.role, nom: user.nom, created_at: user.created_at })
})

app.delete('/api/users/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const user: any = await queryOne('users', u => u.id === id)
  if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
  if (user.id === req.user.id) return res.status(400).json({ message: 'Impossible de supprimer votre propre compte' })
  await remove('users', id)
  res.json({ success: true })
})

app.get('/api/users/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const user: any = await queryOne('users', u => u.id === id)
  if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
  res.json({ id: user.id, email: user.email, role: user.role, nom: user.nom, created_at: user.created_at })
})

app.put('/api/users/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const { email, role, nom, password } = req.body
  const user: any = await queryOne('users', u => u.id === id)
  if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
  const updates: Record<string, unknown> = {}
  if (email !== undefined) {
    if (!isValidEmail(email)) return res.status(400).json({ message: 'Email invalide' })
    updates.email = sanitizeInput(email, 100)
  }
  if (role !== undefined) {
    if (!isValidRole(role)) return res.status(400).json({ message: 'Rôle invalide' })
    updates.role = role
  }
  if (nom !== undefined) {
    if (!isValidNom(nom)) return res.status(400).json({ message: 'Nom invalide (2-50 caractères)' })
    updates.nom = sanitizeInput(nom, 50)
  }
  if (password !== undefined && password) {
    if (!isValidPassword(password)) return res.status(400).json({ message: 'Mot de passe invalide (min 6 caractères, au moins une lettre)' })
    updates.password_hash = bcrypt.hashSync(password, 10)
  }
  const updated: any = await update('users', id, updates)
  res.json({ id: updated.id, email: updated.email, role: updated.role, nom: updated.nom, created_at: updated.created_at })
})

// ===== CONSOLES SINGLE =====
app.get('/api/consoles/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const c: any = await queryOne('consoles', x => x.id === id)
  if (!c) return res.status(404).json({ message: 'Console non trouvée' })
  res.json(c)
})

// ===== JEUX SINGLE =====
app.get('/api/jeux/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const j: any = await queryOne('jeux', x => x.id === id)
  if (!j) return res.status(404).json({ message: 'Jeu non trouvé' })
  res.json(j)
})

// ===== JOUEURS SINGLE & DELETE =====
app.get('/api/joueurs/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const j: any = await queryOne('joueurs', x => x.id === id)
  if (!j) return res.status(404).json({ message: 'Joueur non trouvé' })
  res.json(j)
})

app.delete('/api/joueurs/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const j: any = await queryOne('joueurs', x => x.id === id)
  if (!j) return res.status(404).json({ message: 'Joueur non trouvé' })
  await remove('joueurs', id)
  res.json({ success: true })
})

// ===== TARIFS SINGLE =====
app.get('/api/tarifs/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const t: any = await queryOne('tarifs', x => x.id === id)
  if (!t) return res.status(404).json({ message: 'Tarif non trouvé' })
  res.json(t)
})

// ===== SESSIONS UPDATE & DELETE =====
app.put('/api/sessions/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const s: any = await queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  const { console_id, joueur_id, jeu_id, statut, duree_minutes, montant } = req.body
  const updates: Record<string, unknown> = {}
  if (console_id !== undefined) {
    if (!isValidId(console_id)) return res.status(400).json({ message: 'Console ID invalide' })
    updates.console_id = Number(console_id)
  }
  if (joueur_id !== undefined) {
    if (!isValidId(joueur_id)) return res.status(400).json({ message: 'Joueur ID invalide' })
    updates.joueur_id = Number(joueur_id)
  }
  if (jeu_id !== undefined) {
    if (!isValidId(jeu_id)) return res.status(400).json({ message: 'Jeu ID invalide' })
    updates.jeu_id = Number(jeu_id)
  }
  if (statut !== undefined) {
    if (!isValidSessionStatut(statut)) return res.status(400).json({ message: 'Statut invalide' })
    updates.statut = statut
  }
  if (duree_minutes !== undefined) {
    if (!isValidDuree(duree_minutes)) return res.status(400).json({ message: 'Durée invalide (1-1000)' })
    updates.duree_minutes = Number(duree_minutes)
  }
  if (montant !== undefined) {
    if (!isValidPrix(montant)) return res.status(400).json({ message: 'Montant invalide (1-1000000)' })
    updates.montant = Number(montant)
  }
  const updated = await update('sessions_jeu', id, updates)
  res.json(updated)
})

app.delete('/api/sessions/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const s: any = await queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  await remove('sessions_jeu', id)
  res.json({ success: true })
})

// ===== FACTURES CRUD =====
app.post('/api/factures', authMiddleware, async (req: any, res: any) => {
  const { session_id, joueur_id, montant_ht, taux_tva, montant_tva, montant_ttc, mode_paiement, statut } = req.body
  if (!session_id || !joueur_id || montant_ttc === undefined) return res.status(400).json({ message: 'Champs requis manquants' })
  if (!isValidId(session_id)) return res.status(400).json({ message: 'Session ID invalide' })
  if (!isValidId(joueur_id)) return res.status(400).json({ message: 'Joueur ID invalide' })
  if (!isValidPrix(montant_ttc)) return res.status(400).json({ message: 'Montant TTC invalide (1-1000000)' })
  if (montant_ht !== undefined && montant_ht !== null && montant_ht !== '' && !isValidPrix(montant_ht) && Number(montant_ht) !== 0) return res.status(400).json({ message: 'Montant HT invalide' })
  if (statut && !isValidFactureStatut(statut)) return res.status(400).json({ message: 'Statut invalide' })
  if (mode_paiement && !isValidModePaiement(mode_paiement)) return res.status(400).json({ message: 'Mode paiement invalide' })
  const now = new Date()
  const dateStr = now.toISOString().slice(0, 10).replace(/-/g, '')
  const random = String(Math.floor(Math.random() * 9999)).padStart(4, '0')
  const numero_facture = `FAC-${dateStr}-${random}`
  const f: any = await insert('factures', {
    numero_facture, session_id: Number(session_id), joueur_id: Number(joueur_id),
    montant_ht: Number(montant_ht) || 0, taux_tva: Number(taux_tva) || 20, montant_tva: Number(montant_tva) || 0, montant_ttc: Number(montant_ttc),
    mode_paiement: mode_paiement || 'especes', statut: statut || 'payee', date_paiement: now.toISOString(), created_at: now.toISOString()
  })
  res.status(201).json(f)
})

app.put('/api/factures/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const f: any = await queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const { statut, mode_paiement, montant_ttc } = req.body
  const updates: Record<string, unknown> = {}
  if (statut !== undefined) {
    if (!isValidFactureStatut(statut)) return res.status(400).json({ message: 'Statut invalide' })
    updates.statut = statut
  }
  if (mode_paiement !== undefined) {
    if (!isValidModePaiement(mode_paiement)) return res.status(400).json({ message: 'Mode paiement invalide' })
    updates.mode_paiement = mode_paiement
  }
  if (montant_ttc !== undefined) {
    if (!isValidPrix(montant_ttc)) return res.status(400).json({ message: 'Montant invalide (1-1000000)' })
    updates.montant_ttc = Number(montant_ttc)
  }
  const updated = await update('factures', id, updates)
  res.json(updated)
})

app.delete('/api/factures/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const f: any = await queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  await remove('factures', id)
  res.json({ success: true })
})

// ===== LIGNES FACTURE CRUD =====
app.get('/api/lignes_facture', authMiddleware, async (req: any, res: any) => {
  const { facture_id } = req.query as Record<string, string>
  let lignes: any[] = await queryAll('lignes_facture')
  if (facture_id) lignes = lignes.filter(l => l.facture_id === Number(facture_id))
  res.json(lignes)
})

app.get('/api/lignes_facture/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const l: any = await queryOne('lignes_facture', x => x.id === id)
  if (!l) return res.status(404).json({ message: 'Ligne non trouvée' })
  res.json(l)
})

app.post('/api/lignes_facture', authMiddleware, async (req: any, res: any) => {
  const { facture_id, description, quantite, prix_unitaire, total_ligne } = req.body
  if (!facture_id || !description || quantite === undefined || prix_unitaire === undefined) return res.status(400).json({ message: 'Champs requis manquants' })
  if (!isValidId(facture_id)) return res.status(400).json({ message: 'Facture ID invalide' })
  if (!sanitizeInput(String(description), 500) || String(description).trim().length < 2) return res.status(400).json({ message: 'Description invalide (2-500 caractères)' })
  if (!isValidQuantite(quantite)) return res.status(400).json({ message: 'Quantité invalide (1-10000)' })
  if (!isValidPrix(prix_unitaire)) return res.status(400).json({ message: 'Prix unitaire invalide (1-1000000)' })
  if (total_ligne !== undefined && total_ligne !== null && total_ligne !== '' && !isValidPrix(total_ligne)) return res.status(400).json({ message: 'Total invalide' })
  const sanitizedDesc = sanitizeInput(String(description), 500)
  const l: any = await insert('lignes_facture', { facture_id: Number(facture_id), description: sanitizedDesc, quantite: Number(quantite), prix_unitaire: Number(prix_unitaire), total_ligne: Number(total_ligne) || Number(quantite) * Number(prix_unitaire) })
  res.status(201).json(l)
})

app.put('/api/lignes_facture/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const l: any = await queryOne('lignes_facture', x => x.id === id)
  if (!l) return res.status(404).json({ message: 'Ligne non trouvée' })
  const { description, quantite, prix_unitaire, total_ligne } = req.body
  const updates: Record<string, unknown> = {}
  if (description !== undefined) {
    if (!sanitizeInput(String(description), 500) || String(description).trim().length < 2) return res.status(400).json({ message: 'Description invalide (2-500 caractères)' })
    updates.description = sanitizeInput(String(description), 500)
  }
  if (quantite !== undefined) {
    if (!isValidQuantite(quantite)) return res.status(400).json({ message: 'Quantité invalide (1-10000)' })
    updates.quantite = Number(quantite)
  }
  if (prix_unitaire !== undefined) {
    if (!isValidPrix(prix_unitaire)) return res.status(400).json({ message: 'Prix unitaire invalide (1-1000000)' })
    updates.prix_unitaire = Number(prix_unitaire)
  }
  if (total_ligne !== undefined) {
    if (!isValidPrix(total_ligne)) return res.status(400).json({ message: 'Total invalide' })
    updates.total_ligne = Number(total_ligne)
  }
  const updated = await update('lignes_facture', id, updates)
  res.json(updated)
})

app.delete('/api/lignes_facture/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const l: any = await queryOne('lignes_facture', x => x.id === id)
  if (!l) return res.status(404).json({ message: 'Ligne non trouvée' })
  await remove('lignes_facture', id)
  res.json({ success: true })
})

// ===== JETONS CRUD =====
app.post('/api/jetons', authMiddleware, async (req: any, res: any) => {
  const { joueur_id, type, quantite, raison, session_id } = req.body
  if (!joueur_id || !type || quantite === undefined) return res.status(400).json({ message: 'Champs requis manquants' })
  if (!isValidId(joueur_id)) return res.status(400).json({ message: 'Joueur ID invalide' })
  if (!isValidJetonType(type)) return res.status(400).json({ message: 'Type invalide' })
  if (!isValidQuantite(quantite)) return res.status(400).json({ message: 'Quantité invalide (1-10000)' })
  if (session_id !== undefined && session_id !== null && session_id !== '' && !isValidId(session_id)) return res.status(400).json({ message: 'Session ID invalide' })
  const sanitizedRaison = raison ? sanitizeInput(String(raison), 500) : ''
  const t: any = await insert('jetons_transactions', { joueur_id: Number(joueur_id), type, quantite: Number(quantite), raison: sanitizedRaison, session_id: session_id ? Number(session_id) : null, created_at: new Date().toISOString() })
  if (type === 'gain' || type === 'bonus') {
    const joueur: any = await queryOne('joueurs', j => j.id === Number(joueur_id))
    if (joueur) await update('joueurs', Number(joueur_id), { jetons_solde: (joueur.jetons_solde || 0) + Number(quantite) })
  } else if (type === 'depense') {
    const joueur: any = await queryOne('joueurs', j => j.id === Number(joueur_id))
    if (joueur) await update('joueurs', Number(joueur_id), { jetons_solde: Math.max(0, (joueur.jetons_solde || 0) - Number(quantite)) })
  }
  res.status(201).json(t)
})

app.get('/api/jetons/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const t: any = await queryOne('jetons_transactions', x => x.id === id)
  if (!t) return res.status(404).json({ message: 'Transaction non trouvée' })
  res.json(t)
})

app.put('/api/jetons/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const t: any = await queryOne('jetons_transactions', x => x.id === id)
  if (!t) return res.status(404).json({ message: 'Transaction non trouvée' })
  const { type, quantite, raison } = req.body
  const updates: Record<string, unknown> = {}
  if (type !== undefined) {
    if (!isValidJetonType(type)) return res.status(400).json({ message: 'Type invalide' })
    updates.type = type
  }
  if (quantite !== undefined) {
    if (!isValidQuantite(quantite)) return res.status(400).json({ message: 'Quantité invalide (1-10000)' })
    updates.quantite = Number(quantite)
  }
  if (raison !== undefined) updates.raison = sanitizeInput(String(raison), 500)
  const updated = await update('jetons_transactions', id, updates)
  res.json(updated)
})

app.delete('/api/jetons/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const t: any = await queryOne('jetons_transactions', x => x.id === id)
  if (!t) return res.status(404).json({ message: 'Transaction non trouvée' })
  await remove('jetons_transactions', id)
  res.json({ success: true })
})

// ===== PARAMETRES FIDELITE CRUD =====
app.get('/api/parametres/fidelite/:id', authMiddleware, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const p: any = await queryOne('parametres_fidelite', x => x.id === id)
  if (!p) return res.status(404).json({ message: 'Paramètre non trouvé' })
  res.json(p)
})

app.post('/api/parametres/fidelite', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { regle_type, seuil, jetons_attribues, actif } = req.body
  if (!regle_type || seuil === undefined || jetons_attribues === undefined) return res.status(400).json({ message: 'Champs requis manquants' })
  if (!isValidRegleType(regle_type)) return res.status(400).json({ message: 'Type de règle invalide' })
  if (!isValidSeuil(seuil)) return res.status(400).json({ message: 'Seuil invalide (1-10000)' })
  if (!isValidJetonsAttribues(jetons_attribues)) return res.status(400).json({ message: 'Jetons attribués invalides (1-1000)' })
  const p: any = await insert('parametres_fidelite', { regle_type, seuil: Number(seuil), jetons_attribues: Number(jetons_attribues), actif: actif !== false ? 1 : 0 })
  res.status(201).json(p)
})

app.delete('/api/parametres/fidelite/:id', authMiddleware, adminOnly, async (req: any, res: any) => {
  const id = Number(req.params.id)
  if (!isValidId(id)) return res.status(400).json({ message: 'ID invalide' })
  const p: any = await queryOne('parametres_fidelite', x => x.id === id)
  if (!p) return res.status(404).json({ message: 'Paramètre non trouvé' })
  await remove('parametres_fidelite', id)
  res.json({ success: true })
})

// ===== SYNC =====
app.get('/api/sync/status', authMiddleware, adminOnly, async (req: any, res: any) => {
  res.json({ ...getSyncStatus(), neonEnabled: isNeonEnabled() })
})

app.post('/api/sync/toggle', authMiddleware, adminOnly, async (req: any, res: any) => {
  const { enabled } = req.body
  setSyncEnabled(!!enabled)
  res.json({ enabled: !!enabled, ...getSyncStatus() })
})

app.post('/api/sync/run', authMiddleware, adminOnly, async (req: any, res: any) => {
  try {
    const result = await runFullSync()
    res.json({ success: true, ...result })
  } catch (e: any) {
    res.status(400).json({ message: e.message })
  }
})

// ===== STATIC FILES =====
// All SQL queries use parameterized queries via db.ts (using ? and $1 placeholders) to prevent SQL injection
const distPaths = [
  join(__dirname, 'public'),
  join(__dirname, '../dist'),
  join(process.env.DATA_DIR || __dirname, '../dist'),
  join(process.env.DATA_DIR || __dirname, 'public'),
]
const distDir = distPaths.find(p => existsSync(p)) || distPaths[0]

app.use(express.static(distDir, {
  setHeaders: (res, path) => {
    if (path.endsWith('.html')) {
      res.setHeader('Cache-Control', 'no-cache, no-store, must-revalidate')
      res.setHeader('Pragma', 'no-cache')
      res.setHeader('Expires', '0')
    } else {
      res.setHeader('Cache-Control', 'no-cache, no-store, must-revalidate')
    }
  }
}))
app.get('*', async (req: any, res: any) => {
  if (!req.path.startsWith('/api')) {
    res.sendFile(join(distDir, 'index.html'))
  }
})

// ===== ERROR HANDLER =====
app.use((err: any, req: any, res: any, _next: any) => {
  console.error('Erreur serveur:', err)
  logError(err instanceof Error ? err : new Error(String(err)), req.originalUrl, req.method).catch(() => {})
  res.status(500).json({ message: 'Erreur interne du serveur' })
})

app.listen(PORT, '0.0.0.0', () => {
  console.log(`🎮 Game Lounge API running on http://0.0.0.0:${PORT}`)
})
