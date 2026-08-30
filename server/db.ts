import { DatabaseSync } from 'node:sqlite'
import { existsSync, mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import dotenv from 'dotenv'
let nodemailer: any = null
try { nodemailer = await import('nodemailer') } catch {}

// Load env
try {
  const envDir = process.env.DATA_DIR || (typeof __dirname !== 'undefined' ? __dirname : '.')
  dotenv.config({ path: join(envDir, '.env') })
} catch {}
dotenv.config()

const DATA_DIR = process.env.DATA_DIR || join('.', 'data')
const DB_PATH = join(DATA_DIR, 'app.db')

if (!existsSync(DATA_DIR)) mkdirSync(DATA_DIR, { recursive: true })

// SQLite - Base de données PRIMAIRE (toujours disponible)
let sqliteDb: InstanceType<typeof DatabaseSync> | null = null
function getSqliteDb() {
  if (!sqliteDb) {
    sqliteDb = new DatabaseSync(DB_PATH)
    try { sqliteDb.exec('PRAGMA journal_mode = WAL') } catch {}
    sqliteDb.exec(`
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  email TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('admin','employe')),
  nom TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS consoles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  nom TEXT NOT NULL,
  type TEXT NOT NULL,
  poste_numero INTEGER NOT NULL,
  etat TEXT NOT NULL DEFAULT 'disponible',
  session_id INTEGER,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS jeux (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  titre TEXT NOT NULL,
  genre TEXT,
  console_id INTEGER,
  jaquette_url TEXT,
  actif INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS joueurs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  nom TEXT NOT NULL,
  telephone TEXT,
  email TEXT,
  jetons_solde INTEGER NOT NULL DEFAULT 0,
  date_inscription TEXT NOT NULL,
  derniere_visite TEXT
);
CREATE TABLE IF NOT EXISTS tarifs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL,
  duree_minutes INTEGER NOT NULL,
  prix INTEGER NOT NULL,
  description TEXT,
  actif INTEGER NOT NULL DEFAULT 1,
  console_type TEXT,
  jeu TEXT
);
CREATE TABLE IF NOT EXISTS sessions_jeu (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  console_id INTEGER NOT NULL,
  joueur_id INTEGER NOT NULL,
  jeu_id INTEGER NOT NULL,
  employe_id INTEGER NOT NULL,
  tarif_id INTEGER,
  debut TEXT NOT NULL,
  fin TEXT,
  duree_minutes INTEGER NOT NULL DEFAULT 0,
  montant INTEGER NOT NULL DEFAULT 0,
  tarif_prix INTEGER,
  jetons_gagnes INTEGER NOT NULL DEFAULT 0,
  statut TEXT NOT NULL CHECK(statut IN ('en_cours','pause','terminee','annulee')),
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS factures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  numero_facture TEXT UNIQUE NOT NULL,
  session_id INTEGER NOT NULL,
  joueur_id INTEGER NOT NULL,
  montant_ht INTEGER NOT NULL,
  taux_tva INTEGER NOT NULL,
  montant_tva INTEGER NOT NULL,
  montant_ttc INTEGER NOT NULL,
  mode_paiement TEXT NOT NULL DEFAULT 'especes',
  statut TEXT NOT NULL DEFAULT 'payee',
  date_paiement TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS lignes_facture (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  facture_id INTEGER NOT NULL,
  description TEXT NOT NULL,
  quantite INTEGER NOT NULL,
  prix_unitaire INTEGER NOT NULL,
  total_ligne INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS jetons_transactions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  joueur_id INTEGER NOT NULL,
  type TEXT NOT NULL CHECK(type IN ('gain','depense','bonus')),
  quantite INTEGER NOT NULL,
  raison TEXT,
  session_id INTEGER,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS parametres_fidelite (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  regle_type TEXT NOT NULL,
  seuil INTEGER NOT NULL,
  jetons_attribues INTEGER NOT NULL,
  actif INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS error_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  message TEXT NOT NULL,
  stack TEXT,
  endpoint TEXT,
  method TEXT,
  created_at TEXT NOT NULL,
  sent INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  titre TEXT,
  contenu TEXT NOT NULL,
  auteur TEXT,
  created_at TEXT NOT NULL
);
`)
    // Migration: add tarif_id column if missing
    try { sqliteDb.exec(`ALTER TABLE sessions_jeu ADD COLUMN tarif_id INTEGER`) } catch {}
  }
  return sqliteDb
}

// Email configuration for error reporting
const EMAIL_TO = 'noeakili502@gmail.com'
const EMAIL_ENABLED = process.env.SMTP_HOST && process.env.SMTP_USER && process.env.SMTP_PASS

let emailTransporter: nodemailer.Transporter | null = null

if (EMAIL_ENABLED) {
  try {
    emailTransporter = nodemailer.createTransport({
      host: process.env.SMTP_HOST,
      port: Number(process.env.SMTP_PORT) || 587,
      secure: process.env.SMTP_SECURE === 'true',
      auth: {
        user: process.env.SMTP_USER,
        pass: process.env.SMTP_PASS,
      },
    })
    console.log('📧 Email configuré pour envoi d\'erreurs')
  } catch (e) {
    console.warn('⚠️ Email non configuré, erreurs en local uniquement')
  }
}

// Log error to SQLite and send email if online
export async function logError(error: Error, endpoint?: string, method?: string) {
  const db = getSqliteDb()
  const stmt = db.prepare(`INSERT INTO error_logs (message, stack, endpoint, method, created_at, sent) VALUES (?, ?, ?, ?, ?, ?)`)
  const now = new Date().toISOString()
  stmt.run(error.message, error.stack || '', endpoint || '', method || '', now, 0)

  const htmlBody = `
    <div style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto;">
      <h2 style="color: #dc2626;">🔴 Erreur détectée</h2>
      <table style="width: 100%; border-collapse: collapse;">
        <tr><td style="padding: 8px; border: 1px solid #ddd; font-weight: bold;">Endpoint</td><td style="padding: 8px; border: 1px solid #ddd;">${endpoint || 'N/A'}</td></tr>
        <tr><td style="padding: 8px; border: 1px solid #ddd; font-weight: bold;">Méthode</td><td style="padding: 8px; border: 1px solid #ddd;">${method || 'N/A'}</td></tr>
        <tr><td style="padding: 8px; border: 1px solid #ddd; font-weight: bold;">Message</td><td style="padding: 8px; border: 1px solid #ddd;">${error.message}</td></tr>
        <tr><td style="padding: 8px; border: 1px solid #ddd; font-weight: bold;">Date</td><td style="padding: 8px; border: 1px solid #ddd;">${now}</td></tr>
      </table>
      <h3 style="margin-top: 20px;">Stack Trace:</h3>
      <pre style="background: #f5f5f5; padding: 15px; border-radius: 8px; overflow-x: auto; font-size: 12px;">${error.stack || 'N/A'}</pre>
    </div>
  `

  // Try to send email if transporter available
  if (emailTransporter) {
    try {
      await emailTransporter.sendMail({
        from: `"Game Lounge Erreur" <${process.env.SMTP_USER}>`,
        to: EMAIL_TO,
        subject: `🔴 Erreur Game Lounge - ${endpoint || 'Inconnu'}`,
        html: htmlBody,
      })
      db.prepare(`UPDATE error_logs SET sent = 1 WHERE message = ? AND created_at = ?`).run(error.message, now)
      console.log(`📧 Erreur envoyée par email à ${EMAIL_TO}`)
    } catch (emailErr) {
      console.warn('⚠️ Envoi email échoué:', (emailErr as Error).message)
    }
  } else if (process.env.SMTP_USER) {
    // Fallback: try to send via fetch (useful in bundled APK mode without nodemailer)
    try {
      const nodemailerMod = await import('nodemailer')
      const transport = nodemailerMod.createTransport({
        host: process.env.SMTP_HOST || 'smtp.gmail.com',
        port: Number(process.env.SMTP_PORT) || 587,
        secure: process.env.SMTP_SECURE === 'true',
        auth: { user: process.env.SMTP_USER, pass: process.env.SMTP_PASS },
      })
      await transport.sendMail({
        from: `"Game Lounge Erreur" <${process.env.SMTP_USER}>`,
        to: EMAIL_TO,
        subject: `🔴 Erreur Game Lounge - ${endpoint || 'Inconnu'}`,
        html: htmlBody,
      })
      db.prepare(`UPDATE error_logs SET sent = 1 WHERE message = ? AND created_at = ?`).run(error.message, now)
      console.log(`📧 Erreur envoyée par email (fallback) à ${EMAIL_TO}`)
    } catch (fallbackErr) {
      console.warn('⚠️ Envoi email fallback échoué:', (fallbackErr as Error).message)
    }
  }
}

// Neon sync - Secondaire (optionnel, seulement si connecté)
let neonSql: any = null
let useNeon = false

const DATABASE_URL = process.env.DATABASE_URL || ''

if (DATABASE_URL) {
  try {
    const { neon } = await import('@neondatabase/serverless')
    neonSql = neon(DATABASE_URL)
    useNeon = true
    console.log('🌐 Neon DB configurée - synchronisation secondaire activée')
  } catch (e) {
    console.warn('⚠️ Neon init failed - mode hors ligne uniquement')
    useNeon = false
  }
} else {
  console.log('📁 Mode hors ligne - SQLite local uniquement')
}

async function syncToNeon(table: string, operation: string, data?: any) {
  if (!useNeon || !neonSql) return
  try {
    if (operation === 'insert' && data) {
      const cols = Object.keys(data).filter(k => k !== 'id')
      const vals = cols.map(c => data[c])
      const placeholders = cols.map((_, i) => `$${i + 1}`).join(', ')
      await neonSql(`INSERT INTO ${table} (${cols.join(', ')}) VALUES (${placeholders}) ON CONFLICT DO NOTHING`, vals.map(String))
    } else if (operation === 'update' && data) {
      const { id, ...updates } = data
      const cols = Object.keys(updates)
      if (cols.length > 0) {
        const setClause = cols.map((c, i) => `${c} = $${i + 1}`).join(', ')
        await neonSql(`UPDATE ${table} SET ${setClause} WHERE id = $${cols.length + 1}`, [...cols.map(c => String(updates[c])), String(id)])
      }
    } else if (operation === 'delete' && data) {
      await neonSql(`DELETE FROM ${table} WHERE id = $1`, [String(data.id)])
    }
  } catch (e) {
    console.warn(`Neon sync ${table} ${operation} failed`)
  }
}

// Try to initialize Neon tables in background
if (useNeon && neonSql) {
  (async () => {
    try {
      await neonSql(`CREATE TABLE IF NOT EXISTS users (id SERIAL PRIMARY KEY, email TEXT UNIQUE NOT NULL, password_hash TEXT NOT NULL, role TEXT NOT NULL, nom TEXT NOT NULL, created_at TEXT NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS consoles (id SERIAL PRIMARY KEY, nom TEXT NOT NULL, type TEXT NOT NULL, poste_numero INTEGER NOT NULL, etat TEXT NOT NULL DEFAULT 'disponible', session_id INTEGER, created_at TEXT NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS jeux (id SERIAL PRIMARY KEY, titre TEXT NOT NULL, genre TEXT, console_id INTEGER, jaquette_url TEXT, actif INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS joueurs (id SERIAL PRIMARY KEY, nom TEXT NOT NULL, telephone TEXT, email TEXT, jetons_solde INTEGER NOT NULL DEFAULT 0, date_inscription TEXT NOT NULL, derniere_visite TEXT)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS tarifs (id SERIAL PRIMARY KEY, type TEXT NOT NULL, duree_minutes INTEGER NOT NULL, prix INTEGER NOT NULL, description TEXT, actif INTEGER NOT NULL DEFAULT 1, console_type TEXT, jeu TEXT)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS sessions_jeu (id SERIAL PRIMARY KEY, console_id INTEGER NOT NULL, joueur_id INTEGER NOT NULL, jeu_id INTEGER NOT NULL, employe_id INTEGER NOT NULL, debut TEXT NOT NULL, fin TEXT, duree_minutes INTEGER NOT NULL DEFAULT 0, montant INTEGER NOT NULL DEFAULT 0, tarif_prix INTEGER, jetons_gagnes INTEGER NOT NULL DEFAULT 0, statut TEXT NOT NULL, created_at TEXT NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS factures (id SERIAL PRIMARY KEY, numero_facture TEXT UNIQUE NOT NULL, session_id INTEGER NOT NULL, joueur_id INTEGER NOT NULL, montant_ht INTEGER NOT NULL, taux_tva INTEGER NOT NULL, montant_tva INTEGER NOT NULL, montant_ttc INTEGER NOT NULL, mode_paiement TEXT NOT NULL DEFAULT 'especes', statut TEXT NOT NULL DEFAULT 'payee', date_paiement TEXT NOT NULL, created_at TEXT NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS lignes_facture (id SERIAL PRIMARY KEY, facture_id INTEGER NOT NULL, description TEXT NOT NULL, quantite INTEGER NOT NULL, prix_unitaire INTEGER NOT NULL, total_ligne INTEGER NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS jetons_transactions (id SERIAL PRIMARY KEY, joueur_id INTEGER NOT NULL, type TEXT NOT NULL, quantite INTEGER NOT NULL, raison TEXT, session_id INTEGER, created_at TEXT NOT NULL)`)
      await neonSql(`CREATE TABLE IF NOT EXISTS parametres_fidelite (id SERIAL PRIMARY KEY, regle_type TEXT NOT NULL, seuil INTEGER NOT NULL, jetons_attribues INTEGER NOT NULL, actif INTEGER NOT NULL DEFAULT 1)`)
      console.log('✅ Neon tables synchronisées')
    } catch (e) {
      console.warn('⚠️ Neon init failed - mode hors ligne')
      useNeon = false
    }
  })()
}

type TableName = 'users' | 'consoles' | 'jeux' | 'joueurs' | 'tarifs' | 'sessions_jeu' | 'factures' | 'lignes_facture' | 'jetons_transactions' | 'parametres_fidelite'

function rowToJs(row: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(row)) {
    if (k === 'actif' && typeof v === 'number') {
      out[k] = v === 1 ? true : v === 0 ? false : v
    } else if (k === 'actif' && typeof v === 'boolean') {
      out[k] = v
    } else {
      out[k] = v
    }
  }
  return out
}

function jsToRow(obj: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(obj)) {
    if (k === 'actif' && typeof v === 'boolean') {
      out[k] = v ? 1 : 0
    } else {
      out[k] = v
    }
  }
  return out
}

// SQLite PRIMAIRE - tout passe par SQLite d'abord
export async function queryAll(table: TableName, filterFn?: (row: Record<string, unknown>) => boolean): Promise<Record<string, unknown>[]> {
  const db = getSqliteDb()
  const stmt = db.prepare(`SELECT * FROM ${table}`)
  const rows = stmt.all() as Record<string, unknown>[]
  const jsRows = rows.map(rowToJs)
  return filterFn ? jsRows.filter(filterFn) : jsRows
}

export async function queryOne(table: TableName, filterFn: (row: Record<string, unknown>) => boolean): Promise<Record<string, unknown> | null> {
  const all = await queryAll(table)
  return all.find(filterFn) ?? null
}

export async function query(table: TableName, conditions: Record<string, unknown> = {}): Promise<Record<string, unknown>[]> {
  const all = await queryAll(table)
  return all.filter(row => Object.entries(conditions).every(([key, val]) => row[key] === val))
}

export async function insert(table: TableName, record: Record<string, unknown>): Promise<Record<string, unknown>> {
  const clean = jsToRow({ ...record })
  delete clean.id
  const cols = Object.keys(clean)
  if (cols.length === 0) throw new Error('Aucune colonne à insérer')
  const values = cols.map(c => clean[c])

  const db = getSqliteDb()
  const placeholders = cols.map(() => '?').join(', ')
  const stmt = db.prepare(`INSERT INTO ${table} (${cols.join(', ')}) VALUES (${placeholders})`)
  const result = stmt.run(...values as unknown[])
  const id = result.lastInsertRowid as number
  const inserted = db.prepare(`SELECT * FROM ${table} WHERE id = ?`).get(id) as Record<string, unknown>

  // Sync to Neon in background (async, non-blocking)
  syncToNeon(table, 'insert', { id, ...clean }).catch(() => {})

  return rowToJs(inserted)
}

export async function update(table: TableName, id: number, updates: Record<string, unknown>): Promise<Record<string, unknown> | null> {
  const clean = jsToRow({ ...updates })
  delete clean.id
  const cols = Object.keys(clean)
  const values = cols.map(c => clean[c])

  const db = getSqliteDb()
  if (cols.length === 0) {
    const existing = db.prepare(`SELECT * FROM ${table} WHERE id = ?`).get(id) as Record<string, unknown> | undefined
    return existing ? rowToJs(existing) : null
  }
  const setClause = cols.map(c => `${c} = ?`).join(', ')
  db.prepare(`UPDATE ${table} SET ${setClause} WHERE id = ?`).run(...values as unknown[], id)
  const updated = db.prepare(`SELECT * FROM ${table} WHERE id = ?`).get(id) as Record<string, unknown> | undefined

  // Sync to Neon in background
  syncToNeon(table, 'update', { id, ...clean }).catch(() => {})

  return updated ? rowToJs(updated) : null
}

export async function remove(table: TableName, id: number): Promise<void> {
  const db = getSqliteDb()
  db.prepare(`DELETE FROM ${table} WHERE id = ?`).run(id)

  // Sync to Neon in background
  syncToNeon(table, 'delete', { id }).catch(() => {})
}

export async function raw(table: TableName): Promise<Record<string, unknown>[]> {
  return queryAll(table)
}

export async function getData(): Promise<Record<string, Record<string, unknown>[]>> {
  const [users, consoles, jeux, joueurs, tarifs, sessions_jeu, factures, lignes_facture, jetons_transactions, parametres_fidelite] = await Promise.all([
    queryAll('users'),
    queryAll('consoles'),
    queryAll('jeux'),
    queryAll('joueurs'),
    queryAll('tarifs'),
    queryAll('sessions_jeu'),
    queryAll('factures'),
    queryAll('lignes_facture'),
    queryAll('jetons_transactions'),
    queryAll('parametres_fidelite'),
  ])
  return { users, consoles, jeux, joueurs, tarifs, sessions_jeu, factures, lignes_facture, jetons_transactions, parametres_fidelite }
}

export function save(): void {}

export function getDb() {
  return getSqliteDb()
}

export function isNeonEnabled(): boolean {
  return useNeon
}

// ===== BIDIRECTIONAL SYNC =====
let syncEnabled = false
let lastSyncAt: string | null = null
let syncInProgress = false

const SYNC_TABLES: TableName[] = ['users', 'consoles', 'jeux', 'joueurs', 'tarifs', 'sessions_jeu', 'factures', 'lignes_facture', 'jetons_transactions', 'parametres_fidelite']

export function isSyncEnabled(): boolean { return syncEnabled && useNeon }
export function setSyncEnabled(v: boolean) { syncEnabled = v }
export function getSyncStatus() {
  return { enabled: syncEnabled, neonConnected: useNeon, lastSync: lastSyncAt, syncing: syncInProgress }
}

async function pullTableFromNeon(table: TableName): Promise<number> {
  if (!neonSql) return 0
  const neonRows: any[] = await neonSql(`SELECT * FROM ${table}`)
  if (!neonRows || neonRows.length === 0) return 0

  const db = getSqliteDb()
  const sqliteRows = db.prepare(`SELECT * FROM ${table}`).all() as Record<string, unknown>[]
  const sqliteMap = new Map(sqliteRows.map(r => [r.id, r]))

  let merged = 0
  for (const nr of neonRows) {
    const local = sqliteMap.get(nr.id)
    if (!local) {
      // Existe dans Neon mais pas en local → insert
      const cols = Object.keys(nr)
      const placeholders = cols.map(() => '?').join(', ')
      const vals = cols.map(c => nr[c] === true ? 1 : nr[c] === false ? 0 : nr[c])
      try {
        db.prepare(`INSERT OR IGNORE INTO ${table} (${cols.join(', ')}) VALUES (${placeholders})`).run(...vals)
        merged++
      } catch {}
    } else {
      // Existe des deux côtés → comparer created_at, prendre le plus récent
      const neonDate = new Date(nr.created_at || 0).getTime()
      const localDate = new Date(local.created_at as string || 0).getTime()
      if (neonDate > localDate) {
        const { id, ...updates } = nr
        const cols = Object.keys(updates)
        if (cols.length > 0) {
          const setClause = cols.map(c => `${c} = ?`).join(', ')
          const vals = cols.map(c => updates[c] === true ? 1 : updates[c] === false ? 0 : updates[c])
          try {
            db.prepare(`UPDATE ${table} SET ${setClause} WHERE id = ?`).run(...vals, id)
            merged++
          } catch {}
        }
      }
    }
  }
  return merged
}

async function pushTableToNeon(table: TableName): Promise<number> {
  if (!neonSql) return 0
  const db = getSqliteDb()
  const sqliteRows = db.prepare(`SELECT * FROM ${table}`).all() as Record<string, unknown>[]
  if (sqliteRows.length === 0) return 0

  let pushed = 0
  for (const row of sqliteRows) {
    try {
      const cols = Object.keys(row)
      const vals = cols.map(c => {
        const v = row[c]
        return v === true ? '1' : v === false ? '0' : String(v ?? '')
      })
      const placeholders = cols.map((_, i) => `$${i + 1}`).join(', ')
      await neonSql(
        `INSERT INTO ${table} (${cols.join(', ')}) VALUES (${placeholders}) ON CONFLICT (id) DO UPDATE SET ${cols.filter(c => c !== 'id').map((c, i) => `${c} = $${i + 1}`).join(', ')}`,
        vals
      )
      pushed++
    } catch {}
  }
  return pushed
}

export async function runFullSync(): Promise<{ pushed: Record<string, number>, pulled: Record<string, number>, duration: number }> {
  if (!useNeon || !neonSql) throw new Error('Neon non connecté')
  if (syncInProgress) throw new Error('Sync déjà en cours')
  syncInProgress = true
  const start = Date.now()
  const pushed: Record<string, number> = {}
  const pulled: Record<string, number> = {}

  try {
    for (const table of SYNC_TABLES) {
      pushed[table] = await pushTableToNeon(table)
      pulled[table] = await pullTableFromNeon(table)
    }
    lastSyncAt = new Date().toISOString()
  } finally {
    syncInProgress = false
  }

  return { pushed, pulled, duration: Date.now() - start }
}
