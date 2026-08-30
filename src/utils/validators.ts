// @ts-nocheck
// Reusable regex validators & sanitizers for frontend
// Prevent XSS, SQL injection via input sanitization and strict regex validation

export const EMAIL_REGEX = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/
export const PHONE_REGEX = /^\+?[0-9]{8,15}$/
export const PASSWORD_REGEX = /^(?=.*[A-Za-z]).{6,}$/
export const NOM_REGEX = /^[a-zA-ZÀ-ÿ0-9\s\-'&]{2,50}$/
export const TITRE_REGEX = /^[a-zA-ZÀ-ÿ0-9\s\-'&]{2,100}$/
export const GENRE_REGEX = /^[a-zA-ZÀ-ÿ0-9\s\-'&]{2,50}$/
export const ROLE_REGEX = /^(admin|employe)$/
export const CONSOLE_TYPE_REGEX = /^(PS5|PS4|XBOX|PC|SWITCH)$/
export const TARIF_TYPE_REGEX = /^(horaire|forfait|session|partie)$/
export const REGLE_TYPE_REGEX = /^(temps|montant)$/
export const JETON_TYPE_REGEX = /^(gain|depense|bonus)$/
export const STATUT_SESSION_REGEX = /^(en_cours|pause|terminee|annulee)$/
export const STATUT_FACTURE_REGEX = /^(payee|en_attente|annulee)$/
export const MODE_PAIEMENT_REGEX = /^(especes|carte|mobile|mobile_money|jetons)$/

/**
 * Sanitize generic text: remove < > to prevent XSS, trim, and enforce max length
 */
export function sanitizeInput(input: string, maxLength = 500): string {
  if (typeof input !== 'string') return ''
  return input.replace(/[<>]/g, '').trim().slice(0, maxLength)
}

/**
 * Generic text validator with length checks after sanitization
 */
export function isValidText(input: string, min = 1, max = 500): boolean {
  if (typeof input !== 'string') return false
  const sanitized = sanitizeInput(input, max)
  return sanitized.length >= min && sanitized.length <= max && !/[<>]/.test(input)
}

/**
 * Email validation
 */
export function isValidEmail(email: string): boolean {
  if (typeof email !== 'string') return false
  return EMAIL_REGEX.test(email.trim())
}

/**
 * Phone validation - strips spaces/dashes before testing
 * Valid: 8-15 digits optionally starting with +
 */
export function isValidPhone(phone: string): boolean {
  if (typeof phone !== 'string' || !phone.trim()) return false
  const cleaned = phone.replace(/[\s\-]/g, '')
  return PHONE_REGEX.test(cleaned)
}

export function isValidTelephone(phone: string): boolean {
  return isValidPhone(phone)
}

/**
 * Password validation: min 6 chars, at least one letter
 */
export function isValidPassword(password: string): boolean {
  if (typeof password !== 'string') return false
  return PASSWORD_REGEX.test(password)
}

/**
 * Nom validation: 2-50 chars, allows letters, numbers, spaces, - ' &
 */
export function isValidNom(nom: string): boolean {
  if (typeof nom !== 'string') return false
  const sanitized = sanitizeInput(nom, 50)
  return NOM_REGEX.test(sanitized.trim())
}

/**
 * Titre validation: 2-100 chars
 */
export function isValidTitre(titre: string): boolean {
  if (typeof titre !== 'string') return false
  const sanitized = sanitizeInput(titre, 100)
  return TITRE_REGEX.test(sanitized.trim())
}

/**
 * Genre validation: 2-50 chars (optional field - empty is valid)
 */
export function isValidGenre(genre: string): boolean {
  if (!genre) return true
  if (typeof genre !== 'string') return false
  const sanitized = sanitizeInput(genre, 50)
  return GENRE_REGEX.test(sanitized.trim())
}

export function isValidDescription(desc: string, max = 500): boolean {
  if (!desc) return true
  if (typeof desc !== 'string') return false
  const sanitized = sanitizeInput(desc, max)
  return sanitized.length <= max && sanitized.length >= 2
}

/**
 * Role validation
 */
export function isValidRole(role: string): boolean {
  return ROLE_REGEX.test(role)
}

/**
 * Generic type validation (for tarifs, consoles, etc.)
 */
export function isValidType(type: string): boolean {
  // Allow tarif types and console types as generic fallback
  return TARIF_TYPE_REGEX.test(type) || CONSOLE_TYPE_REGEX.test(type) || REGLE_TYPE_REGEX.test(type)
}

export function isValidConsoleType(type: string): boolean {
  return CONSOLE_TYPE_REGEX.test(type)
}

export function isValidTarifType(type: string): boolean {
  return TARIF_TYPE_REGEX.test(type)
}

export function isValidRegleType(type: string): boolean {
  return REGLE_TYPE_REGEX.test(type)
}

export function isValidJetonType(type: string): boolean {
  return JETON_TYPE_REGEX.test(type)
}

export function isValidSessionStatut(statut: string): boolean {
  return STATUT_SESSION_REGEX.test(statut)
}

export function isValidFactureStatut(statut: string): boolean {
  return STATUT_FACTURE_REGEX.test(statut)
}

export function isValidModePaiement(mode: string): boolean {
  return MODE_PAIEMENT_REGEX.test(mode)
}

/**
 * Poste numero: 1-100
 */
export function isValidPosteNumero(num: number | string): boolean {
  const n = Number(num)
  return Number.isInteger(n) && n >= 1 && n <= 100
}

/**
 * Duree minutes: 1-1000
 */
export function isValidDuree(duree: number | string): boolean {
  const n = Number(duree)
  return Number.isInteger(n) && n >= 1 && n <= 1000
}

export function isValidDureeMinutes(duree: number | string): boolean {
  return isValidDuree(duree)
}

/**
 * Prix: 1-1000000
 */
export function isValidPrix(prix: number | string): boolean {
  const n = Number(prix)
  return Number.isFinite(n) && n >= 1 && n <= 1000000
}

/**
 * Seuil: 1-10000
 */
export function isValidSeuil(seuil: number | string): boolean {
  const n = Number(seuil)
  return Number.isInteger(n) && n >= 1 && n <= 10000
}

/**
 * Jetons attribues: 1-1000
 */
export function isValidJetonsAttribues(n: number | string): boolean {
  const num = Number(n)
  return Number.isInteger(num) && num >= 1 && num <= 1000
}

export function isValidQuantite(q: number | string): boolean {
  const num = Number(q)
  return Number.isInteger(num) && num >= 1 && num <= 10000
}

export function isValidId(id: number | string): boolean {
  const n = Number(id)
  return Number.isInteger(n) && n > 0
}

/**
 * Contenu validation for messages: 1-1000 chars after sanitization
 */
export function isValidContenu(contenu: string): boolean {
  if (typeof contenu !== 'string') return false
  const sanitized = sanitizeInput(contenu, 1000)
  return sanitized.length >= 1 && sanitized.length <= 1000
}

export function isValidTitreMessage(titre: string): boolean {
  if (!titre) return true // optional
  if (typeof titre !== 'string') return false
  const sanitized = sanitizeInput(titre, 100)
  return sanitized.length >= 2 && sanitized.length <= 100
}
