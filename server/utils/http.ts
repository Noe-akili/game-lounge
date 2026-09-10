// @ts-nocheck
// Native Node.js HTTP server + router (replaces express)
// Provides an express-compatible req/res surface so existing route handlers and
// middleware (native.ts) keep working unchanged.

import { createServer } from 'node:http'
import { existsSync, statSync, createReadStream } from 'node:fs'
import { join, resolve, normalize, extname } from 'node:path'

const MIME: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.htm': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.mjs': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.xml': 'application/xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.wasm': 'application/wasm',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.otf': 'font/otf',
  '.eot': 'application/vnd.ms-fontobject',
  '.pdf': 'application/pdf',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
  '.mp3': 'audio/mpeg',
  '.wav': 'audio/wav',
}

function pathToRegex(p: string): RegExp {
  if (p === '*' || p === '/*') return /^\/.*$/
  const parts = p.split('/').map(seg => {
    if (seg.startsWith(':')) return `(?<${seg.slice(1)}>[^/]+)`
    return seg.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  })
  return new RegExp('^' + parts.join('/') + '$')
}

// Run a list of middleware/handler functions (args..., next). Resolves with
// `true` when every function called next() through the end, `false` when a
// function finished without calling next (it either responded or silently
// deferred to continue traversal).
function chainRunner(args: any[], list: any[], res: any): Promise<boolean> {
  return new Promise((resolve, reject) => {
    let i = 0
    const step = () => {
      const fn = list[i]
      if (!fn) return resolve(true)
      i++
      let done = false
      const next = (e?: any) => {
        if (done) return
        done = true
        if (e) return reject(e)
        step()
      }
      try {
        const ret = fn(...args, next)
        if (ret && typeof ret.then === 'function') {
          ret.then(
            () => {
              if (!done && !res.headersSent) { done = true; resolve(false) }
            },
            (e: any) => { if (!done) { done = true; reject(e) } }
          )
        } else if (!done) {
          done = true
          resolve(res.headersSent)
        }
      } catch (e) {
        if (!done) { done = true; reject(e) }
      }
    }
    step()
  })
}

function readBody(req: any): Promise<void> {
  return new Promise((resolve, reject) => {
    const type = String(req.headers['content-type'] || '')
    const chunks: Buffer[] = []
    req.on('data', (c: Buffer) => chunks.push(c))
    req.on('end', () => {
      const raw = Buffer.concat(chunks).toString('utf-8')
      if (type.includes('application/json') && raw.trim()) {
        try { req.body = JSON.parse(raw) } catch {
          return reject(Object.assign(new Error('Invalid JSON'), { status: 400 }))
        }
      } else {
        req.body = {}
      }
      resolve()
    })
    req.on('error', reject)
  })
}

function attachReq(req: any, url: URL) {
  req.path = url.pathname
  req.originalUrl = req.url
  req.query = Object.fromEntries(url.searchParams as any) as Record<string, string>
  req.params = {}
  req.body = {}
  const fwd = req.headers['x-forwarded-for']
  req.ip = (fwd ? String(fwd).split(',')[0].trim() : null) || req.socket?.remoteAddress || ''
}

function attachRes(res: any) {
  res.status = (n: number) => { res.statusCode = n; return res }
  res.json = (data: any) => {
    const payload = JSON.stringify(data)
    if (!res.getHeader('Content-Type')) res.setHeader('Content-Type', 'application/json; charset=utf-8')
    res.end(payload)
  }
  res.send = (data: any) => {
    if (Buffer.isBuffer(data)) {
      if (!res.getHeader('Content-Type')) res.setHeader('Content-Type', 'application/octet-stream')
      res.end(data)
    } else if (typeof data === 'string') {
      if (!res.getHeader('Content-Type')) res.setHeader('Content-Type', 'text/html; charset=utf-8')
      res.end(data)
    } else {
      res.json(data)
    }
  }
  res.sendStatus = (code: number) => { res.statusCode = code; res.end() }
  res.sendFile = (p: string) => {
    if (existsSync(p) && statSync(p).isFile()) {
      const mime = MIME[extname(p)] || 'application/octet-stream'
      res.setHeader('Content-Type', mime)
      res.setHeader('Content-Length', String(statSync(p).size))
      createReadStream(p).pipe(res)
    } else {
      res.status(404).json({ message: 'Fichier introuvable' })
    }
  }
}

function applyNoCache(res: any, path: string) {
  res.setHeader('Cache-Control', 'no-cache, no-store, must-revalidate')
  res.setHeader('Pragma', 'no-cache')
  res.setHeader('Expires', '0')
}

async function traverse(layers: any[], req: any, res: any) {
  for (let i = 0; i < layers.length; i++) {
    const layer = layers[i]
    if (layer.type === 'error') continue
    if (layer.type === 'static') {
      await chainRunner([req, res], [layer.fn], res)
      if (res.headersSent) return
      continue
    }
    if (layer.type === 'mw') {
      await chainRunner([req, res], [layer.fn], res)
      if (res.headersSent) return
      continue
    }
    // route
    if (layer.method !== req.method) continue
    const match = req.path.match(layer.regex)
    if (!match) continue
    req.params = match.groups || {}
    await chainRunner([req, res], layer.handlers, res)
    if (res.headersSent) return
  }
}

function writeError(err: any, req: any, res: any) {
  if (res.headersSent) return
  res.statusCode = err.status || 500
  res.setHeader('Content-Type', 'application/json; charset=utf-8')
  res.end(JSON.stringify({ message: 'Erreur interne du serveur' }))
}

async function handleError(err: any, layers: any[], req: any, res: any) {
  const errLayers = layers.filter(l => l.type === 'error')
  if (!errLayers.length) { writeError(err, req, res); return }
  try {
    await chainRunner([err, req, res], errLayers, res)
  } catch (e) { writeError(e, req, res) }
}

async function handle(req: any, res: any, layers: any[]) {
  const url = new URL(req.url, 'http://localhost')
  attachReq(req, url)
  attachRes(res)
  try {
    if (req.method !== 'GET' && req.method !== 'HEAD') await readBody(req)
    await traverse(layers, req, res)
    if (!res.headersSent && !res.writableEnded) {
      res.statusCode = 404
      res.setHeader('Content-Type', 'application/json; charset=utf-8')
      res.end(JSON.stringify({ message: 'Route non trouvée' }))
    }
  } catch (err) {
    await handleError(err, layers, req, res)
  }
}

// Static file middleware (replaces express.static)
export function staticFiles(root: string, options?: any) {
  const fn = (req: any, res: any, next: any) => {
    try {
      const url = new URL(req.url, 'http://localhost')
      const pathname = decodeURIComponent(url.pathname)
      const safe = normalize(pathname).replace(/^(\.\.[/\\])+/, '')
      const filePath = resolve(root, '.' + safe)
      if (!filePath.startsWith(resolve(root))) return next()
      if (!existsSync(filePath) || !statSync(filePath).isFile()) return next()
      const mime = MIME[extname(filePath)] || 'application/octet-stream'
      applyNoCache(res, filePath)
      res.setHeader('Content-Type', mime)
      res.setHeader('Content-Length', String(statSync(filePath).size))
      createReadStream(filePath).pipe(res)
    } catch { next() }
  }
  ;(fn as any)._nativeStatic = root
  return fn
}

// Create an express-compatible app backed by node:http
export function createApp(): any {
  const layers: any[] = []

  const app: any = {}

  app.use = (...args: any[]) => {
    for (const fn of args) {
      if (typeof fn === 'function') {
        if (fn.length >= 4) layers.push({ type: 'error', fn })
        else if ((fn as any)._nativeStatic) layers.push({ type: 'static', fn })
        else layers.push({ type: 'mw', fn })
      }
    }
  }

  app.get = (p: string, ...h: any[]) => layers.push({ type: 'route', method: 'GET', regex: pathToRegex(p), handlers: h })
  app.post = (p: string, ...h: any[]) => layers.push({ type: 'route', method: 'POST', regex: pathToRegex(p), handlers: h })
  app.put = (p: string, ...h: any[]) => layers.push({ type: 'route', method: 'PUT', regex: pathToRegex(p), handlers: h })
  app.delete = (p: string, ...h: any[]) => layers.push({ type: 'route', method: 'DELETE', regex: pathToRegex(p), handlers: h })
  app.patch = (p: string, ...h: any[]) => layers.push({ type: 'route', method: 'PATCH', regex: pathToRegex(p), handlers: h })
  app.all = (p: string, ...h: any[]) => {
    for (const m of ['GET', 'POST', 'PUT', 'DELETE', 'PATCH']) {
      layers.push({ type: 'route', method: m, regex: pathToRegex(p), handlers: h })
    }
  }

  app.listen = (port: number, host: string, cb: () => void) => {
    const server = createServer((req, res) => { handle(req, res, layers) })
    return server.listen(port, host, cb)
  }

  app._layers = layers
  return app
}