import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * Cloche du header : changements importants effectués par LES AUTRES
 * appareils/utilisateurs (nouvelle session, session terminée, facture,
 * modification...). Le backend Rust émet l'événement Tauri "db-change" à
 * chaque écriture de données — y compris celles appliquées par la sync delta
 * venue du cloud — depuis un thread natif : l'écoute continue même quand
 * l'utilisateur quitte l'écran ou que l'app passe en arrière-plan.
 * Les notifications Android natives sont envoyées côté Rust pour les
 * événements importants (indépendantes de l'écran affiché).
 */
export interface NotifEntry {
  id: number
  table: string
  operation: string
  title: string
  body: string
  ts: string
  read: boolean
}

let seq = 1

export const useNotifStore = defineStore('notifications', () => {
  const items = ref<NotifEntry[]>([])
  const unread = ref(0)
  let bound = false

  function push(e: any) {
    items.value.unshift({
      id: seq++,
      table: String(e?.table || ''),
      operation: String(e?.operation || ''),
      title: String(e?.title || 'Mise à jour'),
      body: String(e?.body || ''),
      ts: String(e?.ts || new Date().toISOString()),
      read: false,
    })
    if (items.value.length > 50) items.value.pop()
    unread.value++
  }

  async function bind() {
    if (bound) return
    bound = true
    try {
      const { listen } = await import('@tauri-apps/api/event')
      await listen('db-change', (ev) => push(ev.payload))
    } catch (e) {
      // Hors Tauri (dev navigateur) : pas d'événements natifs, la cloche reste vide.
      bound = false
    }
  }

  function markAllRead() {
    unread.value = 0
    items.value.forEach((i) => (i.read = true))
  }

  function clear() {
    items.value = []
    unread.value = 0
  }

  return { items, unread, bind, markAllRead, clear }
})
