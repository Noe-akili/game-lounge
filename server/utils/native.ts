// @ts-nocheck
// Native Node.js implementations replacing external dependencies

import { createHmac, timingSafeEqual, randomBytes, scryptSync } from 'node:crypto'
import { readFileSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

// ===== NATIVE CORS =====
export function cors(options?: { origin?: string | string[] | boolean }) {
  const allowedOrigins = options?.origin ?? '*'
  return function corsMiddleware(req, res, next) {
    const origin = req.headers.origin
    if (allowedOrigins === '*') {
      res.setHeader('Access-Control-Allow-Origin', '*')
    } else if (typeof allowedOrigins === 'string' && origin === allowedOrigins) {
      res.setHeader('Access-Control-Allow-Origin', origin)
    } else if (Array.isArray(allowedOrigins) && origin && allowedOrigins.includes(origin)) {
      res.setHeader('Access-Control-Allow-Origin', origin)
    }
    res.setHeader('Access-Control-Allow-Methods', 'GET,POST,PUT,DELETE,PATCH,OPTIONS')
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type,Authorization,X-Requested-With,Accept,Origin')
    res.setHeader('Access-Control-Allow-Credentials', 'true')
    res.setHeader('Access-Control-Max-Age', '86400')
    if (req.method === 'OPTIONS') {
      return res.sendStatus(204)
    }
    next()
  }
}

// ===== NATIVE HELMET =====
export function helmet() {
  return function helmetMiddleware(req, res, next) {
    res.setHeader('X-Content-Type-Options', 'nosniff')
    res.setHeader('X-Frame-Options', 'DENY')
    res.setHeader('X-XSS-Protection', '1; mode=block')
    res.setHeader('Referrer-Policy', 'strict-origin-when-cross-origin')
    res.setHeader('X-DNS-Prefetch-Control', 'off')
    res.setHeader('Strict-Transport-Security', 'max-age=31536000; includeSubDomains')
    res.setHeader('Permissions-Policy', 'camera=(), microphone=(), geolocation=(), payment=()')
    res.removeHeader('X-Powered-By')
    next()
  }
}

// ===== NATIVE RATE LIMITER =====
export function rateLimit(options: { windowMs: number; max: number; message?: any }) {
  const { windowMs, max, message } = options
  const hits = new Map<string, number[]>()

  const cleanup = () => {
    const now = Date.now()
    for (const [key, timestamps] of hits) {
      const valid = timestamps.filter(t => now - t < windowMs)
      if (valid.length === 0) hits.delete(key)
      else hits.set(key, valid)
    }
  }
  const timer = setInterval(cleanup, 60000)
  if (timer.unref) timer.unref()

  return function rateLimitMiddleware(req, res, next) {
    const key = req.ip || req.socket.remoteAddress || 'unknown'
    const now = Date.now()
    const timestamps = (hits.get(key) || []).filter(t => now - t < windowMs)
    if (timestamps.length >= max) {
      return res.status(429).json(message || { message: 'Trop de tentatives, réessayez plus tard' })
    }
    timestamps.push(now)
    hits.set(key, timestamps)
    res.setHeader('RateLimit-Limit', String(max))
    res.setHeader('RateLimit-Remaining', String(max - timestamps.length))
    res.setHeader('RateLimit-Reset', String(Math.ceil((now + windowMs) / 1000)))
    next()
  }
}

// ===== NATIVE JWT (node:crypto HMAC-SHA256) =====
function base64urlEncode(data: string | Buffer): string {
  return (typeof data === 'string' ? Buffer.from(data, 'utf-8') : data).toString('base64url')
}

function base64urlDecode(str: string): string {
  return Buffer.from(str, 'base64url').toString('utf-8')
}

export const jwt = {
  sign(payload: any, secret: string, options?: { expiresIn?: string }): string {
    const header = { alg: 'HS256', typ: 'JWT' }
    const now = Math.floor(Date.now() / 1000)
    const tokenPayload = { ...payload, iat: now }
    if (options?.expiresIn) {
      const match = options.expiresIn.match(/^(\d+)([smhd])$/)
      if (match) {
        const val = parseInt(match[1])
        const unit = match[2]
        const multipliers: Record<string, number> = { s: 1, m: 60, h: 3600, d: 86400 }
        tokenPayload.exp = now + val * multipliers[unit]
      }
    }
    const headerB64 = base64urlEncode(JSON.stringify(header))
    const payloadB64 = base64urlEncode(JSON.stringify(tokenPayload))
    const signature = createHmac('sha256', secret).update(headerB64 + '.' + payloadB64).digest('base64url')
    return headerB64 + '.' + payloadB64 + '.' + signature
  },

  verify(token: string, secret: string): any {
    const parts = token.split('.')
    if (parts.length !== 3) throw new Error('Invalid token format')
    const [headerB64, payloadB64, signature] = parts
    const expectedSig = createHmac('sha256', secret).update(headerB64 + '.' + payloadB64).digest('base64url')
    const sigBuf = Buffer.from(signature, 'base64url')
    const expectedBuf = Buffer.from(expectedSig, 'base64url')
    if (sigBuf.length !== expectedBuf.length || !timingSafeEqual(sigBuf, expectedBuf)) {
      throw new Error('Invalid token signature')
    }
    const payload = JSON.parse(base64urlDecode(payloadB64))
    if (payload.exp && Math.floor(Date.now() / 1000) > payload.exp) {
      throw new Error('Token expired')
    }
    return payload
  },

  decode(token: string): any {
    const parts = token.split('.')
    if (parts.length !== 3) return null
    try { return JSON.parse(base64urlDecode(parts[1])) } catch { return null }
  }
}

// ===== NATIVE .env PARSER (replaces dotenv) =====
const __file = typeof __filename !== 'undefined' ? __filename : fileURLToPath(import.meta.url)
const __dir = dirname(__file)

function parseEnvFile(content: string) {
  for (const line of content.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const eqIdx = trimmed.indexOf('=')
    if (eqIdx === -1) continue
    const key = trimmed.slice(0, eqIdx).trim()
    let value = trimmed.slice(eqIdx + 1).trim()
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1)
    }
    if (!process.env[key]) {
      process.env[key] = value
    }
  }
}

export function loadDotEnv(dir?: string) {
  const base = dir || __dir
  const paths = [
    join(base, '.env'),
    join(base, '..', '.env'),
    join(base, '..', '..', '.env'),
  ]
  for (const p of paths) {
    try {
      if (existsSync(p)) {
        parseEnvFile(readFileSync(p, 'utf-8'))
        return
      }
    } catch {}
  }
}

// ===== NATIVE PASSWORD HASHING (replaces bcryptjs, uses node:crypto scrypt) =====
const SCRYPT_KEYLEN = 64
const SCRYPT_COST = 16384
const SCRYPT_BLOCK_SIZE = 8
const SCRYPT_PARALLELIZATION = 1

const SCRYPT_PREFIX = 'scrypt$v1$'

export function isScryptHash(hash: string | null | undefined): boolean {
  return !!hash && hash.startsWith(SCRYPT_PREFIX)
}

export function hashPassword(password: string): string {
  const salt = randomBytes(16).toString('base64url')
  const derivedKey = scryptSync(password, salt, SCRYPT_KEYLEN, {
    N: SCRYPT_COST,
    r: SCRYPT_BLOCK_SIZE,
    p: SCRYPT_PARALLELIZATION
  })
  return `${SCRYPT_PREFIX}${salt}$${derivedKey.toString('base64url')}`
}

// Lazy fallback for legacy bcrypt hashes ($2a$/$2b$/$2y$) so existing accounts
// keep working after the switch to native scrypt. On first successful login the
// legacy hash is upgraded to scrypt by the caller.
let bcryptLegacy: any = null
async function loadBcryptLegacy() {
  if (bcryptLegacy) return bcryptLegacy
  try {
    const mod: any = await import('bcryptjs')
    bcryptLegacy = mod?.default || mod
  } catch {
    bcryptLegacy = false
  }
  return bcryptLegacy
}

export async function comparePassword(password: string, stored: string): Promise<boolean> {
  if (!password || !stored) return false
  if (isScryptHash(stored)) {
    const parts = stored.split('$')
    if (parts.length !== 4) return false
    const salt = parts[2]
    const hashB64 = parts[3]
    try {
      const derivedKey = scryptSync(password, salt, SCRYPT_KEYLEN, {
        N: SCRYPT_COST,
        r: SCRYPT_BLOCK_SIZE,
        p: SCRYPT_PARALLELIZATION
      })
      const expected = Buffer.from(hashB64, 'base64url')
      const actual = Buffer.from(derivedKey.toString('base64url'), 'base64url')
      return expected.length === actual.length && timingSafeEqual(expected, actual)
    } catch { return false }
  }
  // Legacy bcrypt hash — verify via the optional fallback
  try {
    const b = await loadBcryptLegacy()
    if (b) return b.compareSync(password, stored)
  } catch {}
  return false
}
