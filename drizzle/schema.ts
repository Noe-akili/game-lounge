import { sqliteTable, text, integer } from 'drizzle-orm/sqlite-core'

export const users = sqliteTable('users', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  email: text('email').notNull().unique(),
  passwordHash: text('password_hash').notNull(),
  role: text('role', { enum: ['admin', 'employe'] }).notNull(),
  nom: text('nom').notNull(),
  createdAt: text('created_at').notNull(),
})

export const consoles = sqliteTable('consoles', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  nom: text('nom').notNull(),
  type: text('type', { enum: ['PS4', 'PS5', 'XBOX', 'PC', 'SWITCH'] }).notNull(),
  etat: text('etat', { enum: ['disponible', 'occupee', 'pause', 'maintenance', 'hors_service'] }).notNull(),
  posteNumero: integer('poste_numero').notNull(),
  createdAt: text('created_at').notNull(),
})

export const jeux = sqliteTable('jeux', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  titre: text('titre').notNull(),
  genre: text('genre'),
  consoleId: integer('console_id'),
  jaquetteUrl: text('jaquette_url'),
  actif: integer('actif', { mode: 'boolean' }).notNull().default(true),
  createdAt: text('created_at').notNull(),
})

export const joueurs = sqliteTable('joueurs', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  nom: text('nom').notNull(),
  telephone: text('telephone'),
  email: text('email'),
  jetonsSolde: integer('jetons_solde').notNull().default(0),
  dateInscription: text('date_inscription').notNull(),
  derniereVisite: text('derniere_visite'),
})

export const tarifs = sqliteTable('tarifs', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  type: text('type').notNull(),
  dureeMinutes: integer('duree_minutes').notNull(),
  prix: integer('prix').notNull(),
  description: text('description'),
  actif: integer('actif', { mode: 'boolean' }).notNull().default(true),
  consoleType: text('console_type'),
  jeu: text('jeu'),
})

export const sessionsJeu = sqliteTable('sessions_jeu', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  consoleId: integer('console_id').notNull(),
  joueurId: integer('joueur_id').notNull(),
  jeuId: integer('jeu_id').notNull(),
  employeId: integer('employe_id').notNull(),
  debut: text('debut').notNull(),
  fin: text('fin'),
  dureeMinutes: integer('duree_minutes').notNull().default(0),
  montant: integer('montant').notNull().default(0),
  tarifPrix: integer('tarif_prix'),
  jetonsGagnes: integer('jetons_gagnes').notNull().default(0),
  statut: text('statut', { enum: ['en_cours', 'pause', 'terminee', 'annulee'] }).notNull(),
  createdAt: text('created_at').notNull(),
})

export const factures = sqliteTable('factures', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  numeroFacture: text('numero_facture').notNull().unique(),
  sessionId: integer('session_id').notNull(),
  joueurId: integer('joueur_id').notNull(),
  montantHt: integer('montant_ht').notNull(),
  tauxTva: integer('taux_tva').notNull(),
  montantTva: integer('montant_tva').notNull(),
  montantTtc: integer('montant_ttc').notNull(),
  modePaiement: text('mode_paiement', { enum: ['especes', 'carte', 'mobile_money', 'jetons'] }).notNull(),
  statut: text('statut', { enum: ['payee', 'en_attente', 'annulee'] }).notNull(),
  datePaiement: text('date_paiement').notNull(),
  createdAt: text('created_at').notNull(),
})

export const lignesFacture = sqliteTable('lignes_facture', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  factureId: integer('facture_id').notNull(),
  description: text('description').notNull(),
  quantite: integer('quantite').notNull(),
  prixUnitaire: integer('prix_unitaire').notNull(),
  totalLigne: integer('total_ligne').notNull(),
})

export const jetonsTransactions = sqliteTable('jetons_transactions', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  joueurId: integer('joueur_id').notNull(),
  type: text('type', { enum: ['gain', 'depense', 'bonus'] }).notNull(),
  quantite: integer('quantite').notNull(),
  raison: text('raison'),
  sessionId: integer('session_id'),
  createdAt: text('created_at').notNull(),
})

export const parametresFidelite = sqliteTable('parametres_fidelite', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  regleType: text('regle_type', { enum: ['temps', 'montant'] }).notNull(),
  seuil: integer('seuil').notNull(),
  jetonsAttribues: integer('jetons_attribues').notNull(),
  actif: integer('actif', { mode: 'boolean' }).notNull().default(true),
})
