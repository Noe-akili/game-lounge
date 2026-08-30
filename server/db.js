import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs'
import { fileURLToPath } from 'url'
import { dirname, join } from 'path'

const __dirname = dirname(fileURLToPath(import.meta.url))
const DATA_DIR = join(__dirname, 'data')
const DB_PATH = join(DATA_DIR, 'db.json')

if (!existsSync(DATA_DIR)) mkdirSync(DATA_DIR, { recursive: true })

let data = existsSync(DB_PATH)
  ? JSON.parse(readFileSync(DB_PATH, 'utf8'))
  : { users: [], consoles: [], jeux: [], joueurs: [], tarifs: [], sessions_jeu: [], factures: [], lignes_facture: [], jetons_transactions: [], parametres_fidelite: [] }

let _nextId = {}
function nextId(table) {
  if (!_nextId[table]) {
    _nextId[table] = (data[table]?.length || 0) > 0 ? Math.max(...data[table].map(r => r.id || 0)) + 1 : 1
  }
  return _nextId[table]++
}

function save() {
  writeFileSync(DB_PATH, JSON.stringify(data, null, 2))
}

export function queryAll(table, filterFn) {
  const rows = data[table] || []
  return filterFn ? rows.filter(filterFn) : [...rows]
}

export function queryOne(table, filterFn) {
  return (data[table] || []).find(filterFn) || null
}

export function insert(table, record) {
  if (!data[table]) data[table] = []
  const row = { id: nextId(table), ...record }
  data[table].push(row)
  save()
  return row
}

export function update(table, id, updates) {
  const rows = data[table] || []
  const idx = rows.findIndex(r => r.id === id)
  if (idx === -1) return null
  rows[idx] = { ...rows[idx], ...updates }
  save()
  return rows[idx]
}

export function remove(table, id) {
  data[table] = (data[table] || []).filter(r => r.id !== id)
  save()
}

export function query(table, conditions = {}) {
  return (data[table] || []).filter(row => {
    return Object.entries(conditions).every(([key, val]) => row[key] === val)
  })
}

export function raw(table) {
  return data[table] || []
}

export function getData() { return data }
export { save }
