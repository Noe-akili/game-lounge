import express from 'express'
import cors from 'cors'
import { fileURLToPath } from 'url'
import { dirname, join } from 'path'
import { queryAll, queryOne, insert, update, remove } from './db.js'
import bcrypt from 'bcryptjs'
import jwt from 'jsonwebtoken'

const __dirname = dirname(fileURLToPath(import.meta.url))
const JWT_SECRET = process.env.JWT_SECRET || 'game-lounge-secret-2024'
const app = express()
const PORT = process.env.PORT || 3001

app.use(cors())
app.use(express.json())

function authMiddleware(req, res, next) {
  const header = req.headers.authorization
  if (!header || !header.startsWith('Bearer ')) return res.status(401).json({ message: 'Token manquant' })
  try {
    req.user = jwt.verify(header.split(' ')[1], JWT_SECRET)
    next()
  } catch { return res.status(401).json({ message: 'Token invalide' }) }
}

function adminOnly(req, res, next) {
  if (req.user?.role !== 'admin') return res.status(403).json({ message: 'Accès réservé aux administrateurs' })
  next()
}

// ===== AUTH =====
app.post('/api/auth/login', (req, res) => {
  const { email, password } = req.body
  if (!email || !password) return res.status(400).json({ message: 'Email et mot de passe requis' })
  const user = queryOne('users', u => u.email === email)
  if (!user || !bcrypt.compareSync(password, user.password_hash)) return res.status(401).json({ message: 'Identifiants incorrects' })
  const token = jwt.sign({ id: user.id, email: user.email, role: user.role, nom: user.nom }, JWT_SECRET, { expiresIn: '24h' })
  res.json({ token, user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
})

app.get('/api/auth/me', authMiddleware, (req, res) => {
  const user = queryOne('users', u => u.id === req.user.id)
  if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
  res.json({ user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
})

// ===== CONSOLES =====
app.get('/api/consoles', authMiddleware, (req, res) => {
  const consoles = queryAll('consoles')
  const sessions = queryAll('sessions_jeu').filter(s => s.statut === 'en_cours' || s.statut === 'pause')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')

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
  }).sort((a, b) => a.poste_numero - b.poste_numero)
  res.json(result)
})

app.post('/api/consoles', authMiddleware, adminOnly, (req, res) => {
  const { nom, type, poste_numero, etat } = req.body
  if (!nom || !type || !poste_numero) return res.status(400).json({ message: 'Champs requis manquants' })
  const c = insert('consoles', { nom, type, poste_numero: Number(poste_numero), etat: etat || 'disponible', created_at: new Date().toISOString() })
  res.status(201).json(c)
})

app.put('/api/consoles/:id', authMiddleware, adminOnly, (req, res) => {
  const id = Number(req.params.id)
  const { nom, type, poste_numero, etat } = req.body
  const c = update('consoles', id, { nom, type, poste_numero: Number(poste_numero), etat })
  if (!c) return res.status(404).json({ message: 'Console non trouvée' })
  res.json(c)
})

app.delete('/api/consoles/:id', authMiddleware, adminOnly, (req, res) => {
  remove('consoles', Number(req.params.id))
  res.json({ success: true })
})

// ===== JEUX =====
app.get('/api/jeux', authMiddleware, (req, res) => {
  const { console_id } = req.query
  let jeux = queryAll('jeux').filter(j => j.actif !== false)
  if (console_id) jeux = jeux.filter(j => j.console_id === Number(console_id))
  res.json(jeux)
})

app.post('/api/jeux', authMiddleware, adminOnly, (req, res) => {
  const { titre, genre, console_id, jaquette_url } = req.body
  if (!titre) return res.status(400).json({ message: 'Titre requis' })
  const j = insert('jeux', { titre, genre, console_id: Number(console_id), jaquette_url, actif: true, created_at: new Date().toISOString() })
  res.status(201).json(j)
})

app.put('/api/jeux/:id', authMiddleware, adminOnly, (req, res) => {
  const id = Number(req.params.id)
  const { titre, genre, console_id, jaquette_url } = req.body
  const j = update('jeux', id, { titre, genre, console_id: Number(console_id), jaquette_url })
  if (!j) return res.status(404).json({ message: 'Jeu non trouvé' })
  res.json(j)
})

app.delete('/api/jeux/:id', authMiddleware, adminOnly, (req, res) => {
  remove('jeux', Number(req.params.id))
  res.json({ success: true })
})

// ===== JOUEURS =====
app.get('/api/joueurs', authMiddleware, (req, res) => {
  const { search } = req.query
  let joueurs = queryAll('joueurs')
  if (search) {
    const q = search.toLowerCase()
    joueurs = joueurs.filter(j => j.nom?.toLowerCase().includes(q) || j.telephone?.includes(q) || j.email?.toLowerCase().includes(q))
  }
  res.json(joueurs)
})

app.post('/api/joueurs', authMiddleware, (req, res) => {
  const { nom, telephone, email } = req.body
  if (!nom) return res.status(400).json({ message: 'Nom requis' })
  const j = insert('joueurs', { nom, telephone: telephone || '', email: email || '', jetons_solde: 0, date_inscription: new Date().toISOString(), derniere_visite: null })
  res.status(201).json(j)
})

app.put('/api/joueurs/:id', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const { nom, telephone, email, jetons_solde } = req.body
  const updates = {}
  if (nom !== undefined) updates.nom = nom
  if (telephone !== undefined) updates.telephone = telephone
  if (email !== undefined) updates.email = email
  if (jetons_solde !== undefined) updates.jetons_solde = Number(jetons_solde)
  const j = update('joueurs', id, updates)
  if (!j) return res.status(404).json({ message: 'Joueur non trouvé' })
  res.json(j)
})

app.get('/api/joueurs/:id/historique', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const joueur = queryOne('joueurs', j => j.id === id)
  if (!joueur) return res.status(404).json({ message: 'Joueur non trouvé' })

  const sessions = queryAll('sessions_jeu').filter(s => s.joueur_id === id).sort((a, b) => new Date(b.created_at) - new Date(a.created_at))
  const transactions = queryAll('jetons_transactions').filter(t => t.joueur_id === id).sort((a, b) => new Date(b.created_at) - new Date(a.created_at))
  const factures = queryAll('factures').filter(f => f.joueur_id === id).sort((a, b) => new Date(b.created_at) - new Date(a.created_at))

  const consoles = queryAll('consoles')
  const jeux = queryAll('jeux')

  const enrichedSessions = sessions.map(s => ({
    ...s,
    console_nom: consoles.find(c => c.id === s.console_id)?.nom,
    jeu_nom: jeux.find(j => j.id === s.jeu_id)?.titre,
  }))

  res.json({ joueur, sessions: enrichedSessions, transactions, factures })
})

// ===== TARIFS =====
app.get('/api/tarifs', authMiddleware, (req, res) => {
  res.json(queryAll('tarifs'))
})

app.post('/api/tarifs', authMiddleware, adminOnly, (req, res) => {
  const { type, duree_minutes, prix, description } = req.body
  if (!type || !duree_minutes || !prix) return res.status(400).json({ message: 'Champs requis manquants' })
  const t = insert('tarifs', { type, duree_minutes: Number(duree_minutes), prix: Number(prix), description: description || '', actif: true })
  res.status(201).json(t)
})

app.put('/api/tarifs/:id', authMiddleware, adminOnly, (req, res) => {
  const id = Number(req.params.id)
  const { type, duree_minutes, prix, description, actif } = req.body
  const t = update('tarifs', id, { type, duree_minutes: Number(duree_minutes), prix: Number(prix), description, actif })
  if (!t) return res.status(404).json({ message: 'Tarif non trouvé' })
  res.json(t)
})

app.delete('/api/tarifs/:id', authMiddleware, adminOnly, (req, res) => {
  remove('tarifs', Number(req.params.id))
  res.json({ success: true })
})

app.get('/api/parametres/fidelite', authMiddleware, (req, res) => {
  const regle = queryOne('parametres_fidelite', r => r.actif !== false)
  res.json(regle || { id: 0, regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
})

app.put('/api/parametres/fidelite', authMiddleware, adminOnly, (req, res) => {
  const { regle_type, seuil, jetons_attribues, actif } = req.body
  const existing = queryOne('parametres_fidelite', () => true)
  if (existing) {
    update('parametres_fidelite', existing.id, { regle_type, seuil: Number(seuil), jetons_attribues: Number(jetons_attribues), actif: !!actif })
  } else {
    insert('parametres_fidelite', { regle_type, seuil: Number(seuil), jetons_attribues: Number(jetons_attribues), actif: !!actif })
  }
  res.json(queryOne('parametres_fidelite', () => true))
})

// ===== SESSIONS =====
app.get('/api/sessions', authMiddleware, (req, res) => {
  const { statut } = req.query
  let sessions = queryAll('sessions_jeu')
  if (statut) sessions = sessions.filter(s => s.statut === statut)
  sessions.sort((a, b) => new Date(b.created_at) - new Date(a.created_at))

  const consoles = queryAll('consoles')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')
  const users = queryAll('users')

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

app.get('/api/sessions/:id', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const s = queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  const console_ = queryOne('consoles', c => c.id === s.console_id)
  const joueur = queryOne('joueurs', j => j.id === s.joueur_id)
  const jeu = queryOne('jeux', j => j.id === s.jeu_id)
  const dureeSecondes = (s.statut === 'en_cours')
    ? Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000)
    : (s.statut === 'pause' ? s.duree_minutes * 60 : Math.floor((new Date(s.fin).getTime() - new Date(s.debut).getTime()) / 1000))
  res.json({ ...s, console_nom: console_?.nom, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, jeu_nom: jeu?.titre, duree_secondes: Math.max(0, dureeSecondes) })
})

app.post('/api/sessions', authMiddleware, (req, res) => {
  const { console_id, joueur_id, jeu_id } = req.body
  if (!console_id || !joueur_id || !jeu_id) return res.status(400).json({ message: 'Console, joueur et jeu requis' })

  const existingSession = queryOne('sessions_jeu', s => s.console_id === Number(console_id) && (s.statut === 'en_cours' || s.statut === 'pause'))
  if (existingSession) return res.status(400).json({ message: 'Cette console a déjà une session en cours' })

  const tarifs = queryAll('tarifs')
  const tarif = tarifs.find(t => t.type === 'horaire' && t.actif !== false)
  const tarif_prix = tarif?.prix || 2000

  const session = insert('sessions_jeu', {
    console_id: Number(console_id), joueur_id: Number(joueur_id), jeu_id: Number(jeu_id),
    employe_id: req.user.id, debut: new Date().toISOString(), fin: null,
    duree_minutes: 0, montant: 0, tarif_prix, jetons_gagnes: 0,
    statut: 'en_cours', created_at: new Date().toISOString()
  })
  update('consoles', Number(console_id), { etat: 'occupee' })
  update('joueurs', Number(joueur_id), { derniere_visite: new Date().toISOString() })

  const consoles = queryAll('consoles')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')

  res.status(201).json({
    ...session,
    console_nom: consoles.find(c => c.id === session.console_id)?.nom,
    joueur_nom: joueurs.find(j => j.id === session.joueur_id)?.nom,
    jeu_nom: jeux.find(j => j.id === session.jeu_id)?.titre,
  })
})

app.put('/api/sessions/:id/pause', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const s = queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  if (s.statut !== 'en_cours') return res.status(400).json({ message: 'Session non en cours' })

  const elapsed = Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000)
  const totalDuree = (s.duree_minutes || 0) * 60 + elapsed

  update('sessions_jeu', id, { statut: 'pause', duree_minutes: Math.floor(totalDuree / 60) })
  update('consoles', s.console_id, { etat: 'pause' })

  const updated = queryOne('sessions_jeu', x => x.id === id)
  res.json(updated)
})

app.put('/api/sessions/:id/reprendre', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const s = queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  if (s.statut !== 'pause') return res.status(400).json({ message: 'Session non en pause' })

  update('sessions_jeu', id, { statut: 'en_cours', debut: new Date().toISOString() })
  update('consoles', s.console_id, { etat: 'occupee' })

  const updated = queryOne('sessions_jeu', x => x.id === id)
  res.json(updated)
})

app.put('/api/sessions/:id/terminer', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const s = queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })

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

  const console_ = queryOne('consoles', c => c.id === s.console_id)
  const jeu = queryOne('jeux', j => j.id === s.jeu_id)
  insert('lignes_facture', {
    facture_id: facture.id,
    description: `Session ${console_?.nom || ''} - ${jeu?.titre || ''} - ${dureeMinutes}min`,
    quantite: 1, prix_unitaire: montant, total_ligne: montant
  })

  const regle = queryOne('parametres_fidelite', r => r.actif !== false)
  let jetonsGagnes = 0
  if (regle?.regle_type === 'temps') {
    jetonsGagnes = Math.floor(dureeMinutes / (regle.seuil || 60)) * (regle.jetons_attribues || 1)
  }
  if (jetonsGagnes > 0) {
    const joueur = queryOne('joueurs', j => j.id === s.joueur_id)
    update('joueurs', s.joueur_id, { jetons_solde: (joueur?.jetons_solde || 0) + jetonsGagnes })
    insert('jetons_transactions', {
      joueur_id: s.joueur_id, type: 'gain', quantite: jetonsGagnes,
      raison: `Session ${dureeMinutes}min - ${console_?.nom || ''}`, session_id: id, created_at: now.toISOString()
    })
  }

  const joueur = queryOne('joueurs', j => j.id === s.joueur_id)
  const enrichedFacture = { ...facture, joueur_nom: joueur?.nom, lignes: queryAll('lignes_facture').filter(l => l.facture_id === facture.id) }

  res.json({ session: queryOne('sessions_jeu', x => x.id === id), facture: enrichedFacture, montant, jetonsGagnes, dureeMinutes })
})

// ===== FACTURES =====
app.get('/api/factures', authMiddleware, (req, res) => {
  const { statut, joueur_id, date_start, date_end } = req.query
  let factures = queryAll('factures')
  if (statut) factures = factures.filter(f => f.statut === statut)
  if (joueur_id) factures = factures.filter(f => f.joueur_id === Number(joueur_id))
  if (date_start) factures = factures.filter(f => f.created_at >= date_start)
  if (date_end) factures = factures.filter(f => f.created_at <= date_end + 'T23:59:59')
  factures.sort((a, b) => new Date(b.created_at) - new Date(a.created_at))

  const joueurs = queryAll('joueurs')
  res.json(factures.map(f => ({ ...f, joueur_nom: joueurs.find(j => j.id === f.joueur_id)?.nom || 'N/A' })))
})

app.get('/api/factures/:id', authMiddleware, (req, res) => {
  const id = Number(req.params.id)
  const f = queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const joueur = queryOne('joueurs', j => j.id === f.joueur_id)
  const lignes = queryAll('lignes_facture').filter(l => l.facture_id === id)
  res.json({ ...f, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, lignes })
})

app.get('/api/factures/:id/pdf', authMiddleware, async (req, res) => {
  const id = Number(req.params.id)
  const f = queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const joueur = queryOne('joueurs', j => j.id === f.joueur_id)
  const lignes = queryAll('lignes_facture').filter(l => l.facture_id === id)

  try {
    const { jsPDF } = await import('jspdf')
    const doc = new jsPDF()
    doc.setFontSize(22); doc.setFont('helvetica', 'bold'); doc.text('GAME LOUNGE', 105, 18, { align: 'center' })
    doc.setFontSize(10); doc.setFont('helvetica', 'normal'); doc.text('Facture de prestation', 105, 25, { align: 'center' })

    doc.setDrawColor(168, 85, 247); doc.setLineWidth(0.5); doc.line(20, 30, 190, 30)

    doc.setFontSize(11); doc.setFont('helvetica', 'bold')
    doc.text(`N° ${f.numero_facture}`, 20, 40)
    doc.setFont('helvetica', 'normal'); doc.setFontSize(10)
    doc.text(`Date : ${new Date(f.created_at).toLocaleDateString('fr-FR')}`, 20, 48)
    doc.text(`Joueur : ${joueur?.nom || 'N/A'}`, 20, 56)
    doc.text(`Téléphone : ${joueur?.telephone || 'N/A'}`, 20, 64)

    doc.setFillColor(168, 85, 247); doc.rect(20, 74, 170, 8, 'F')
    doc.setTextColor(255, 255, 255); doc.setFontSize(9); doc.setFont('helvetica', 'bold')
    doc.text('Désignation', 22, 80)
    doc.text('Qté', 120, 80); doc.text('P.U.', 135, 80); doc.text('Total', 165, 80)
    doc.setTextColor(0, 0, 0)

    let y = 90
    doc.setFont('helvetica', 'normal'); doc.setFontSize(10)
    lignes.forEach(l => {
      doc.text(l.description, 22, y)
      doc.text(String(l.quantite), 120, y)
      doc.text(`${new Intl.NumberFormat('fr-FR').format(l.prix_unitaire)} FC`, 135, y)
      doc.text(`${new Intl.NumberFormat('fr-FR').format(l.total_ligne)} FC`, 165, y)
      y += 8
    })

    doc.setDrawColor(200, 200, 200); doc.line(120, y, 190, y); y += 8

    doc.setFontSize(10); doc.setFont('helvetica', 'normal')
    doc.text(`Montant HT :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_ht)} FC`, 165, y); y += 7
    doc.text(`TVA (${f.taux_tva}%) :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_tva)} FC`, 165, y); y += 7

    doc.setDrawColor(168, 85, 247); doc.setLineWidth(0.8); doc.line(120, y, 190, y); y += 8
    doc.setFontSize(14); doc.setFont('helvetica', 'bold')
    doc.text(`TOTAL :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_ttc)} FC`, 165, y)

    doc.setDrawColor(168, 85, 247); doc.setLineWidth(0.5); doc.line(20, 270, 190, 270)
    doc.setFontSize(8); doc.setFont('helvetica', 'normal'); doc.setTextColor(128, 128, 128)
    doc.text('Game Lounge — Merci pour votre visite !', 105, 276, { align: 'center' })

    res.setHeader('Content-Type', 'application/pdf')
    res.setHeader('Content-Disposition', `attachment; filename="${f.numero_facture}.pdf"`)
    res.send(Buffer.from(doc.output('arraybuffer')))
  } catch (err) {
    res.status(500).json({ message: 'Erreur génération PDF', error: err.message })
  }
})

app.put('/api/factures/:id/annuler', authMiddleware, adminOnly, (req, res) => {
  const id = Number(req.params.id)
  const f = queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  update('factures', id, { statut: 'annulee' })
  const updated = queryOne('factures', x => x.id === id)
  const joueur = queryOne('joueurs', j => j.id === updated.joueur_id)
  res.json({ ...updated, joueur_nom: joueur?.nom })
})

// ===== JETONS =====
app.get('/api/jetons', authMiddleware, (req, res) => {
  const { joueur_id } = req.query
  let transactions = queryAll('jetons_transactions')
  if (joueur_id) transactions = transactions.filter(t => t.joueur_id === Number(joueur_id))
  transactions.sort((a, b) => new Date(b.created_at) - new Date(a.created_at))

  const joueurs = queryAll('joueurs')
  res.json(transactions.map(t => ({ ...t, joueur_nom: joueurs.find(j => j.id === t.joueur_id)?.nom })))
})

// ===== RAPPORTS =====
app.get('/api/rapports/ca', authMiddleware, (req, res) => {
  const { periode } = req.query
  const today = new Date()
  const todayStr = today.toISOString().slice(0, 10)

  const factures = queryAll('factures').filter(f => f.statut === 'payee')
  const sessions = queryAll('sessions_jeu')
  const joueurs = queryAll('joueurs')
  const jeux = queryAll('jeux')
  const consoles = queryAll('consoles')

  const todayFactures = factures.filter(f => f.date_paiement?.startsWith(todayStr))
  const todaySessions = sessions.filter(s => s.created_at?.startsWith(todayStr))

  const totalRevenus = factures.reduce((s, f) => s + (f.montant_ttc || 0), 0)
  const totalSessions = sessions.length

  const topJeux = jeux.map(j => {
    const count = sessions.filter(s => s.jeu_id === j.id).length
    return { titre: j.titre, sessions: count, pct: totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }
  }).sort((a, b) => b.sessions - a.sessions).slice(0, 5)

  const repartitionConsoles = consoles.map(c => {
    const count = sessions.filter(s => s.console_id === c.id).length
    return { nom: c.nom, sessions: count, pct: totalSessions > 0 ? Math.round(count / totalSessions * 100) : 0 }
  }).sort((a, b) => b.sessions - a.sessions)

  res.json({
    revenus_aujourd_hui: todayFactures.reduce((s, f) => s + (f.montant_ttc || 0), 0),
    sessions_aujourd_hui: todaySessions.length,
    joueurs_actifs: new Set(todaySessions.map(s => s.joueur_id)).size,
    jetons_attribues: queryAll('jetons_transactions').filter(t => t.created_at?.startsWith(todayStr) && t.type === 'gain').reduce((s, t) => s + (t.quantite || 0), 0),
    total_revenus: totalRevenus,
    total_sessions: totalSessions,
    total_joueurs: joueurs.length,
    top_jeux: topJeux,
    repartition_consoles: repartitionConsoles,
  })
})

// ===== USERS (admin) =====
app.get('/api/users', authMiddleware, adminOnly, (req, res) => {
  const users = queryAll('users').map(u => ({ id: u.id, email: u.email, role: u.role, nom: u.nom, created_at: u.created_at }))
  res.json(users)
})

app.post('/api/users', authMiddleware, adminOnly, (req, res) => {
  const { email, password, role, nom } = req.body
  if (!email || !password || !role || !nom) return res.status(400).json({ message: 'Nom, email, mot de passe et rôle requis' })
  if (!['admin', 'employe'].includes(role)) return res.status(400).json({ message: 'Rôle invalide' })
  if (queryOne('users', u => u.email === email)) return res.status(400).json({ message: 'Email déjà utilisé' })
  const password_hash = bcrypt.hashSync(password, 10)
  const user = insert('users', { email, password_hash, role, nom, created_at: new Date().toISOString() })
  res.status(201).json({ id: user.id, email: user.email, role: user.role, nom: user.nom, created_at: user.created_at })
})

app.delete('/api/users/:id', authMiddleware, adminOnly, (req, res) => {
  const id = Number(req.params.id)
  const user = queryOne('users', u => u.id === id)
  if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
  if (user.id === req.user.id) return res.status(400).json({ message: 'Impossible de supprimer votre propre compte' })
  remove('users', id)
  res.json({ success: true })
})

// ===== STATIC FILES =====
app.use(express.static(join(__dirname, '../dist')))
app.get('*', (req, res) => {
  if (!req.path.startsWith('/api')) {
    res.sendFile(join(__dirname, '../dist/index.html'))
  }
})

// ===== ERROR HANDLER =====
app.use((err, req, res, next) => {
  console.error('Server error:', err)
  res.status(500).json({ message: 'Erreur interne du serveur' })
})

app.listen(PORT, '0.0.0.0', () => {
  console.log(`🎮 Game Lounge API running on http://0.0.0.0:${PORT}`)
})
