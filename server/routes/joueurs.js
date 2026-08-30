import { Router } from 'express'
import { queryAll, queryOne, insert, update } from '../db.js'
import { authMiddleware } from '../middleware/auth.js'

const router = Router()
router.use(authMiddleware)

router.get('/', (req, res) => {
  const { search } = req.query
  let joueurs = queryAll('joueurs')
  if (search) {
    const q = search.toLowerCase()
    joueurs = joueurs.filter(j => j.nom?.toLowerCase().includes(q) || j.telephone?.includes(q) || j.email?.toLowerCase().includes(q))
  }
  res.json(joueurs)
})

router.post('/', (req, res) => {
  const { nom, telephone, email } = req.body
  const j = insert('joueurs', { nom, telephone, email, jetons_solde: 0, date_inscription: new Date().toISOString(), derniere_visite: null })
  res.status(201).json(j)
})

router.put('/:id', (req, res) => {
  const id = Number(req.params.id)
  const { nom, telephone, email } = req.body
  const j = update('joueurs', id, { nom, telephone, email })
  if (!j) return res.status(404).json({ message: 'Joueur non trouvé' })
  res.json(j)
})

router.get('/:id/historique', (req, res) => {
  const id = Number(req.params.id)
  const sessions = queryAll('sessions_jeu').filter(s => s.joueur_id === id)
  const transactions = queryAll('jetons_transactions').filter(t => t.joueur_id === id)
  res.json({ sessions, transactions })
})

export default router
