// @ts-nocheck
import { isTauri, handleRequest } from './transport'

function base64ToUint8Array(b64: string): Uint8Array {
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return bytes
}

export async function getFacturePdfBlob(id: number): Promise<Blob> {
  const token = localStorage.getItem('gl_token')

  // En mode Tauri : PDF généré par le backend Rust (commande factures_pdf).
  if (isTauri()) {
    const result = await handleRequest(`/factures/${id}/pdf`, 'GET', undefined, token)
    if (result.status >= 400) throw new Error(result.body?.message || 'Facture non trouvée')
    const b64 = result.body?.pdf_base64
    if (!b64) throw new Error('PDF non généré')
    return new Blob([base64ToUint8Array(b64)], { type: 'application/pdf' })
  }

  // Mode navigateur : génération locale (jspdf).
  const result = await handleRequest(`/factures/${id}`, 'GET', undefined, token)
  if (result.status >= 400) throw new Error(result.body?.message || 'Facture non trouvée')
  const f = result.body

  const { jsPDF } = await import('jspdf')
  const doc = new jsPDF()
  doc.setFontSize(22); doc.setFont('helvetica', 'bold'); doc.text('GAME LOUNGE', 105, 18, { align: 'center' })
  doc.setFontSize(10); doc.setFont('helvetica', 'normal'); doc.text('Facture de prestation', 105, 25, { align: 'center' })

  doc.setDrawColor(168, 85, 247); doc.setLineWidth(0.5); doc.line(20, 30, 190, 30)

  doc.setFontSize(11); doc.setFont('helvetica', 'bold')
  doc.text(`N° ${f.numero_facture}`, 20, 40)
  doc.setFont('helvetica', 'normal'); doc.setFontSize(10)
  doc.text(`Date : ${new Date(f.created_at).toLocaleDateString('fr-FR')}`, 20, 48)
  doc.text(`Joueur : ${f.joueur_nom || 'N/A'}`, 20, 56)
  doc.text(`Téléphone : ${f.joueur_telephone || 'N/A'}`, 20, 64)

  doc.setFillColor(168, 85, 247); doc.rect(20, 74, 170, 8, 'F')
  doc.setTextColor(255, 255, 255); doc.setFontSize(9); doc.setFont('helvetica', 'bold')
  doc.text('Désignation', 22, 80)
  doc.text('Qté', 120, 80); doc.text('P.U.', 135, 80); doc.text('Total', 165, 80)
  doc.setTextColor(0, 0, 0)

  let y = 90
  doc.setFont('helvetica', 'normal'); doc.setFontSize(10)
  const lignes = f.lignes || []
  lignes.forEach((l: any) => {
    doc.text(l.description || '', 22, y)
    doc.text(String(l.quantite), 120, y)
    doc.text(`${new Intl.NumberFormat('fr-FR').format(l.prix_unitaire)} FC`, 135, y)
    doc.text(`${new Intl.NumberFormat('fr-FR').format(l.total_ligne)} FC`, 165, y)
    y += 8
  })

  doc.setDrawColor(200, 200, 200); doc.line(120, y, 190, y); y += 8

  doc.setFontSize(10); doc.setFont('helvetica', 'normal')
  doc.text('Montant HT :', 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_ht)} FC`, 165, y); y += 7
  doc.text(`TVA (${f.taux_tva}%) :`, 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_tva)} FC`, 165, y); y += 7

  doc.setDrawColor(168, 85, 247); doc.setLineWidth(0.8); doc.line(120, y, 190, y); y += 8
  doc.setFontSize(14); doc.setFont('helvetica', 'bold')
  doc.text('TOTAL :', 120, y); doc.text(`${new Intl.NumberFormat('fr-FR').format(f.montant_ttc)} FC`, 165, y)

  doc.setDrawColor(168, 85, 247); doc.setLineWidth(0.5); doc.line(20, 270, 190, 270)
  doc.setFontSize(8); doc.setFont('helvetica', 'normal'); doc.setTextColor(128, 128, 128)
  doc.text('Game Lounge — Merci pour votre visite !', 105, 276, { align: 'center' })

  return new Blob([doc.output('blob')], { type: 'application/pdf' })
}