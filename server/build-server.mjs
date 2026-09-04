import { build } from 'esbuild'
import { existsSync, mkdirSync, cpSync, writeFileSync, rmSync, readdirSync, statSync } from 'node:fs'
import { join, dirname } from 'node:path'

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
    '@neondatabase/serverless',
    'sql.js',
    'undici',
    'dotenv',
    'buffer',
    'path',
    'fs',
    'os',
    'crypto'
  ],
  target: 'node20',
  minify: false,
  sourcemap: false,
})

// Copy node_modules deps
function copyDirContents(src, dest) {
  if (!existsSync(src)) return
  mkdirSync(dest, { recursive: true })
  for (const f of readdirSync(src)) {
    const sp = join(src, f)
    const dp = join(dest, f)
    if (statSync(sp).isDirectory()) {
      copyDirContents(sp, dp)
    } else {
      cpSync(sp, dp)
    }
  }
}

const deps = [
  ['sql.js', 'dist'],
  ['@neondatabase/serverless', null],
  ['undici', null],
  ['dotenv', null],
]

for (const [pkg, sub] of deps) {
  const src = sub ? join('node_modules', pkg, sub) : join('node_modules', pkg)
  const dest = join(outdir, 'node_modules', pkg, sub || '')
  if (existsSync(src)) {
    copyDirContents(src, dest)
    console.log(`  Copied ${pkg}${sub ? '/' + sub : ''}`)
  } else {
    console.warn(`  Skipped ${pkg} (not found)`)
  }
}

// Copy .env if exists
try {
  cpSync('server/.env', join(outdir, '.env'))
} catch {
  writeFileSync(join(outdir, '.env'), "DATABASE_URL=''\n")
}

// Create package.json
writeFileSync(join(outdir, 'package.json'), JSON.stringify({
  name: 'game-lounge-server',
  version: '1.0.0',
  type: 'module',
  main: 'index.js'
}, null, 2))

console.log('\nServer bundled to', outdir)
console.log('Files:')
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
