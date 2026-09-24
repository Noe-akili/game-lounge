#!/usr/bin/env node
/**
 * bump-version.mjs — source unique de version pour Game Lounge.
 *
 * La v1.0.0 était dupliquée à 4 endroits (package.json, server-rust/package.json,
 * Cargo.toml, tauri.conf.json) sans aucun outil : il était facile d'oublier une
 * copie, et Android continuait alors d'accepter l'ancien APK. Ce script met les
 * 5 fichiers (Cargo.lock inclus) à jour en une commande.
 *
 * Usage :
 *   node scripts/bump-version.mjs patch          # 1.0.0 -> 1.0.1
 *   node scripts/bump-version.mjs minor --notes "Sync par défaut"
 *   node scripts/bump-version.mjs 1.2.0          # version explicite
 *   node scripts/bump-version.mjs minor --tag    # crée le tag git vX.Y.Z
 *   node scripts/bump-version.mjs minor --dry-run
 *
 * Note Android : Tauri dérive le versionCode de la version
 * (major * 1 000 000 + minor * 1000 + patch) ; ce script l'écrit aussi
 * explicitement dans tauri.conf.json pour lever toute ambiguïté.
 */
import { readFileSync, writeFileSync, existsSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')

const FILES = {
  frontendPkg: join(ROOT, 'package.json'),
  tauriPkg: join(ROOT, 'server-rust/package.json'),
  cargoToml: join(ROOT, 'server-rust/src-tauri/Cargo.toml'),
  cargoLock: join(ROOT, 'server-rust/src-tauri/Cargo.lock'),
  tauriConf: join(ROOT, 'server-rust/src-tauri/tauri.conf.json'),
  changelog: join(ROOT, 'CHANGELOG.md'),
}

const args = process.argv.slice(2)
const dryRun = args.includes('--dry-run')
const makeTag = args.includes('--tag')
const notes = []
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--notes') notes.push(args[++i])
}
const target = args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--notes')

function fail(msg) {
  console.error(`\x1b[31m✖ ${msg}\x1b[0m`)
  process.exit(1)
}

const parse = (v) => {
  const m = /^(\d+)\.(\d+)\.(\d+)(?:-.+)?$/.exec(v)
  if (!m) fail(`Version invalide : "${v}" (attendu X.Y.Z)`)
  return [Number(m[1]), Number(m[2]), Number(m[3])]
}

// --- 1. Version actuelle (package.json = source de vérité) ---
const current = parse(readFileSync(FILES.frontendPkg, 'utf8').match(/"version"\s*:\s*"([\d.]+)"/)?.[1] ?? '')

// --- 2. Nouvelle version ---
if (!target) fail('Usage : node scripts/bump-version.mjs patch|minor|major|X.Y.Z [--notes "..."] [--tag] [--dry-run]')
const [maj, min, pat] = current
const nextVersion =
  target === 'major' ? `${maj + 1}.0.0`
  : target === 'minor' ? `${maj}.${min + 1}.0`
  : target === 'patch' ? `${maj}.${min}.${pat + 1}`
  : target
if (!['major', 'minor', 'patch'].includes(target) && !/^\d+\.\d+\.\d+$/.test(target)) {
  fail(`Cible inconnue : "${target}" (attendu patch|minor|major ou X.Y.Z)`)
}
const [nMaj, nMin, nPat] = parse(nextVersion)
const next = `${nMaj}.${nMin}.${nPat}`
const versionCode = nMaj * 1_000_000 + nMin * 1000 + nPat
const currentStr = current.join('.')
if (next === currentStr && !dryRun) fail(`La version est déjà ${currentStr}`)

console.log(
  `\n\x1b[36m▸ Version ${currentStr} → ${next}\x1b[0m  (versionCode Android : ${versionCode})` +
  `${dryRun ? ' \x1b[33m[dry-run]\x1b[0m' : ''}\n`
)

// --- 3. Écriture des 5 fichiers de version ---
const writes = []
const stage = (file, content, label) => writes.push({ file, content, label })
const readVersion = (file, re) => {
  const content = readFileSync(file, 'utf8')
  const m = content.match(re)
  if (!m) return fail(`Version introuvable dans ${file}`)
  return { content, match: m[0] }
}

const frontend = readVersion(FILES.frontendPkg, /"version"\s*:\s*"[\d.]+"/)
stage(FILES.frontendPkg, frontend.content.replace(frontend.match, `"version": "${next}"`), 'package.json')

const tauriPkg = readVersion(FILES.tauriPkg, /"version"\s*:\s*"[\d.]+"/)
stage(FILES.tauriPkg, tauriPkg.content.replace(tauriPkg.match, `"version": "${next}"`), 'server-rust/package.json')

const cargoToml = readVersion(FILES.cargoToml, /^\s*version\s*=\s*"[\d.]+"/m)
stage(FILES.cargoToml, cargoToml.content.replace(cargoToml.match, `version = "${next}"`), 'server-rust/src-tauri/Cargo.toml')

if (existsSync(FILES.cargoLock)) {
  // Dans Cargo.lock, seule la version du crate game-lounge-rust est concernée.
  const lockRe = /(name = "game-lounge-rust"\nversion = ")[\d.]+(")/
  const lock = readFileSync(FILES.cargoLock, 'utf8')
  if (lockRe.test(lock)) {
    if (!lock.includes(`name = "game-lounge-rust"\nversion = "${currentStr}"`)) {
      console.warn(`  \x1b[33m!\x1b[0m Cargo.lock ne contenait pas ${currentStr} pour game-lounge-rust`)
    }
    stage(FILES.cargoLock, lock.replace(lockRe, `$1${next}$2`), 'server-rust/src-tauri/Cargo.lock')
  }
}

const conf = readVersion(FILES.tauriConf, /"version"\s*:\s*"[\d.]+"/)
let confContent = conf.content.replace(conf.match, `"version": "${next}"`)
if (/"minSdkVersion"\s*:\s*\d+/.test(confContent)) {
  confContent = /"versionCode"\s*:\s*\d+/.test(confContent)
    ? confContent.replace(/"versionCode"\s*:\s*\d+/, `"versionCode": ${versionCode}`)
    : confContent.replace(/(\n\s*)("minSdkVersion"\s*:\s*\d+)/, `$1"versionCode": ${versionCode},$1$2`)
}
stage(FILES.tauriConf, confContent, 'server-rust/src-tauri/tauri.conf.json')

for (const w of writes) {
  if (readFileSync(w.file, 'utf8') === w.content) continue
  if (!dryRun) writeFileSync(w.file, w.content, 'utf8')
  console.log(`  \x1b[32m✓\x1b[0m ${w.label}`)
}

// --- 4. CHANGELOG ---
const date = new Date().toISOString().slice(0, 10)
const bullets = notes.length ? notes.map((n) => `- ${n}`).join('\n') : '- Mise à jour.'
const section = `## [${next}] - ${date}\n\n${bullets}\n\n`
const header =
  '# Journal des versions — Game Lounge\n\n' +
  "Toutes les versions notables de l'application Android (APK signé). Format inspiré de\n" +
  '[Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).\n\n'

let changelog = existsSync(FILES.changelog) ? readFileSync(FILES.changelog, 'utf8') : header
if (changelog.includes(`## [${next}]`)) {
  console.log(`  \x1b[33m!\x1b[0m CHANGELOG.md contient déjà ${next} (inchangé)`)
} else {
  const firstSection = changelog.indexOf('\n## ')
  changelog =
    firstSection === -1
      ? changelog.trimEnd() + '\n\n' + section
      : changelog.slice(0, firstSection + 1) + '\n' + section + changelog.slice(firstSection + 1)
  if (!dryRun) writeFileSync(FILES.changelog, changelog, 'utf8')
  console.log(`  \x1b[32m✓\x1b[0m CHANGELOG.md`)
}

// --- 5. Tag git optionnel ---
if (makeTag) {
  const tag = `v${next}`
  if (dryRun) {
    console.log(`\n  \x1b[33m[dry-run]\x1b[0m git tag ${tag}`)
  } else {
    try {
      execFileSync('git', ['tag', '-a', tag, '-m', `Game Lounge ${next}`], { cwd: ROOT })
      console.log(`\n  \x1b[32m✓\x1b[0m tag git ${tag} créé`)
    } catch (e) {
      console.warn(`  \x1b[33m!\x1b[0m tag ${tag} non créé : ${String(e.message).split('\n')[0]}`)
    }
  }
}

console.log(
  `\n${dryRun ? 'Simulation terminée' : 'OK'} — ${next}\n` +
  `Prochaine étape :\n` +
  `  git add -A && git commit -m "chore(release): v${next}"\n` +
  `  git push origin main && git push origin v${next}   # publie l'APK dans la Release GitHub\n`
)
