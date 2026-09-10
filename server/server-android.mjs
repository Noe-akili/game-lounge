import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { loadDotEnv } from './utils/native.ts'

const __dirname = dirname(fileURLToPath(import.meta.url))

// Load .env
loadDotEnv(__dirname)

// Force IPv4 for Neon connection
import dns from 'node:dns'
dns.setDefaultResultOrder('ipv4first')

// Import and start the bundled server
await import('./index.js')

console.log('Node.js server started on Android')
