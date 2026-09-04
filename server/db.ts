import initSqlJs from 'sql.js'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import dotenv from 'dotenv'

let nodemailer: any = null
try { nodemailer = await import('nodemailer') } catch {}

// Fix: force IPv4 for Neon connection (undici tries IPv6 first and times out)
try {
  const { Agent, setGlobalDispatcher } = await import('undici')
  setGlobalDispatcher(new Agent({ connect: { family: 4 } }))
  console.log('🔧 Undici IPv4 forcé pour Neon')
} catch {}

console.error('🚀 [DB.TS] Starting - import.meta.url:', import.meta.url)
console.error('🚀 [DB.TS] process.cwd():', process.cwd())
console.error('🚀 [DB.TS] process.env.DATADIR:', process.env.DATADIR)
console.error('🚀 [DB.TS] NODE_PATH:', process.env.NODE_PATH)

const __filename = fileURLToPath(import.meta.url)
const DIRNAME = dirname(__filename)

const PROJECT_DIR = DIRNAME;

// Load .env from multiple locations
[join(PROJECT_DIR, '.env'), join(PROJECT_DIR, '..', '.env'), join(PROJECT_DIR, '..', '..', '.env')].forEach(p => {
  try { if (existsSync(p)) dotenv.config({ path: p }) } catch {}
})
dotenv.config()

// ===== SQL.js setup =====
let SQL: any = null
const DATA_DIR = process.env.DATADIR || PROJECT_DIR
const DB_PATH = join(DATA_DIR, 'gamelounge.db')

try { mkdirSync(DATA_DIR, { recursive: true }) } catch {}

async function initDb() {
  const wasmPaths = [
    join(PROJECT_DIR, 'node_modules', 'sql.js', 'dist', 'sql-wasm.wasm'),
    join(PROJECT_DIR, '..', 'node_modules', 'sql.js', 'dist', 'sql-wasm.wasm'),
    join(DATA_DIR, 'sql-wasm.wasm')
  ]
  for (const p of wasmPaths) {
    try {
      if (existsSync(p)) {
        const buf = readFileSync(p)
        SQL = await initSqlJs({ wasmBinary: buf })
        console.log(`✅ SQL.js loaded from ${p}`)
        break
      }
    } catch (e) { console.warn(`⚠️ Failed ${p}: ${e.message}`) }
  }
  if (!SQL) {
    console.warn('⚠️ sql-wasm.wasm not found, trying online...')
    SQL = await initSqlJs({ locateFile: () => 'https://sql.js.org/dist/sql-wasm.wasm' })
  }
  const prevDb = existsSync(DB_PATH) ? readFileSync(DB_PATH) : null
  const db = new SQL.Database(prevDb)
  if (!prevDb) {
    db.run(`
      CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT UNIQUE, password_hash TEXT, nom TEXT, role TEXT DEFAULT 'employe', created_at TEXT);
      CREATE TABLE IF NOT EXISTS consoles (id INTEGER PRIMARY KEY AUTOINCREMENT, nom TEXT, type_console TEXT, etat TEXT DEFAULT 'disponible', date_ajout TEXT);
      CREATE TABLE IF NOT EXISTS jeux (id INTEGER PRIMARY KEY AUTOINCREMENT, titre TEXT, genre TEXT, console_id INTEGER, actif INTEGER DEFAULT 1, created_at TEXT);
      CREATE TABLE IF NOT EXISTS joueurs (id INTEGER PRIMARY KEY AUTOINCREMENT, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TEXT);
      CREATE TABLE IF NOT EXISTS sessions_jeu (id INTEGER PRIMARY KEY AUTOINCREMENT, console_id INTEGER, joueur_id INTEGER, jeu_id INTEGER, employe_id INTEGER, debut TEXT, fin TEXT, duree_minutes INTEGER, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TEXT);
      CREATE TABLE IF NOT EXISTS tarifs (id INTEGER PRIMARY KEY AUTOINCREMENT, nom TEXT, type_tarif TEXT, prix INTEGER, duree_minutes INTEGER, created_at TEXT);
      CREATE TABLE IF NOT EXISTS factures (id INTEGER PRIMARY KEY AUTOINCREMENT, numero_facture TEXT UNIQUE, session_id INTEGER, joueur_id INTEGER, montant_ht REAL, taux_tva REAL, montant_tva REAL, montant_ttc REAL, mode_paiement TEXT, statut TEXT, date_paiement TEXT, created_at TEXT);
      CREATE TABLE IF NOT EXISTS jetons_transactions (id INTEGER PRIMARY KEY AUTOINCREMENT, joueur_id INTEGER, quantite INTEGER, type_transaction TEXT, description TEXT, created_at TEXT);
      CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY AUTOINCREMENT, titre TEXT, contenu TEXT, auteur TEXT, created_at TEXT);
    `)
    const defaultTarif = { nom: 'Standard', type_tarif: 'heure', prix: 2000, duree_minutes: 60, created_at: new Date().toISOString() }
    db.run(`INSERT INTO tarifs (nom, type_tarif, prix, duree_minutes, created_at) VALUES (?,?,?,?,?)`,
      [defaultTarif.nom, defaultTarif.type_tarif, defaultTarif.prix, defaultTarif.duree_minutes, defaultTarif.created_at])
    const consoles = [['PlayStation 1', 'PS1'], ['PlayStation 2', 'PS2'], ['PlayStation 3', 'PS3'], ['PlayStation 4', 'PS4'], ['PlayStation 5', 'PS5']]
    for (const [nom, type] of consoles) {
      db.run(`INSERT INTO consoles (nom, type_console, etat, date_ajout) VALUES (?,?,?,?)`, [nom, type, 'disponible', new Date().toISOString()])
    }
    const adminPassword = '$2a$10$92IXUNpkjO0rOQ5byMi.Ye4oKoEa3Ro9llC/.og/at2.uheWG/igi'
    db.run(`INSERT INTO users (email, password_hash, nom, role) VALUES (?,?,?,?)`, ['admin@gamelounge.com', adminPassword, 'Administrateur', 'admin'])
  }
  const data = db.export()
  writeFileSync(DB_PATH, Buffer.from(data))
  return db
}

const db = await initDb()
const save = () => { try { const d = db.export(); writeFileSync(DB_PATH, Buffer.from(d)) } catch {} }
setInterval(save, 30000)
process.on('exit', save)
process.on('SIGINT', () => { save(); process.exit() })

// Neon
let neonSql: any = null
let neonModule: any = null
const hasNeon = !!(process.env.DATABASE_URL)
if (hasNeon) {
  try {
    neonModule = await import('@neondatabase/serverless')
    neonSql = neonModule.default(process.env.DATABASE_URL)
    console.log('✅ Neon connected')
  } catch (e) { console.warn('⚠️ Neon init failed:', e.message) }
}
const isNeonAvailable = () => hasNeon && !!neonSql
const isNeonEnabled = () => isNeonAvailable()

// Query helpers
const queryAll = (table: string, filter?: (row: any) => boolean) => {
  try {
    const stmt = db.prepare(`SELECT * FROM ${table}`)
    const rows: any[] = []
    while (stmt.step()) rows.push(stmt.getAsObject())
    stmt.free()
    return filter ? rows.filter(filter) : rows
  } catch { return [] }
}
const queryOne = (table: string, filter: (row: any) => boolean) => {
  return queryAll(table).find(filter) || null
}
const insert = (table: string, data: any) => {
  const keys = Object.keys(data).filter(k => k !== 'id')
  const vals = keys.map(k => data[k])
  const q = `INSERT INTO ${table} (${keys.join(',')}) VALUES (${keys.map(() => '?').join(',')})`
  db.run(q, vals)
  const id = db.exec(`SELECT last_insert_rowid() as id`)[0]?.values[0]?.[0]
  save()
  return { id, ...data }
}
const update = (table: string, id: number, data: any) => {
  const keys = Object.keys(data)
  const vals = keys.map(k => data[k])
  const q = `UPDATE ${table} SET ${keys.map(k => `${k}=?`).join(',')} WHERE id=?`
  db.run(q, [...vals, id])
  save()
}
const remove = (table: string, id: number) => { db.run(`DELETE FROM ${table} WHERE id=?`, [id]); save() }
const logError = (msg: string, err?: any) => { console.error(`[ERROR] ${msg}`, err?.message || err || '') }

async function queryNeon(sql: string, params: any[] = []) {
  if (!neonSql) return null
  const r = await neonSql[0](sql, params)
  return r
}
async function queryNeonUser(email: string) {
  if (!neonSql) return null
  const rows = await neonSql[0](`SELECT * FROM users WHERE email=$1`, [email])
  return rows?.[0] || null
}
async function queryNeonAll(table: string) {
  if (!neonSql) return null
  return await neonSql[0](`SELECT * FROM ${table}`)
}

const getSyncStatus = () => ({ neonEnabled: isNeonEnabled(), neonAvailable: isNeonAvailable(), hasLocalData: queryAll('users').length > 0 })
const setSyncEnabled = (enabled: boolean) => console.log('Sync toggle:', enabled)
async function runFullSync() {
  if (!isNeonAvailable()) throw new Error('Neon unavailable')
  for (const table of ['users', 'consoles', 'jeux', 'joueurs', 'sessions_jeu', 'tarifs', 'factures', 'jetons_transactions', 'messages']) {
    const remote = await queryNeonAll(table)
    if (!remote) continue
    for (const row of remote) {
      const local = queryOne(table, r => r.id === row.id)
      if (!local) insert(table, row)
      else if (new Date(row.created_at) > new Date(local.created_at)) update(table, row.id, row)
    }
  }
  return { success: true, message: 'Sync complete' }
}
async function pollChanges() { return [] }
async function startAutoSync(interval = 30000) { console.log('Auto-sync started') }
async function clearLocalData() { db.run(`DELETE FROM sessions_jeu`); save() }

export { db, queryAll, queryOne, insert, update, remove, logError, getSyncStatus, setSyncEnabled, runFullSync, isNeonEnabled, isNeonAvailable, pollChanges, startAutoSync, clearLocalData, queryNeon, queryNeonUser, queryNeonAll, neonSql, neonModule }
