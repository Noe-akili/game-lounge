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

export function calcJetonsEarned(durationMinutes, rules, montant) {
  if (!rules || !rules.actif) return 0
  // Règle 'temps' : jetons par tranche de durée jouée (seuil en minutes).
  if (rules.regle_type === 'temps') {
    return Math.floor(durationMinutes / rules.seuil) * rules.jetons_attribues
  }
  // Règle 'montant' : bonus selon la dépense (seuil = montant en FC),
  // alignée sur le backend (sessions.rs) : (montant / seuil) × jetons.
  if (rules.regle_type === 'montant' && montant) {
    return rules.seuil > 0 && montant >= rules.seuil
      ? Math.floor(montant / rules.seuil) * rules.jetons_attribues
      : 0
  }
  return 0
}

/**
 * Normalise une URL d'image (jaquette de console ou de jeu).
 *
 * Trois causes d'« image qui ne s'affiche pas » sont traitées ici :
 *  - lien collé sans schéma ("images.site.com/ps5.jpg") : le navigateur le lit
 *    comme un chemin relatif à l'application et ne trouve rien -> on ajoute https://
 *  - guillemets / espaces collés autour du lien lors du copier-coller ;
 *  - lien http:// simple, conservé tel quel (la CSP l'autorise désormais).
 *
 * Retourne '' si rien d'exploitable.
 */
export function normalizeImageUrl(url) {
  if (typeof url !== 'string') return ''
  let u = url.trim().replace(/^["'\s]+|["'\s]+$/g, '')
  if (!u) return ''
  if (u.startsWith('data:') || u.startsWith('blob:')) return u
  if (/^\/\//.test(u)) return 'https:' + u
  if (!/^https?:\/\//i.test(u)) {
    // Un chemin local de téléphone n'est pas affichable par la WebView.
    if (/^(file:|\/storage\/|\/data\/|[a-zA-Z]:\\)/.test(u)) return ''
    u = 'https://' + u.replace(/^\/+/, '')
  }
  return u
}
