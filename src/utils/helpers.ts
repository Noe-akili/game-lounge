// @ts-nocheck
export function formatDuration(seconds) {
  if (!seconds || seconds < 0) return '00:00:00'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)
  return [h, m, s].map(v => String(v).padStart(2, '0')).join(':')
}

export function formatCurrency(amount) {
  return new Intl.NumberFormat('fr-FR').format(amount) + ' FC'
}

export function formatDate(date) {
  if (!date) return ''
  return new Intl.DateTimeFormat('fr-FR', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit'
  }).format(new Date(date))
}

export function formatDateShort(date) {
  if (!date) return ''
  return new Intl.DateTimeFormat('fr-FR', {
    day: '2-digit', month: '2-digit', year: 'numeric'
  }).format(new Date(date))
}

export function timeAgo(date) {
  if (!date) return ''
  const now = Date.now()
  const diff = now - new Date(date).getTime()
  const minutes = Math.floor(diff / 60000)
  const hours = Math.floor(diff / 3600000)
  const days = Math.floor(diff / 86400000)
  if (minutes < 1) return "Aujourd'hui"
  if (minutes < 60) return `Il y a ${minutes}min`
  if (hours < 24) return `Il y a ${hours}h`
  if (days === 1) return 'Hier'
  return `Il y a ${days}j`
}

export function generateInvoiceNumber() {
  const now = new Date()
  const date = now.toISOString().slice(0, 10).replace(/-/g, '')
  const random = String(Math.floor(Math.random() * 9999)).padStart(4, '0')
  return `FAC-${date}-${random}`
}

export function calcSessionAmount(durationMinutes, tarif) {
  if (!tarif) return 0
  if (tarif.type === 'forfait') return tarif.prix
  return Math.ceil(durationMinutes / 60) * tarif.prix
}

export function calcJetonsEarned(durationMinutes, rules) {
  if (!rules || !rules.actif) return 0
  if (rules.regle_type === 'temps') {
    return Math.floor(durationMinutes / rules.seuil) * rules.jetons_attribues
  }
  return 0
}

export function calcTVA(montantHT, taux = 20) {
  const tva = montantHT * (taux / 100)
  return { montantHT, taux_tva: taux, montant_tva: tva, montant_ttc: montantHT + tva }
}
