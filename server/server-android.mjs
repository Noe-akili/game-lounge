import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import dotenv from 'dotenv'

const __dirname = dirname(fileURLToPath(import.meta.url))

// Load .env
dotenv.config({ path: join(__dirname, '.env') })

// Force IPv4 for Neon connection
try {
  const { Agent, setGlobalDispatcher } = await import('undici')
  setGlobalDispatcher(new Agent({ connect: { family: 4 } }))
} catch {}

// Import and start the bundled server
await import('./index.js')

console.log('Node.js server started on Android')
