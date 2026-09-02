import { build } from 'esbuild'
import { existsSync, mkdirSync, cpSync, writeFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'

const outdir = 'dist/nodejs'

// Clean
if (existsSync(outdir)) rmSync(outdir, { recursive: true })
mkdirSync(outdir, { recursive: true })

// Bundle server
await build({
  entryPoints: ['server/server.ts'],
  bundle: true,
  platform: 'node',
  format: 'esm',
  outfile: `${outdir}/index.js`,
  banner: {
    js: `import { createRequire } from 'node:module'; const require = createRequire(import.meta.url);`
  },
  external: [
    'nodemailer',
    '@neondatabase/serverless',
    'sql.js'
  ],
  target: 'node20',
  minify: false,
  sourcemap: false,
})

// Copy sql.js WASM
const sqlWasmDir = join(outdir, 'node_modules', 'sql.js', 'dist')
mkdirSync(sqlWasmDir, { recursive: true })
cpSync('node_modules/sql.js/dist/sql-wasm.wasm', join(sqlWasmDir, 'sql-wasm.wasm'))

// Copy @neondatabase/serverless
const neonDir = join(outdir, 'node_modules', '@neondatabase', 'serverless')
mkdirSync(neonDir, { recursive: true })
cpSync('node_modules/@neondatabase/serverless/index.js', join(neonDir, 'index.js'))

// Copy undici
const undiciDir = join(outdir, 'node_modules', 'undici')
mkdirSync(undiciDir, { recursive: true })
cpSync('node_modules/undici/package.json', join(undiciDir, 'package.json'))
cpSync('node_modules/undici/index.js', join(undiciDir, 'index.js'))

// Copy .env if exists
try {
  cpSync('server/.env', join(outdir, '.env'))
} catch {
  writeFileSync(join(outdir, '.env'), "DATABASE_URL=''\n")
}

// Copy dotenv
const dotenvDir = join(outdir, 'node_modules', 'dotenv')
mkdirSync(dotenvDir, { recursive: true })
cpSync('node_modules/dotenv/package.json', join(dotenvDir, 'package.json'))
cpSync('node_modules/dotenv/lib/main.js', join(dotenvDir, 'lib', 'main.js'))
mkdirSync(join(dotenvDir, 'lib'), { recursive: true })
cpSync('node_modules/dotenv/lib/main.js', join(dotenvDir, 'lib', 'main.js'))

// Create package.json
writeFileSync(join(outdir, 'package.json'), JSON.stringify({
  name: 'game-lounge-server',
  version: '1.0.0',
  type: 'module',
  main: 'index.js'
}, null, 2))

console.log('Server bundled to', outdir)
console.log('Files:')
import { readdirSync, statSync } from 'node:fs'
function listFiles(dir, prefix = '') {
  for (const f of readdirSync(dir)) {
    const p = join(dir, f)
    if (statSync(p).isDirectory()) {
      listFiles(p, prefix + f + '/')
    } else {
      const size = statSync(p).size
      console.log(`  ${prefix}${f} (${(size / 1024).toFixed(1)}KB)`)
    }
  }
}
listFiles(outdir)
