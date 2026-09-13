// @ts-nocheck
// Tauri-only PDF : généré par Rust (factures_pdf / factures_save_pdf)
import { handleRequest } from './transport'

// Dossier par défaut demandé (mission) : Documents/GameLounge/Factures.
// Sur Android le chemin direct /storage/emulated/0/... est refusé (Scoped
// Storage) -> fallback automatique sur le SAF (sélecteur de dossier système).
export const DEFAULT_PDF_DIR = '/storage/emulated/0/Documents/GameLounge/Factures'

const DIR_KEY = 'gl_pdf_dir'

/// Dossier choisi par l'utilisateur (mémorisé pour les exports suivants).
export function getSavedPdfDir(): string {
  try { return localStorage.getItem(DIR_KEY) || DEFAULT_PDF_DIR } catch { return DEFAULT_PDF_DIR }
}

export function setSavedPdfDir(dir: string) {
  try { localStorage.setItem(DIR_KEY, dir) } catch {}
}

function base64ToUint8Array(b64: string): Uint8Array {
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return bytes
}

async function token(): Promise<string | undefined> {
  try { return localStorage.getItem('gl_token') || undefined } catch { return undefined }
}

/// Récupère le PDF (base64) d'une facture.
export async function getFacturePdfBase64(id: number): Promise<{ b64: string; filename: string }> {
  const result = await handleRequest(`/factures/${id}/pdf`, 'GET', undefined, await token())
  if (result.status >= 400) throw new Error(result.body?.message || 'Facture non trouvée')
  const b64 = result.body?.pdf_base64
  if (!b64) throw new Error('PDF non généré')
  if (b64.length > 20 * 1024 * 1024) throw new Error('PDF trop volumineux')
  return { b64, filename: `facture-${id}.pdf` }
}

/// Récupère le PDF sous forme de Blob (aperçu / impression navigateur).
export async function getFacturePdfBlob(id: number): Promise<Blob> {
  const { b64 } = await getFacturePdfBase64(id)
  return new Blob([base64ToUint8Array(b64)], { type: 'application/pdf' })
}

/// Enregistre le PDF de la facture dans le dossier demandé (ou le dossier
/// mémorisé / par défaut). Retourne { path } en cas de succès.
/// Si Android refuse l'accès direct (Scoped Storage), tente le SAF via le
/// pont Tauri (invoke Android injecté par le patch de build) ; en dernier
/// recours ouvre le PDF (l'utilisateur peut l'enregistrer depuis le viewer).
export async function saveFacturePdf(id: number, opts?: { folder?: string; filename?: string }): Promise<{ path?: string; opened?: boolean }> {
  const folder = opts?.folder || getSavedPdfDir()
  const result = await handleRequest(`/factures/${id}/save-pdf`, 'POST', { folder, filename: opts?.filename }, await token())
  const body = result.body || {}
  if (result.status >= 400) throw new Error(body.message || 'Erreur enregistrement PDF')

  if (body.success) {
    // Mémorise le dossier qui a fonctionné.
    setSavedPdfDir(folder)
    return { path: body.path }
  }

  // ---- Fallback SAF (Android 10+) : écriture directe refusée ----
  if (body.saf_required && body.pdf_base64) {
    const w: any = window as any
    // 1) Pont natif injecté par le patch Android (EdgeInsetActivity) :
    //    GameLoungePdf.save(filename, base64) -> dossier choisi au premier
    //    appel via ACTION_OPEN_DOCUMENT_TREE, mémorisé (URI persistée).
    try {
      if (w.GameLoungePdf?.save) {
        const saved = await w.GameLoungePdf.save(body.filename || `facture-${id}.pdf`, body.pdf_base64)
        if (saved) {
          // Mémorise le marqueur "SAF" : le dossier direct ne marche pas ici.
          setSavedPdfDir('saf://chosen')
          return { path: saved }
        }
      }
    } catch {}
    // 2) Dernier recours : ouvrir le PDF (le viewer système permet
    //    "Enregistrer"/"Imprimer" ; aucun contenu n'est perdu).
    await openFacturePdf(id)
    return { opened: true }
  }

  throw new Error(body.error || 'Enregistrement impossible')
}

/// Ouvre le PDF (viewer système / onglet navigateur) — utilisé par "Imprimer".
export async function openFacturePdf(id: number): Promise<void> {
  const blob = await getFacturePdfBlob(id)
  const url = URL.createObjectURL(blob)
  try {
    const a = document.createElement('a')
    a.href = url
    a.target = '_blank'
    document.body.appendChild(a)
    a.click()
    a.remove()
  } finally {
    setTimeout(() => URL.revokeObjectURL(url), 8000)
  }
}
