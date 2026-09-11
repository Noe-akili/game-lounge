// @ts-nocheck
// Tauri-only PDF : généré par Rust (factures_pdf)
import { handleRequest } from './transport'

function base64ToUint8Array(b64: string): Uint8Array {
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return bytes
}

export async function getFacturePdfBlob(id: number): Promise<Blob> {
  let token: string | null = null
  try { token = localStorage.getItem('gl_token') } catch { token = null }
  const result = await handleRequest(`/factures/${id}/pdf`, 'GET', undefined, token || undefined)
  if (result.status >= 400) throw new Error(result.body?.message || 'Facture non trouvée')
  const b64 = result.body?.pdf_base64
  if (!b64) throw new Error('PDF non généré')
  if (b64.length > 5 * 1024 * 1024) throw new Error('PDF trop volumineux')
  return new Blob([base64ToUint8Array(b64)], { type: 'application/pdf' })
}
