import { build } from 'esbuild'
import { existsSync, mkdirSync, cpSync, writeFileSync, rmSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

const outdir = 'server-dist'

// Clean previous build
if (existsSync(outdir)) rmSync(outdir, { recursive: true, force: true })
mkdirSync(outdir, { recursive: true })

// Bundle server with all deps included
await build({
  entryPoints: ['server/server.ts'],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  outfile: `${outdir}/server.js`,
  target: 'node20',
  minify: false,
  sourcemap: false,
  external: [],
  // Make sure sql.js wasm can be found
  loader: { '.wasm': 'binary' }
})

console.log('Server bundled to', `${outdir}/server.js`)
console.log(`Size: ${(statSync(`${outdir}/server.js`).size / 1024 / 1024).toFixed(1)}MB`)

// Copy sql.js wasm file
const wasmPaths = [
  'node_modules/sql.js/dist/sql-wasm.wasm',
  'server/node_modules/sql.js/dist/sql-wasm.wasm'
]

let wasmCopied = false
for (const wp of wasmPaths) {
  if (existsSync(wp)) {
    cpSync(wp, join(outdir, 'sql-wasm.wasm'))
    console.log(`Copied sql-wasm.wasm from ${wp}`)
    wasmCopied = true
    break
  }
}

if (!wasmCopied) {
  console.warn('WARNING: sql-wasm.wasm not found, server may not start')
}

// Copy .env if exists
if (existsSync('.env')) {
  cpSync('.env', join(outdir, '.env'))
  console.log('Copied .env')
} else {
  writeFileSync(join(outdir, '.env'), "DATABASE_URL=''\nPORT=3001\n")
}

// Create package.json for the bundled server
writeFileSync(join(outdir, 'package.json'), JSON.stringify({
  name: 'game-lounge-server',
  version: '1.0.0',
  main: 'server.js',
  type: 'commonjs'
}, null, 2))

console.log('\nBuild complete!')
console.log(`Output directory: ${outdir}/`)