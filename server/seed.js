import bcrypt from 'bcryptjs'
import { insert, queryOne } from './db.js'

console.log('🌱 Seeding database with REAL KING OF GAME tariffs...')

const adminHash = bcrypt.hashSync('admin123', 10)
const empHash = bcrypt.hashSync('employe123', 10)

if (!queryOne('users', u => u.email === 'admin@gamelounge.com')) {
  insert('users', { email: 'admin@gamelounge.com', password_hash: adminHash, role: 'admin', nom: 'Admin', created_at: new Date().toISOString() })
  insert('users', { email: 'john@gamelounge.com', password_hash: empHash, role: 'employe', nom: 'John Doe', created_at: new Date().toISOString() })
}

if (!queryOne('consoles', () => true)) {
  const consolesData = [
    { nom: 'PS5 - Poste 1', type: 'PS5', poste_numero: 1, etat: 'disponible' },
    { nom: 'PS5 - Poste 2', type: 'PS5', poste_numero: 2, etat: 'disponible' },
    { nom: 'PS4 - Poste 3', type: 'PS4', poste_numero: 3, etat: 'disponible' },
    { nom: 'PS4 - Poste 4', type: 'PS4', poste_numero: 4, etat: 'disponible' },
    { nom: 'PS5 - Poste 5', type: 'PS5', poste_numero: 5, etat: 'disponible' },
    { nom: 'PS4 - Poste 6', type: 'PS4', poste_numero: 6, etat: 'disponible' },
  ]
  consolesData.forEach(c => insert('consoles', { ...c, created_at: new Date().toISOString() }))

  const c1 = queryOne('consoles', c => c.poste_numero === 1)
  const c2 = queryOne('consoles', c => c.poste_numero === 2)
  const c3 = queryOne('consoles', c => c.poste_numero === 3)
  const c4 = queryOne('consoles', c => c.poste_numero === 4)
  const c5 = queryOne('consoles', c => c.poste_numero === 5)
  const c6 = queryOne('consoles', c => c.poste_numero === 6)

  const jeuxData = [
    { titre: 'FIFA 26', genre: 'Sport', console_id: c1.id },
    { titre: 'FIFA 26', genre: 'Sport', console_id: c3.id },
    { titre: 'Mortal Kombat 1', genre: 'Combat', console_id: c1.id },
    { titre: 'Mortal Kombat 11', genre: 'Combat', console_id: c3.id },
    { titre: 'Tekken 8', genre: 'Combat', console_id: c2.id },
    { titre: 'Need for Speed', genre: 'Course', console_id: c1.id },
    { titre: 'Need for Speed', genre: 'Course', console_id: c3.id },
    { titre: 'WWE 2K25', genre: 'Combat', console_id: c2.id },
    { titre: 'NBA 2K25', genre: 'Sport', console_id: c1.id },
    { titre: 'NBA 2K25', genre: 'Sport', console_id: c4.id },
    { titre: 'Gran Turismo 7', genre: 'Course', console_id: c5.id },
    { titre: 'GTA V', genre: 'Action', console_id: c1.id },
    { titre: 'GTA V', genre: 'Action', console_id: c3.id },
    { titre: 'God of War', genre: 'Action', console_id: c2.id },
    { titre: 'God of War', genre: 'Action', console_id: c4.id },
    { titre: 'Call of Duty', genre: 'Action', console_id: c5.id },
    { titre: 'Call of Duty', genre: 'Action', console_id: c6.id },
    { titre: 'Fortnite', genre: 'Action', console_id: c5.id },
    { titre: 'Spider-Man 2', genre: 'Action', console_id: c2.id },
    { titre: 'Red Dead Redemption 2', genre: 'Action', console_id: c4.id },
    { titre: 'Resident Evil 4', genre: 'Horreur', console_id: c6.id },
    { titre: 'Undisputed', genre: 'Combat', console_id: c6.id },
    { titre: 'EA Sports UFC 5', genre: 'Combat', console_id: c1.id },
    { titre: 'Naruto Storm 4', genre: 'Combat', console_id: c3.id },
    { titre: 'Uncharted 4', genre: 'Aventure', console_id: c4.id },
  ]
  jeuxData.forEach(j => insert('jeux', { ...j, jaquette_url: null, actif: true, created_at: new Date().toISOString() }))

  const joueursData = [
    { nom: 'Kevin M.', telephone: '+243812345678', email: 'kevin@email.com', jetons_solde: 12 },
    { nom: 'Alex B.', telephone: '+243823456789', email: 'alex@email.com', jetons_solde: 5 },
    { nom: 'Tom D.', telephone: '+243834567890', email: 'tom@email.com', jetons_solde: 8 },
    { nom: 'Sarah L.', telephone: '+243845678901', email: 'sarah@email.com', jetons_solde: 3 },
    { nom: 'John R.', telephone: '+243856789012', email: 'johnr@email.com', jetons_solde: 15 },
    { nom: 'Lucas R.', telephone: '+243867890123', email: 'lucas@email.com', jetons_solde: 2 },
    { nom: 'Ethan G.', telephone: '+243878901234', email: 'ethan@email.com', jetons_solde: 7 },
  ]
  joueursData.forEach(j => insert('joueurs', { ...j, date_inscription: new Date().toISOString(), derniere_visite: null }))

  // ====== TARIFS RÉELS KING OF GAME ======

  // --- PS4 TARIFS ---
  // FIFA 26 PS4
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'FIFA 26', duree_minutes: 5, prix: 500, description: 'FIFA 26 PS4 - 1 Match 5min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'FIFA 26', duree_minutes: 30, prix: 2000, description: 'FIFA 26 PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'FIFA 26', duree_minutes: 60, prix: 4000, description: 'FIFA 26 PS4 - Séance 1h', actif: true })

  // Mortal Kombat PS4
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'Mortal Kombat', duree_minutes: 5, prix: 500, description: 'Mortal Kombat PS4 - 2 Combats simples', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Mortal Kombat', duree_minutes: 30, prix: 1500, description: 'Mortal Kombat PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Mortal Kombat', duree_minutes: 60, prix: 3000, description: 'Mortal Kombat PS4 - Séance 1h', actif: true })

  // Tekken 7 PS4
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'Tekken', duree_minutes: 5, prix: 500, description: 'Tekken PS4 - 2 Combats simples', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Tekken', duree_minutes: 30, prix: 1500, description: 'Tekken PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Tekken', duree_minutes: 60, prix: 3000, description: 'Tekken PS4 - Séance 1h', actif: true })

  // Need for Speed PS4
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Need for Speed', duree_minutes: 15, prix: 500, description: 'NFS PS4 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Need for Speed', duree_minutes: 30, prix: 1000, description: 'NFS PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Need for Speed', duree_minutes: 60, prix: 2000, description: 'NFS PS4 - Séance 1h', actif: true })

  // WWE 2K25 PS4
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'WWE 2K25', duree_minutes: 10, prix: 1000, description: 'WWE PS4 - 1 Combat 10min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'WWE 2K25', duree_minutes: 30, prix: 2000, description: 'WWE PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'WWE 2K25', duree_minutes: 60, prix: 4000, description: 'WWE PS4 - Séance 1h', actif: true })

  // NBA 2K25 PS4
  insert('tarifs', { type: 'partie', console_type: 'PS4', jeu: 'NBA 2K25', duree_minutes: 15, prix: 1500, description: 'NBA PS4 - 1 Match 10-20min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'NBA 2K25', duree_minutes: 30, prix: 2500, description: 'NBA PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'NBA 2K25', duree_minutes: 60, prix: 4000, description: 'NBA PS4 - Séance 1h', actif: true })

  // Actions PS4 (GTA V, God of War, CoD, Spider-Man 2, RDR2, Uncharted 4, RE4)
  const ps4Actions = ['GTA V', 'God of War', 'Call of Duty', 'Spider-Man 2', 'Red Dead Redemption 2', 'Uncharted 4', 'Resident Evil 4']
  ps4Actions.forEach(jeu => {
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 15, prix: 500, description: `${jeu} PS4 - Séance 15min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 30, prix: 1500, description: `${jeu} PS4 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 60, prix: 3000, description: `${jeu} PS4 - Séance 1h`, actif: true })
  })

  // UFC/Undisputed/Naruto PS4
  const ps4Combat = ['EA Sports UFC 5', 'Undisputed', 'Naruto Storm 4']
  ps4Combat.forEach(jeu => {
    insert('tarifs', { type: 'partie', console_type: 'PS4', jeu, duree_minutes: 5, prix: 500, description: `${jeu} PS4 - Combat simple`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 30, prix: 1500, description: `${jeu} PS4 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS4', jeu, duree_minutes: 60, prix: 3000, description: `${jeu} PS4 - Séance 1h`, actif: true })
  })

  // Fortnite PS4
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Fortnite', duree_minutes: 15, prix: 500, description: 'Fortnite PS4 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Fortnite', duree_minutes: 30, prix: 2000, description: 'Fortnite PS4 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS4', jeu: 'Fortnite', duree_minutes: 60, prix: 3000, description: 'Fortnite PS4 - Séance 1h', actif: true })

  // --- PS5 TARIFS ---
  // FIFA 26 PS5
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'FIFA 26', duree_minutes: 5, prix: 1000, description: 'FIFA 26 PS5 - 1 Match 5min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'FIFA 26', duree_minutes: 30, prix: 4000, description: 'FIFA 26 PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'FIFA 26', duree_minutes: 60, prix: 8000, description: 'FIFA 26 PS5 - Séance 1h', actif: true })

  // Mortal Kombat PS5
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'Mortal Kombat', duree_minutes: 5, prix: 1000, description: 'Mortal Kombat PS5 - Combat simple', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Mortal Kombat', duree_minutes: 30, prix: 3000, description: 'Mortal Kombat PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Mortal Kombat', duree_minutes: 60, prix: 6000, description: 'Mortal Kombat PS5 - Séance 1h', actif: true })

  // Tekken 8 PS5
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'Tekken 8', duree_minutes: 5, prix: 1000, description: 'Tekken 8 PS5 - Combat simple', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Tekken 8', duree_minutes: 30, prix: 3000, description: 'Tekken 8 PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Tekken 8', duree_minutes: 60, prix: 6000, description: 'Tekken 8 PS5 - Séance 1h', actif: true })

  // Gran Turismo 7 PS5
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 5, prix: 1000, description: 'GT7 PS5 - Course courte', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 15, prix: 2000, description: 'GT7 PS5 - 15min Volant G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 15, prix: 3000, description: 'GT7 PS5 - 15min Volant G29 + VR2', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 30, prix: 3500, description: 'GT7 PS5 - 30min Volant G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 30, prix: 5000, description: 'GT7 PS5 - 30min Volant G29 + VR2', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 60, prix: 7000, description: 'GT7 PS5 - 1h Volant G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Gran Turismo 7', duree_minutes: 60, prix: 10000, description: 'GT7 PS5 - 1h Volant G29 + VR2', actif: true })

  // Need for Speed PS5
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 15, prix: 1000, description: 'NFS PS5 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 15, prix: 1500, description: 'NFS PS5 - 15min avec G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 30, prix: 2000, description: 'NFS PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 30, prix: 3000, description: 'NFS PS5 - 30min avec G29', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 60, prix: 4000, description: 'NFS PS5 - Séance 1h', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Need for Speed', duree_minutes: 60, prix: 6000, description: 'NFS PS5 - 1h avec G29', actif: true })

  // WWE 2K25 PS5
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'WWE 2K25', duree_minutes: 10, prix: 1500, description: 'WWE PS5 - 1 Combat 10min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'WWE 2K25', duree_minutes: 30, prix: 3000, description: 'WWE PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'WWE 2K25', duree_minutes: 60, prix: 6000, description: 'WWE PS5 - Séance 1h', actif: true })

  // NBA 2K25 PS5
  insert('tarifs', { type: 'partie', console_type: 'PS5', jeu: 'NBA 2K25', duree_minutes: 15, prix: 2000, description: 'NBA PS5 - 1 Match 10-20min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'NBA 2K25', duree_minutes: 30, prix: 4000, description: 'NBA PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'NBA 2K25', duree_minutes: 60, prix: 7000, description: 'NBA PS5 - Séance 1h', actif: true })

  // Actions PS5 (GTA V, God of War, CoD, Spider-Man 2, RDR2, RE4)
  const ps5Actions1500 = ['GTA V', 'God of War', 'Spider-Man 2', 'Red Dead Redemption 2', 'Uncharted 4', 'Resident Evil 4']
  ps5Actions1500.forEach(jeu => {
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 15, prix: 1000, description: `${jeu} PS5 - Séance 15min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 30, prix: 2000, description: `${jeu} PS5 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 60, prix: 4000, description: `${jeu} PS5 - Séance 1h`, actif: true })
  })

  // Call of Duty PS5 (tarifs plus élevés)
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Call of Duty', duree_minutes: 15, prix: 1500, description: 'CoD PS5 - Séance 15min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Call of Duty', duree_minutes: 30, prix: 3000, description: 'CoD PS5 - Séance 30min', actif: true })
  insert('tarifs', { type: 'session', console_type: 'PS5', jeu: 'Call of Duty', duree_minutes: 60, prix: 6000, description: 'CoD PS5 - Séance 1h', actif: true })

  // UFC/Undisputed/Naruto PS5
  const ps5Combat = ['EA Sports UFC 5', 'Undisputed', 'Naruto Storm 4']
  ps5Combat.forEach(jeu => {
    insert('tarifs', { type: 'partie', console_type: 'PS5', jeu, duree_minutes: 5, prix: 1000, description: `${jeu} PS5 - Combat simple`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 30, prix: 3000, description: `${jeu} PS5 - Séance 30min`, actif: true })
    insert('tarifs', { type: 'session', console_type: 'PS5', jeu, duree_minutes: 60, prix: 6000, description: `${jeu} PS5 - Séance 1h`, actif: true })
  })

  insert('parametres_fidelite', { regle_type: 'temps', seuil: 60, jetons_attribues: 1, actif: true })
}

console.log('✅ Seed complete with real KING OF GAME tariffs!')
console.log('   admin@gamelounge.com / admin123')
console.log('   john@gamelounge.com / employe123')
