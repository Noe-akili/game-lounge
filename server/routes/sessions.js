import { Router } from 'express'
import { queryAll, queryOne, insert, update, save } from '../db.js'
import { authMiddleware } from '../middleware/auth.js'

const router = Router()
router.use(authMiddleware)

router.get('/', (req, res) => {
  const { statut } = req.query
  let sessions = queryAll('sessions_jeu')
  if (statut) sessions = sessions.filter(s => s.statut === statut)

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

router.get('/:id', (req, res) => {
  const id = Number(req.params.id)
  const s = queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })
  const console_ = queryOne('consoles', c => c.id === s.console_id)
  const joueur = queryOne('joueurs', j => j.id === s.joueur_id)
  const jeu = queryOne('jeux', j => j.id === s.jeu_id)
  const dureeSecondes = s.statut === 'en_cours' ? Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000) : (s.duree_minutes || 0) * 60
  res.json({ ...s, console_nom: console_?.nom, joueur_nom: joueur?.nom, jeu_nom: jeu?.titre, duree_secondes: dureeSecondes })
})

router.post('/', (req, res) => {
  const { console_id, joueur_id, jeu_id } = req.body
  const tarifs = queryAll('tarifs')
  const tarif = tarifs.find(t => t.type === 'horaire' && t.actif !== false)
  const tarif_prix = tarif?.prix || 2000
  const session = insert('sessions_jeu', {
    console_id, joueur_id, jeu_id, employe_id: req.user.id,
    debut: new Date().toISOString(), fin: null, duree_minutes: 0,
    montant: 0, tarif_prix, jetons_gagnes: 0, statut: 'en_cours',
    created_at: new Date().toISOString()
  })
  update('consoles', console_id, { etat: 'occupee' })
  update('joueurs', joueur_id, { derniere_visite: new Date().toISOString() })
  res.status(201).json(session)
})

router.put('/:id/pause', (req, res) => {
  const id = Number(req.params.id)
  const s = update('sessions_jeu', id, { statut: 'pause' })
  if (s) update('consoles', s.console_id, { etat: 'pause' })
  res.json(s)
})

router.put('/:id/reprendre', (req, res) => {
  const id = Number(req.params.id)
  const s = update('sessions_jeu', id, { statut: 'en_cours' })
  if (s) update('consoles', s.console_id, { etat: 'occupee' })
  res.json(s)
})

router.put('/:id/terminer', (req, res) => {
  const id = Number(req.params.id)
  const s = queryOne('sessions_jeu', x => x.id === id)
  if (!s) return res.status(404).json({ message: 'Session non trouvée' })

  const dureeSecondes = Math.floor((Date.now() - new Date(s.debut).getTime()) / 1000)
  const dureeMinutes = Math.max(1, Math.ceil(dureeSecondes / 60))
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

  insert('lignes_facture', {
    facture_id: facture.id, description: `Session console ${s.console_id} - ${dureeMinutes}min`,
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
    insert('jetons_transactions', { joueur_id: s.joueur_id, type: 'gain', quantite: jetonsGagnes, raison: 'Session terminée', session_id: id, created_at: now.toISOString() })
  }

  const updated = queryOne('sessions_jeu', x => x.id === id)
  res.json({ session: updated, montant, jetonsGagnes })
})

export default router
