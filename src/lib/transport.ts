// @ts-nocheck
// Dispatcher de requêtes API : détecte l'hôte Tauri (application Android légère,
// backend Rust) et route vers les commandes Tauri. Sinon, repli sur la base
// locale sql.js (mode navigateur / démo).

import { handleClientRequest } from './clientApi'
import { handleTauriRequest } from './tauriApi'

export function isTauri() {
  return (
    typeof window !== 'undefined' &&
    !!(window as any).__TAURI__?.core?.invoke
  )
}

export async function handleRequest(
  path: string,
  method: string,
  body?: any,
  authToken?: string
): Promise<{ status: number; body: any }> {
  if (isTauri()) {
    return handleTauriRequest(path, method, body, authToken)
  }
  return handleClientRequest(path, method, body, authToken)
}