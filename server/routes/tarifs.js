import { Router } from 'express'
import { queryAll, queryOne, insert, update } from '../db.js'
import { authMiddleware } from '../middleware/auth.js'

const router = Router()
router.use(authMiddleware)

router.get('/', (req, res) => {
  res.json(queryAll('tarifs'))
})

router.post('/', (req, res) => {
  const { type, duree_minutes, prix, description } = req.body
  const t = insert('tarifs', { type, duree_minutes, prix, description, actif: true })
  res.status(201).json(t)
})

router.get('/fidelite', (req, res) => {
  const regle = queryOne('parametres_fidelite', r => r.actif !== false)
  res.json(regle || { regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
})

router.put('/fidelite', (req, res) => {
  const { regle_type, seuil, jetons_attribues, actif } = req.body
  const existing = queryOne('parametres_fidelite', () => true)
  if (existing) {
    update('parametres_fidelite', existing.id, { regle_type, seuil, jetons_attribues, actif: actif ? true : false })
  } else {
    insert('parametres_fidelite', { regle_type, seuil, jetons_attribues, actif: actif ? true : false })
  }
  res.json(queryOne('parametres_fidelite', () => true))
})

export default router
