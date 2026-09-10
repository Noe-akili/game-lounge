// @ts-nocheck
import initSqlJs from 'sql.js'

let db: any = null
let SQL: any = null

const DB_NAME = 'gamelounge'
const STORE_NAME = 'sqlitedb'
const DB_VERSION = 1

function openIdb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const idb = req.result
      if (!idb.objectStoreNames.contains(STORE_NAME)) {
        idb.createObjectStore(STORE_NAME)
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

async function loadFromIdb(): Promise<Uint8Array | null> {
  try {
    const idb = await openIdb()
    return new Promise((resolve, reject) => {
      const tx = idb.transaction(STORE_NAME, 'readonly')
      const store = tx.objectStore(STORE_NAME)
      const req = store.get('db')
      req.onsuccess = () => {
        idb.close()
        resolve(req.result || null)
      }
      req.onerror = () => {
        idb.close()
        reject(req.error)
      }
    })
  } catch { return null }
}

async function saveToIdb(data: Uint8Array) {
  try {
    const idb = await openIdb()
    return new Promise((resolve, reject) => {
      const tx = idb.transaction(STORE_NAME, 'readwrite')
      const store = tx.objectStore(STORE_NAME)
      const req = store.put(data, 'db')
      req.onsuccess = () => { idb.close(); resolve() }
      req.onerror = () => { idb.close(); reject(req.error) }
    })
  } catch {}
}

let saveTimeout: any = null
export function scheduleSave() {
  if (saveTimeout) clearTimeout(saveTimeout)
  saveTimeout = setTimeout(() => {
    if (db) {
      const data = db.export()
      saveToIdb(new Uint8Array(data))
    }
  }, 500)
}

const SCHEMA = `
  CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT UNIQUE, password_hash TEXT, nom TEXT, role TEXT DEFAULT 'employe', created_at TEXT);
  CREATE TABLE IF NOT EXISTS consoles (id INTEGER PRIMARY KEY AUTOINCREMENT, nom TEXT, type TEXT, etat TEXT DEFAULT 'disponible', poste_numero INTEGER, created_at TEXT);
  CREATE TABLE IF NOT EXISTS jeux (id INTEGER PRIMARY KEY AUTOINCREMENT, titre TEXT, genre TEXT, console_id INTEGER, actif INTEGER DEFAULT 1, jaquette_url TEXT, created_at TEXT);
  CREATE TABLE IF NOT EXISTS joueurs (id INTEGER PRIMARY KEY AUTOINCREMENT, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TEXT, derniere_visite TEXT);
  CREATE TABLE IF NOT EXISTS sessions_jeu (id INTEGER PRIMARY KEY AUTOINCREMENT, console_id INTEGER, joueur_id INTEGER, jeu_id INTEGER, employe_id INTEGER, tarif_id INTEGER, debut TEXT, fin TEXT, duree_minutes INTEGER, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TEXT);
  CREATE TABLE IF NOT EXISTS tarifs (id INTEGER PRIMARY KEY AUTOINCREMENT, nom TEXT, type TEXT, prix INTEGER, duree_minutes INTEGER, description TEXT, actif INTEGER DEFAULT 1, console_type TEXT, jeu TEXT, created_at TEXT);
  CREATE TABLE IF NOT EXISTS factures (id INTEGER PRIMARY KEY AUTOINCREMENT, numero_facture TEXT UNIQUE, session_id INTEGER, joueur_id INTEGER, montant_ht REAL, taux_tva REAL DEFAULT 20, montant_tva REAL, montant_ttc REAL, mode_paiement TEXT, statut TEXT, date_paiement TEXT, created_at TEXT);
  CREATE TABLE IF NOT EXISTS jetons_transactions (id INTEGER PRIMARY KEY AUTOINCREMENT, joueur_id INTEGER, quantite INTEGER, type TEXT, raison TEXT, session_id INTEGER, created_at TEXT);
  CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY AUTOINCREMENT, titre TEXT, contenu TEXT, auteur TEXT, created_at TEXT);
  CREATE TABLE IF NOT EXISTS parametres_fidelite (id INTEGER PRIMARY KEY AUTOINCREMENT, regle_type TEXT, seuil INTEGER, jetons_attribues INTEGER, actif INTEGER DEFAULT 1);
  CREATE TABLE IF NOT EXISTS lignes_facture (id INTEGER PRIMARY KEY AUTOINCREMENT, facture_id INTEGER, description TEXT, quantite INTEGER, prix_unitaire INTEGER, total_ligne INTEGER, created_at TEXT);
`

export async function initClientDb() {
  if (db) return db

  SQL = await initSqlJs({ locateFile: () => 'https://sql.js.org/dist/sql-wasm.wasm' })

  const prev = await loadFromIdb()
  db = new SQL.Database(prev || undefined)

  if (!prev) {
    db.exec(SCHEMA)
    const data = db.export()
    await saveToIdb(new Uint8Array(data))
  }

  return db
}

export function getDb() {
  if (!db) throw new Error('Database not initialized')
  return db
}

export function queryAll(table: string, filter?: (row: any) => boolean): any[] {
  try {
    const stmt = db.prepare(`SELECT * FROM ${table}`)
    const rows: any[] = []
    while (stmt.step()) rows.push(stmt.getAsObject())
    stmt.free()
    return filter ? rows.filter(filter) : rows
  } catch { return [] }
}

export function queryOne(table: string, filter: (row: any) => boolean): any {
  return queryAll(table).find(filter) || null
}

export function insert(table: string, data: any): any {
  const keys = Object.keys(data).filter(k => k !== 'id')
  const vals = keys.map(k => data[k])
  const q = data.id != null
    ? `INSERT OR REPLACE INTO ${table} (id, ${keys.join(',')}) VALUES (?, ${keys.map(() => '?').join(',')})`
    : `INSERT INTO ${table} (${keys.join(',')}) VALUES (${keys.map(() => '?').join(',')})`
  const params = data.id != null ? [data.id, ...vals] : vals
  db.run(q, params)
  const id = data.id ?? db.exec(`SELECT last_insert_rowid() as id`)[0]?.values?.[0]?.[0]
  scheduleSave()
  return { id, ...data }
}

export function update(table: string, id: number, data: any): any {
  const keys = Object.keys(data)
  const vals = keys.map(k => data[k])
  const q = `UPDATE ${table} SET ${keys.map(k => `${k}=?`).join(',')} WHERE id=?`
  db.run(q, [...vals, id])
  scheduleSave()
  return queryOne(table, (r: any) => r.id === id)
}

export function remove(table: string, id: number) {
  db.run(`DELETE FROM ${table} WHERE id=?`, [id])
  scheduleSave()
}

export async function seedClientDb() {
  const bcrypt = await import('bcryptjs')
  if (queryAll('users').length > 0) return

  const now = new Date().toISOString()
  const adminHash = bcrypt.hashSync('admin123', 10)
  const empHash = bcrypt.hashSync('employe123', 10)

  insert('users', { email: 'admin@gamelounge.com', password_hash: adminHash, role: 'admin', nom: 'Admin', created_at: now })
  insert('users', { email: 'john@gamelounge.com', password_hash: empHash, role: 'employe', nom: 'John Doe', created_at: now })

  const consolesData = [
    { nom: 'PS5 - Poste 1', type: 'PS5', poste_numero: 1, etat: 'disponible' },
    { nom: 'PS5 - Poste 2', type: 'PS5', poste_numero: 2, etat: 'disponible' },
    { nom: 'PS4 - Poste 3', type: 'PS4', poste_numero: 3, etat: 'disponible' },
    { nom: 'PS4 - Poste 4', type: 'PS4', poste_numero: 4, etat: 'disponible' },
    { nom: 'PS5 - Poste 5', type: 'PS5', poste_numero: 5, etat: 'disponible' },
    { nom: 'PS4 - Poste 6', type: 'PS4', poste_numero: 6, etat: 'disponible' },
  ]
  for (const c of consolesData) insert('consoles', { ...c, created_at: now })

  const c1 = queryOne('consoles', (c: any) => c.poste_numero === 1)
  const c2 = queryOne('consoles', (c: any) => c.poste_numero === 2)
  const c3 = queryOne('consoles', (c: any) => c.poste_numero === 3)
  const c4 = queryOne('consoles', (c: any) => c.poste_numero === 4)
  const c5 = queryOne('consoles', (c: any) => c.poste_numero === 5)
  const c6 = queryOne('consoles', (c: any) => c.poste_numero === 6)

  const jeuxData = [
    { titre: 'FIFA 26', genre: 'Sport', console_id: c1.id },
    { titre: 'FIFA 26', genre: 'Sport', console_id: c3.id },
    { titre: 'Mortal Kombat 1', genre: 'Combat', console_id: c1.id },
    { titre: 'Mortal Kombat 11', genre: 'Combat', console_id: c3.id },
    { titre: 'Tekken 8', genre: 'Combat', console_id: c2.id },
    { titre: 'Need for Speed', genre: 'Course', console_id: c1.id },
    { titre: 'Need for Speed', genre: 'Course', console_id: c3.id },
    { titre: 'WWE 2K25', genre: 'Combat', console_id: c2.id },
    { titre: 'NBA 2K25', genre: 'Sport', console_id: c1.id },
    { titre: 'NBA 2K25', genre: 'Sport', console_id: c4.id },
    { titre: 'Gran Turismo 7', genre: 'Course', console_id: c5.id },
    { titre: 'GTA V', genre: 'Action', console_id: c1.id },
    { titre: 'GTA V', genre: 'Action', console_id: c3.id },
    { titre: 'God of War', genre: 'Action', console_id: c2.id },
    { titre: 'God of War', genre: 'Action', console_id: c4.id },
    { titre: 'Call of Duty', genre: 'Action', console_id: c5.id },
    { titre: 'Call of Duty', genre: 'Action', console_id: c6.id },
    { titre: 'Fortnite', genre: 'Action', console_id: c5.id },
    { titre: 'Spider-Man 2', genre: 'Action', console_id: c2.id },
    { titre: 'Red Dead Redemption 2', genre: 'Action', console_id: c4.id },
    { titre: 'Resident Evil 4', genre: 'Horreur', console_id: c6.id },
    { titre: 'Undisputed', genre: 'Combat', console_id: c6.id },
    { titre: 'EA Sports UFC 5', genre: 'Combat', console_id: c1.id },
    { titre: 'Naruto Storm 4', genre: 'Combat', console_id: c3.id },
    { titre: 'Uncharted 4', genre: 'Aventure', console_id: c4.id },
  ]
  for (const j of jeuxData) insert('jeux', { ...j, jaquette_url: null, actif: 1, created_at: now })

  const joueursData = [
    { nom: 'Kevin M.', telephone: '+243812345678', email: 'kevin@email.com', jetons_solde: 12 },
    { nom: 'Alex B.', telephone: '+243823456789', email: 'alex@email.com', jetons_solde: 5 },
    { nom: 'Tom D.', telephone: '+243834567890', email: 'tom@email.com', jetons_solde: 8 },
    { nom: 'Sarah L.', telephone: '+243845678901', email: 'sarah@email.com', jetons_solde: 3 },
    { nom: 'John R.', telephone: '+243856789012', email: 'johnr@email.com', jetons_solde: 15 },
    { nom: 'Lucas R.', telephone: '+243867890123', email: 'lucas@email.com', jetons_solde: 2 },
    { nom: 'Ethan G.', telephone: '+243878901234', email: 'ethan@email.com', jetons_solde: 7 },
  ]
  for (const j of joueursData) insert('joueurs', { ...j, date_inscription: now, derniere_visite: null })

  // PS4 Tarifs
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'FIFA 26', duree_minutes: 5, prix: 500, description: 'FIFA 26 PS4 - 1 Match 5min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'FIFA 26', duree_minutes: 30, prix: 2000, description: 'FIFA 26 PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'FIFA 26', duree_minutes: 60, prix: 4000, description: 'FIFA 26 PS4 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'Mortal Kombat', duree_minutes: 5, prix: 500, description: 'Mortal Kombat PS4 - 2 Combats simples', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Mortal Kombat', duree_minutes: 30, prix: 1500, description: 'Mortal Kombat PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Mortal Kombat', duree_minutes: 60, prix: 3000, description: 'Mortal Kombat PS4 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'Tekken', duree_minutes: 5, prix: 500, description: 'Tekken PS4 - 2 Combats simples', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Tekken', duree_minutes: 30, prix: 1500, description: 'Tekken PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Tekken', duree_minutes: 60, prix: 3000, description: 'Tekken PS4 - Séance 1h', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Need for Speed', duree_minutes: 15, prix: 500, description: 'NFS PS4 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Need for Speed', duree_minutes: 30, prix: 1000, description: 'NFS PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Need for Speed', duree_minutes: 60, prix: 2000, description: 'NFS PS4 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'WWE 2K25', duree_minutes: 10, prix: 1000, description: 'WWE PS4 - 1 Combat 10min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'WWE 2K25', duree_minutes: 30, prix: 2000, description: 'WWE PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'WWE 2K25', duree_minutes: 60, prix: 4000, description: 'WWE PS4 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'NBA 2K25', duree_minutes: 15, prix: 1500, description: 'NBA PS4 - 1 Match 10-20min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'NBA 2K25', duree_minutes: 30, prix: 2500, description: 'NBA PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'NBA 2K25', duree_minutes: 60, prix: 4000, description: 'NBA PS4 - Séance 1h', actif: true })

  const ps4Actions = ['GTA V', 'God of War', 'Call of Duty', 'Spider-Man 2', 'Red Dead Redemption 2', 'Uncharted 4', 'Resident Evil 4']
  for (const jeu of ps4Actions) {
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 15, prix: 500, description: `${jeu} PS4 - Séance 15min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 30, prix: 1500, description: `${jeu} PS4 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 60, prix: 3000, description: `${jeu} PS4 - Séance 1h`, actif: true })
  }
  const ps4Combat = ['EA Sports UFC 5', 'Undisputed', 'Naruto Storm 4']
  for (const jeu of ps4Combat) {
    insert('tarifs', { type: 'partie', console_type: 'PS4', jeu, duree_minutes: 5, prix: 500, description: `${jeu} PS4 - Combat simple`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 30, prix: 1500, description: `${jeu} PS4 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 60, prix: 3000, description: `${jeu} PS4 - Séance 1h`, actif: true })
  }
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Fortnite', duree_minutes: 15, prix: 500, description: 'Fortnite PS4 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Fortnite', duree_minutes: 30, prix: 2000, description: 'Fortnite PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Fortnite', duree_minutes: 60, prix: 3000, description: 'Fortnite PS4 - Séance 1h', actif: true })

  // PS5 Tarifs
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'FIFA 26', duree_minutes: 5, prix: 1000, description: 'FIFA 26 PS5 - 1 Match 5min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'FIFA 26', duree_minutes: 30, prix: 4000, description: 'FIFA 26 PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'FIFA 26', duree_minutes: 60, prix: 8000, description: 'FIFA 26 PS5 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'Mortal Kombat', duree_minutes: 5, prix: 1000, description: 'Mortal Kombat PS5 - Combat simple', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Mortal Kombat', duree_minutes: 30, prix: 3000, description: 'Mortal Kombat PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Mortal Kombat', duree_minutes: 60, prix: 6000, description: 'Mortal Kombat PS5 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'Tekken 8', duree_minutes: 5, prix: 1000, description: 'Tekken 8 PS5 - Combat simple', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Tekken 8', duree_minutes: 30, prix: 3000, description: 'Tekken 8 PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Tekken 8', duree_minutes: 60, prix: 6000, description: 'Tekken 8 PS5 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 5, prix: 1000, description: 'GT7 PS5 - Course courte', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 15, prix: 2000, description: 'GT7 PS5 - 15min Volant G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 15, prix: 3000, description: 'GT7 PS5 - 15min Volant G29 + VR2', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 30, prix: 3500, description: 'GT7 PS5 - 30min Volant G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 30, prix: 5000, description: 'GT7 PS5 - 30min Volant G29 + VR2', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 60, prix: 7000, description: 'GT7 PS5 - 1h Volant G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 60, prix: 10000, description: 'GT7 PS5 - 1h Volant G29 + VR2', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 15, prix: 1000, description: 'NFS PS5 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 15, prix: 1500, description: 'NFS PS5 - 15min avec G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 30, prix: 2000, description: 'NFS PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 30, prix: 3000, description: 'NFS PS5 - 30min avec G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 60, prix: 4000, description: 'NFS PS5 - Séance 1h', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 60, prix: 6000, description: 'NFS PS5 - 1h avec G29', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'WWE 2K25', duree_minutes: 10, prix: 1500, description: 'WWE PS5 - 1 Combat 10min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'WWE 2K25', duree_minutes: 30, prix: 3000, description: 'WWE PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'WWE 2K25', duree_minutes: 60, prix: 6000, description: 'WWE PS5 - Séance 1h', actif: true })
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'NBA 2K25', duree_minutes: 15, prix: 2000, description: 'NBA PS5 - 1 Match 10-20min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'NBA 2K25', duree_minutes: 30, prix: 4000, description: 'NBA PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'NBA 2K25', duree_minutes: 60, prix: 7000, description: 'NBA PS5 - Séance 1h', actif: true })

  const ps5Actions = ['GTA V', 'God of War', 'Spider-Man 2', 'Red Dead Redemption 2', 'Uncharted 4', 'Resident Evil 4']
  for (const jeu of ps5Actions) {
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 15, prix: 1000, description: `${jeu} PS5 - Séance 15min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 30, prix: 2000, description: `${jeu} PS5 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 60, prix: 4000, description: `${jeu} PS5 - Séance 1h`, actif: true })
  }
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Call of Duty', duree_minutes: 15, prix: 1500, description: 'CoD PS5 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Call of Duty', duree_minutes: 30, prix: 3000, description: 'CoD PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Call of Duty', duree_minutes: 60, prix: 6000, description: 'CoD PS5 - Séance 1h', actif: true })
  const ps5Combat = ['EA Sports UFC 5', 'Undisputed', 'Naruto Storm 4']
  for (const jeu of ps5Combat) {
    insert('tarifs', { type: 'partie', console_type: 'PS5', jeu, duree_minutes: 5, prix: 1000, description: `${jeu} PS5 - Combat simple`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 30, prix: 3000, description: `${jeu} PS5 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 60, prix: 6000, description: `${jeu} PS5 - Séance 1h`, actif: true })
  }

  insert('parametres_fidelite', { regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })

  scheduleSave()
}
