// @ts-nocheck
// Vérifie si le mode debug est autorisé sur cet appareil (présence du dossier tarif)

export async function isDebugAllowed(): Promise<boolean> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const allowed = await invoke('debug_is_allowed')
    console.log('[DEBUG_CHECK] debug_is_allowed =', allowed)
    return !!allowed
  } catch (e) {
    console.warn('[DEBUG_CHECK] invoke failed, fallback false', e)
    return false
  }
}

export async function importDefaultTarifs(): Promise<any> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const res = await invoke('import_default_tarifs')
    console.log('[IMPORT] import_default_tarifs =', res)
    return res
  } catch (e) {
    console.error('[IMPORT] failed', e)
    throw e
  }
}

export async function getNeonStatus(): Promise<any> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke('neon_status')
  } catch (e) {
    console.warn('[NEON_STATUS] failed', e)
    return { neonEnabled: false, neonAvailable: false }
  }
}
