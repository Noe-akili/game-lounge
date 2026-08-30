import { Router } from 'express'
import { queryAll, queryOne, insert, update, remove } from '../db.js'
import { authMiddleware } from '../middleware/auth.js'

const router = Router()
router.use(authMiddleware)

router.get('/', (req, res) => {
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
  })
  res.json(result)
})

router.post('/', (req, res) => {
  const { nom, type, poste_numero, etat } = req.body
  const c = insert('consoles', { nom, type, poste_numero, etat: etat || 'disponible', created_at: new Date().toISOString() })
  res.status(201).json(c)
})

router.put('/:id', (req, res) => {
  const id = Number(req.params.id)
  const { nom, type, poste_numero, etat } = req.body
  const c = update('consoles', id, { nom, type, poste_numero, etat })
  if (!c) return res.status(404).json({ message: 'Console non trouvée' })
  res.json(c)
})

router.delete('/:id', (req, res) => {
  remove('consoles', Number(req.params.id))
  res.json({ success: true })
})

export default router
