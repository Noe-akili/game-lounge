import { Router } from 'express'
import { queryAll, insert, remove } from '../db.js'
import { authMiddleware } from '../middleware/auth.js'

const router = Router()
router.use(authMiddleware)

router.get('/', (req, res) => {
  const { console_id } = req.query
  let jeux = queryAll('jeux').filter(j => j.actif !== false)
  if (console_id) jeux = jeux.filter(j => j.console_id === Number(console_id))
  res.json(jeux)
})

router.post('/', (req, res) => {
  const { titre, genre, console_id, jaquette_url } = req.body
  const j = insert('jeux', { titre, genre, console_id, jaquette_url, actif: true, created_at: new Date().toISOString() })
  res.status(201).json(j)
})

router.delete('/:id', (req, res) => {
  remove('jeux', Number(req.params.id))
  res.json({ success: true })
})

export default router
