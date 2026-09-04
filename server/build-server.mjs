import { build } from 'esbuild'
import { existsSync, mkdirSync, cpSync, writeFileSync, rmSync, readdirSync, statSync, readFileSync, writeFileSync as wf } from 'node:fs'
import { join } from 'node:path'

const outdir = 'dist/nodejs'

if (existsSync(outdir)) rmSync(outdir, { recursive: true })
mkdirSync(outdir, { recursive: true })

// Bundle as ESM with all dependencies (so require() inside bundled CJS deps works)
await build({
  entryPoints: ['server/server.ts'],
  bundle: true,
  platform: 'node',
  format: 'esm',
  outfile: `${outdir}/server.mjs`,
  external: [
    'sql.js',
    '@neondatabase/serverless',
    'undici',
    'dotenv',
    'express', 'cors', 'helmet', 'bcryptjs', 'jsonwebtoken', 'express-rate-limit'
  ],
  target: 'node20',
  minify: false,
  sourcemap: false
})

// CJS wrapper
const wrapperClean = `// CJS wrapper for Android Node runtime
const path = require('path')
const urlMod = require('url')
const { createRequire } = require('module')
const r2 = createRequire(__filename)
process.chdir(__dirname)
try { require('dotenv/config') } catch {}
;(async () => {
  try {
    const esmUrl = urlMod.pathToFileURL(path.join(__dirname, 'server.mjs')).href
    await import(esmUrl)
  } catch (e) { console.error('Fatal:', e); process.exit(1) }
})()
`
wf(`${outdir}/index.js`, wrapperClean)

writeFileSync(join(outdir, 'package.json'), JSON.stringify({
  name: 'game-lounge-server',
  version: '1.0.0',
  main: 'index.js'
}, null, 2))

function copyDirContents(src, dest, files = null) {
  if (!existsSync(src)) return
  mkdirSync(dest, { recursive: true })
  const items = files || readdirSync(src)
  for (const f of items) {
    const sp = join(src, f)
    const dp = join(dest, f)
    if (existsSync(sp) && statSync(sp).isDirectory()) {
      copyDirContents(sp, dp)
    } else if (existsSync(sp)) {
      cpSync(sp, dp)
    }
  }
}

const deps = ['sql.js', '@neondatabase/serverless', 'undici', 'dotenv',
  'express', 'cors', 'helmet', 'bcryptjs', 'jsonwebtoken', 'express-rate-limit']
for (const pkg of deps) {
  const srcBase = join('node_modules', pkg)
  const destBase = join(outdir, 'node_modules', pkg)
  if (existsSync(srcBase)) {
    const entries = readdirSync(srcBase).filter(f => f !== 'node_modules')
    copyDirContents(srcBase, destBase, entries)
    console.log(`  Copied ${pkg}`)
  }
}

const srcWasm = 'node_modules/sql.js/dist/sql-wasm.wasm'
const destWasm = join(outdir, 'node_modules/sql.js/dist/sql-wasm.wasm')
if (existsSync(srcWasm)) cpSync(srcWasm, destWasm)

if (existsSync('server/.env')) cpSync('server/.env', join(outdir, '.env'))
else writeFileSync(join(outdir, '.env'), "DATABASE_URL=''\n")

console.log('\nServer bundled to', outdir)
console.log(`Total size: ${(statSync(outdir).size / 1024 / 1024).toFixed(1)}MB`)
