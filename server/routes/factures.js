import { Router } from 'express'
import { queryAll, queryOne, update } from '../db.js'
import { authMiddleware } from '../middleware/auth.js'

const router = Router()
router.use(authMiddleware)

router.get('/', (req, res) => {
  const { statut, joueur_id } = req.query
  let factures = queryAll('factures')
  if (statut) factures = factures.filter(f => f.statut === statut)
  if (joueur_id) factures = factures.filter(f => f.joueur_id === Number(joueur_id))
  const joueurs = queryAll('joueurs')
  res.json(factures.map(f => ({ ...f, joueur_nom: joueurs.find(j => j.id === f.joueur_id)?.nom })))
})

router.get('/:id', (req, res) => {
  const id = Number(req.params.id)
  const f = queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const joueur = queryOne('joueurs', j => j.id === f.joueur_id)
  const lignes = queryAll('lignes_facture').filter(l => l.facture_id === id)
  res.json({ ...f, joueur_nom: joueur?.nom, joueur_telephone: joueur?.telephone, lignes })
})

router.get('/:id/pdf', async (req, res) => {
  const id = Number(req.params.id)
  const f = queryOne('factures', x => x.id === id)
  if (!f) return res.status(404).json({ message: 'Facture non trouvée' })
  const joueur = queryOne('joueurs', j => j.id === f.joueur_id)
  const lignes = queryAll('lignes_facture').filter(l => l.facture_id === id)

  const { jsPDF } = await import('jspdf')
  const doc = new jsPDF()
  doc.setFontSize(20); doc.text('GAME LOUNGE', 20, 20)
  doc.setFontSize(10); doc.text('Facture', 20, 28)
  doc.text(`N: ${f.numero_facture}`, 20, 35)
  doc.text(`Date: ${f.created_at}`, 20, 42)
  doc.text(`Joueur: ${joueur?.nom || 'N/A'}`, 20, 49)
  let y = 70
  doc.setFontSize(12); doc.text('Designite', 20, y); doc.text('Qte', 120, y); doc.text('Prix', 140, y); doc.text('Total', 170, y); y += 10
  doc.setFontSize(10)
  lignes.forEach(l => { doc.text(l.description, 20, y); doc.text(String(l.quantite), 120, y); doc.text(`${l.prix_unitaire} FC`, 140, y); doc.text(`${l.total_ligne} FC`, 170, y); y += 8 })
  y += 10; doc.setFontSize(11)
  doc.text(`Montant HT: ${f.montant_ht} FC`, 120, y); y += 8
  doc.text(`TVA (${f.taux_tva}%): ${f.montant_tva} FC`, 120, y); y += 8
  doc.setFontSize(14); doc.text(`TOTAL: ${f.montant_ttc} FC`, 120, y)
  res.setHeader('Content-Type', 'application/pdf')
  res.setHeader('Content-Disposition', `attachment; filename="${f.numero_facture}.pdf"`)
  res.send(Buffer.from(doc.output('arraybuffer')))
})

router.put('/:id/annuler', (req, res) => {
  const id = Number(req.params.id)
  update('factures', id, { statut: 'annulee' })
  const f = queryOne('factures', x => x.id === id)
  res.json(f)
})

export default router
