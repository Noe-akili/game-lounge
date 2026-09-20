
function sanitizeErrorMessage(msg: string): string {
  if (!msg) return "Une erreur inattendue est survenue";
  const lower = msg.toLowerCase();
  if (lower.includes("no column named") || lower.includes("sqlite") || lower.includes("insert") || lower.includes("table")) {
    return "Une erreur de base de données est survenue. Veuillez réessayer.";
  }
  if (lower.includes("pool connect") || lower.includes("offline") || lower.includes("postgres") || lower.includes("pgbouncer") || lower.includes("supabase")) {
    return "Connexion au serveur impossible pour le moment. Vérifiez votre connexion Internet.";
  }
  if (lower.includes("timeout") || lower.includes("délai dépassé")) {
    return "Le serveur a mis trop de temps à répondre. Veuillez réessayer.";
  }
  if (lower.includes("spawn_blocking") || lower.includes("internal")) {
    return "Une erreur de traitement interne est survenue.";
  }
  return msg;
}
// @ts-nocheck
import { handleRequest } from "@/lib/transport"
import { SessionStorage } from "@/lib/sessionStorage"

function getToken(): string | null {
  return SessionStorage.getToken()
}

async function request(path: string, options: any = {}) {
  const token = getToken()
  const body = options.body ? (typeof options.body === "string" ? JSON.parse(options.body) : options.body) : undefined
  const result = await handleRequest(path, options.method || "GET", body, token)
  if (result.status >= 400) {
    const err: any = new Error(result.body?.message || "Erreur " + result.status)
    err.status = result.status
    throw err
  }
  return result.body
}

export const api = {
  get: (p: string) => request(p, { method: "GET" }),
  post: (p: string, b?: any) => request(p, { method: "POST", body: b }),
  put: (p: string, b?: any) => request(p, { method: "PUT", body: b }),
  delete: (p: string) => request(p, { method: "DELETE" }),
}
