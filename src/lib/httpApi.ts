// @ts-nocheck
// Transport HTTP pour le mode "backend Rust dans le navigateur" (débogage).
// Activé via VITE_GL_SERVER_URL=<url> (ex: http://localhost:8080) au build.

const SERVER = import.meta.env.VITE_GL_SERVER_URL as string | undefined

export function isHttpServerMode() {
  return typeof SERVER === 'string' && SERVER.length > 0
}

function getPath(url: string): string {
  return url.split('?')[0]
}

export async function handleHttpServerRequest(
  path: string,
  method: string,
  body?: any,
  authToken?: string
): Promise<{ status: number; body: any }> {
  const clean = getPath(path)
  const ready = clean.startsWith('/api') ? clean : `/api${clean}`
  const query = path.includes('?') ? path.slice(path.indexOf('?')) : ''

  let url = `${SERVER}${ready}${query}`
  if (method === 'GET' && authToken && !path.includes('token=')) {
    // empty
  }

  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  if (authToken) headers['Authorization'] = `Bearer ${authToken}`

  try {
    const res = await fetch(url, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    })
    let data: any = null
    const text = await res.text()
    try { data = text ? JSON.parse(text) : null } catch { data = text }
    return { status: res.status, body: data }
  } catch (e: any) {
    return { status: 500, body: { message: `Serveur HTTP injoignable (${SERVER}): ${e?.message || e}` } }
  }
}