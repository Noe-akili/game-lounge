import { Router } from 'express'
import bcrypt from 'bcryptjs'
import jwt from 'jsonwebtoken'
import { queryOne } from '../db.js'
import { JWT_SECRET } from '../middleware/auth.js'

const router = Router()

router.post('/login', (req, res) => {
  const { email, password } = req.body
  if (!email || !password) return res.status(400).json({ message: 'Email et mot de passe requis' })
  const user = queryOne('users', u => u.email === email)
  if (!user) return res.status(401).json({ message: 'Identifiants incorrects' })
  if (!bcrypt.compareSync(password, user.password_hash)) return res.status(401).json({ message: 'Identifiants incorrects' })
  const token = jwt.sign({ id: user.id, email: user.email, role: user.role, nom: user.nom }, JWT_SECRET, { expiresIn: '24h' })
  res.json({ token, user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
})

router.get('/me', (req, res) => {
  const header = req.headers.authorization
  if (!header) return res.status(401).json({ message: 'Non autorisé' })
  try {
    const decoded = jwt.verify(header.split(' ')[1], JWT_SECRET)
    const user = queryOne('users', u => u.id === decoded.id)
    if (!user) return res.status(404).json({ message: 'Utilisateur non trouvé' })
    res.json({ user: { id: user.id, email: user.email, role: user.role, nom: user.nom } })
  } catch { res.status(401).json({ message: 'Token invalide' }) }
})

export default router
