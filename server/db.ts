import initSqlJs from 'sql.js'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import dotenv from 'dotenv'

let nodemailer: any = null
try { nodemailer = await import('nodemailer') } catch {}

// Force IPv4 for Neon since Termux/Proot can't connect via IPv6
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
      CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, email TEXT UNIQUE, password_hash TEXT, nom TEXT, role TEXT DEFAULT 'employe', created_at TEXT);
      CREATE TABLE IF NOT EXISTS consoles (id INTEGER PRIMARY KEY, nom TEXT, type_console TEXT, etat TEXT DEFAULT 'disponible', date_ajout TEXT);
      CREATE TABLE IF NOT EXISTS jeux (id INTEGER PRIMARY KEY, titre TEXT, genre TEXT, console_id INTEGER, actif INTEGER DEFAULT 1, created_at TEXT);
      CREATE TABLE IF NOT EXISTS joueurs (id INTEGER PRIMARY KEY, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TEXT);
      CREATE TABLE IF NOT EXISTS sessions_jeu (id INTEGER PRIMARY KEY, console_id INTEGER, joueur_id INTEGER, jeu_id INTEGER, employe_id INTEGER, debut TEXT, fin TEXT, duree_minutes INTEGER, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TEXT);
      CREATE TABLE IF NOT EXISTS tarifs (id INTEGER PRIMARY KEY, nom TEXT, type_tarif TEXT, prix INTEGER, duree_minutes INTEGER, created_at TEXT);
      CREATE TABLE IF NOT EXISTS factures (id INTEGER PRIMARY KEY, numero_facture TEXT UNIQUE, session_id INTEGER, joueur_id INTEGER, montant_ht REAL, taux_tva REAL, montant_tva REAL, montant_ttc REAL, mode_paiement TEXT, statut TEXT, date_paiement TEXT, created_at TEXT);
      CREATE TABLE IF NOT EXISTS jetons_transactions (id INTEGER PRIMARY KEY, joueur_id INTEGER, quantite INTEGER, type_transaction TEXT, description TEXT, created_at TEXT);
      CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, titre TEXT, contenu TEXT, auteur TEXT, created_at TEXT);
    `)
    // No local seeding — all data comes from Neon and is cached locally for offline use only
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
const hasNeon = !!(process.env.DATABASE_URL)
if (hasNeon) {
  try {
    const { neon } = await import('@neondatabase/serverless')
    neonSql = neon(process.env.DATABASE_URL)
    console.log('✅ Neon connected')
  } catch (e) { console.warn('⚠️ Neon init failed:', e.message) }
}
const isNeonAvailable = () => hasNeon && !!neonSql
const isNeonEnabled = () => isNeonAvailable()

// Wrap neon query with timeout to avoid hanging on bad network
function withTimeout(promise, ms = 8000, label = 'neon query') {
  return Promise.race([
    promise,
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms))
  ])
}
async function safeNeon(sql, params = [], label = 'neon') {
  try {
    return await withTimeout(neonSql(sql, params), 8000, label)
  } catch (e) {
    console.warn(`  ⚠️ ${label}:`, e.message?.slice(0, 100))
    return null
  }
}

async function ensureNeonSchema() {
  if (!neonSql) return
  console.log('🌱 ensureNeonSchema starting...')
  try {
    // Create schemas (non-blocking, ignore errors if exists)
    const schemas = [
      `CREATE TABLE IF NOT EXISTS users (id SERIAL PRIMARY KEY, email TEXT UNIQUE, password_hash TEXT, nom TEXT, role TEXT DEFAULT 'employe', created_at TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS consoles (id SERIAL PRIMARY KEY, nom TEXT, type_console TEXT, etat TEXT DEFAULT 'disponible', date_ajout TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS jeux (id SERIAL PRIMARY KEY, titre TEXT, genre TEXT, console_id INTEGER, actif BOOLEAN DEFAULT TRUE, created_at TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS joueurs (id SERIAL PRIMARY KEY, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS sessions_jeu (id SERIAL PRIMARY KEY, console_id INTEGER, joueur_id INTEGER, jeu_id INTEGER, employe_id INTEGER, debut TIMESTAMPTZ, fin TIMESTAMPTZ, duree_minutes INTEGER, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS tarifs (id SERIAL PRIMARY KEY, nom TEXT, type_tarif TEXT, prix INTEGER, duree_minutes INTEGER, created_at TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS factures (id SERIAL PRIMARY KEY, numero_facture TEXT UNIQUE, session_id INTEGER, joueur_id INTEGER, montant_ht REAL, taux_tva REAL DEFAULT 20, montant_tva REAL, montant_ttc REAL, mode_paiement TEXT, statut TEXT, date_paiement TIMESTAMPTZ, created_at TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS jetons_transactions (id SERIAL PRIMARY KEY, joueur_id INTEGER, quantite INTEGER, type_transaction TEXT, description TEXT, created_at TIMESTAMPTZ DEFAULT NOW())`,
      `CREATE TABLE IF NOT EXISTS messages (id SERIAL PRIMARY KEY, titre TEXT, contenu TEXT, auteur TEXT, created_at TIMESTAMPTZ DEFAULT NOW())`,
    ]
    for (const s of schemas) {
      await safeNeon(s, [], 'create-schema')
    }
    console.log('  schemas done')

    // Check & seed admin user
    const existingUsers = await safeNeon(`SELECT id FROM users LIMIT 1`, [], 'check-users')
    if (!existingUsers || existingUsers.length === 0) {
      console.log('  Seeding admin user...')
      const bcrypt = await import('bcryptjs')
      const hash = bcrypt.hashSync('admin1234', 10)
      await safeNeon(`INSERT INTO users (email, password_hash, nom, role) VALUES ($1, $2, $3, $4)`,
        ['admin@gamelounge.com', hash, 'Administrateur', 'admin'], 'insert-admin')
      console.log('  admin seeded')
    } else {
      console.log('  users already exist, skip seed')
    }

    // Check & seed consoles
    const existingConsoles = await safeNeon(`SELECT id FROM consoles LIMIT 1`, [], 'check-consoles')
    if (!existingConsoles || existingConsoles.length === 0) {
      console.log('  Seeding consoles...')
      for (const c of [['PS5 - Poste 1','PS5'],['PS5 - Poste 2','PS5'],['PS4 - Poste 3','PS4'],['PS4 - Poste 4','PS4'],['PS5 - Poste 5','PS5'],['PS4 - Poste 6','PS4']]) {
        await safeNeon(`INSERT INTO consoles (nom, type_console) VALUES ($1, $2)`, c, 'insert-console')
      }
      console.log('  consoles seeded')
    } else {
      console.log('  consoles already exist, skip seed')
    }

    // Check & seed tarifs
    const existingTarifs = await safeNeon(`SELECT id FROM tarifs LIMIT 1`, [], 'check-tarifs')
    if (!existingTarifs || existingTarifs.length === 0) {
      console.log('  Seeding tarifs...')
      await safeNeon(`INSERT INTO tarifs (nom, type_tarif, prix, duree_minutes) VALUES ('Standard', 'heure', 2000, 60)`, [], 'insert-tarif')
      console.log('  tarifs seeded')
    } else {
      console.log('  tarifs already exist, skip seed')
    }

    // Sync to local SQLite
    try {
      const neonConsoles = await safeNeon(`SELECT * FROM consoles`, [], 'sync-consoles')
      if (neonConsoles) {
        for (const c of neonConsoles) {
          db.run(`INSERT OR IGNORE INTO consoles (id, nom, type_console, etat, date_ajout) VALUES (?,?,?,?,?)`,
            [c.id, c.nom, c.type_console, 'disponible', c.date_ajout])
        }
      }
      const neonTarifs = await safeNeon(`SELECT * FROM tarifs`, [], 'sync-tarifs')
      if (neonTarifs) {
        for (const t of neonTarifs) {
          db.run(`INSERT OR IGNORE INTO tarifs (id, nom, type_tarif, prix, duree_minutes, created_at) VALUES (?,?,?,?,?,?)`,
            [t.id, t.nom, t.type_tarif, t.prix, t.duree_minutes, t.created_at])
        }
      }
      save()
      console.log('  local synced')
    } catch (e) { console.warn('  ⚠️ local sync error:', e.message) }

    console.log('✅ Neon schema ready')
  } catch (e) {
    console.error('❌ ensureNeonSchema failed:', e.message)
  }
}

// Run schema init in background (don't block server startup)
setTimeout(() => { ensureNeonSchema().catch(e => console.warn('⚠️ ensureNeonSchema error:', e.message)) }, 100)

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
  // If data has an explicit id, use it; otherwise omit so AUTOINCREMENT assigns
  const keys = Object.keys(data).filter(k => k !== 'id')
  const vals = keys.map(k => data[k])
  const q = data.id != null
    ? `INSERT OR REPLACE INTO ${table} (id, ${keys.join(',')}) VALUES (?, ${keys.map(() => '?').join(',')})`
    : `INSERT INTO ${table} (${keys.join(',')}) VALUES (${keys.map(() => '?').join(',')})`
  const params = data.id != null ? [data.id, ...vals] : vals
  db.run(q, params)
  const id = data.id ?? db.exec(`SELECT last_insert_rowid() as id`)[0]?.values[0]?.[0]
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
  const r = await neonSql(sql, params)
  return r
}
async function queryNeonUser(email: string) {
  if (!neonSql) return null
  const rows = await neonSql(`SELECT * FROM users WHERE email=$1`, [email])
  return rows?.[0] || null
}
async function queryNeonAll(table: string) {
  if (!neonSql) return null
  return await neonSql(`SELECT * FROM ${table}`)
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
