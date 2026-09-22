<template>
  <div class="space-y-6 w-full max-w-5xl mx-auto pb-12">
    <!-- En-tête -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-white/5 pb-5">
      <div>
        <div class="flex items-center gap-2 text-neon-violet mb-1">
          <BookOpen class="w-5 h-5" />
          <span class="text-xs font-semibold uppercase tracking-widest">Guide & Aide</span>
        </div>
        <h2 class="font-gaming text-2xl font-bold text-txt">Documentation Game Lounge</h2>
        <p class="text-sm text-txt-dim mt-1">Tout ce qu'il faut savoir pour utiliser l'application facilement au quotidien.</p>
      </div>
      <div class="flex items-center gap-2">
        <div class="relative w-full sm:w-64">
          <Search class="w-4 h-4 text-txt-dim absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Rechercher une explication..."
            class="input-field pl-9 py-2 text-sm w-full"
          />
        </div>
      </div>
    </div>

    <!-- Filtres par catégorie -->
    <div class="flex gap-2 overflow-x-auto pb-1 scrollbar-none">
      <button
        v-for="cat in categories"
        :key="cat.id"
        @click="activeCategory = cat.id"
        class="px-3.5 py-1.5 rounded-xl text-xs font-medium whitespace-nowrap transition-colors"
        :class="activeCategory === cat.id ? 'bg-neon-violet text-white shadow-neon-violet/30 shadow' : 'bg-bg-surface text-txt-dim hover:text-txt hover:bg-bg-hover'"
      >
        {{ cat.label }}
      </button>
    </div>

    <!-- Liste des cartes de documentation -->
    <div class="space-y-4">
      <div
        v-for="item in filteredDocs"
        :key="item.id"
        class="card hover:border-white/10 transition-all overflow-hidden"
      >
        <button
          @click="toggleItem(item.id)"
          class="w-full flex items-start sm:items-center justify-between gap-3 text-left p-1"
        >
          <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-xl flex items-center justify-center shrink-0" :class="item.iconBg">
              <component :is="item.icon" class="w-5 h-5" :class="item.iconColor" />
            </div>
            <div>
              <h3 class="font-gaming font-bold text-base text-txt">{{ item.title }}</h3>
              <p class="text-xs text-txt-dim mt-0.5">{{ item.subtitle }}</p>
            </div>
          </div>
          <div class="p-2 text-txt-dim hover:text-txt">
            <ChevronDown class="w-5 h-5 transition-transform duration-200" :class="{ 'rotate-180': openItems.includes(item.id) }" />
          </div>
        </button>

        <Transition name="expand">
          <div v-if="openItems.includes(item.id)" class="mt-4 pt-4 border-t border-white/5 space-y-3 text-sm text-txt leading-relaxed">
            <div v-for="(sec, idx) in item.sections" :key="idx" class="space-y-1.5">
              <h4 v-if="sec.heading" class="font-semibold text-neon-violet text-sm flex items-center gap-2">
                <span class="w-1.5 h-1.5 rounded-full bg-neon-violet"></span>
                {{ sec.heading }}
              </h4>
              <p class="text-txt-muted text-xs sm:text-sm pl-3.5">{{ sec.content }}</p>
              <ul v-if="sec.bullets" class="space-y-1 pl-6 list-disc text-txt-dim text-xs sm:text-sm">
                <li v-for="(b, bIdx) in sec.bullets" :key="bIdx">
                  <strong class="text-txt">{{ b.title }}:</strong> {{ b.desc }}
                </li>
              </ul>
              <div v-if="sec.tip" class="mt-2 p-3 rounded-xl bg-neon-blue/10 border border-neon-blue/20 flex items-start gap-2 text-xs text-neon-blue">
                <Info class="w-4 h-4 shrink-0 mt-0.5" />
                <span>{{ sec.tip }}</span>
              </div>
            </div>
          </div>
        </Transition>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  BookOpen, Search, ChevronDown, Monitor, PlayCircle, Users, Coins,
  Receipt, DollarSign, Cloud, Shield, Settings, MessageSquare, Info, Zap,
  UserCheck
} from 'lucide-vue-next'

const searchQuery = ref('')
const activeCategory = ref('all')
const openItems = ref<string[]>(['offline', 'sessions', 'app_name'])

function toggleItem(id: string) {
  if (openItems.value.includes(id)) {
    openItems.value = openItems.value.filter(x => x !== id)
  } else {
    openItems.value.push(id)
  }
}

const categories = [
  { id: 'all', label: 'Toutes les rubriques' },
  { id: 'general', label: 'Général & Cloud' },
  { id: 'jeu', label: 'Consoles & Sessions' },
  { id: 'clients', label: 'Joueurs & Fidélité' },
  { id: 'finance', label: 'Paiements & Tarifs' },
  { id: 'admin', label: 'Administration' },
]

const docs = [
  {
    id: 'offline',
    category: 'general',
    title: 'Fonctionnement hors-ligne et Synchronisation Cloud',
    subtitle: 'Comment l\'app marche même sans connexion internet',
    icon: Cloud,
    iconBg: 'bg-neon-blue/20',
    iconColor: 'text-neon-blue',
    sections: [
      {
        heading: '100% Fonctionnel sans Internet',
        content: 'Toutes vos données (joueurs, consoles, sessions, factures) sont d\'abord sauvegardées directement sur votre téléphone dans une base de données rapide (SQLite). Même s\'il y a une coupure de courant ou de réseau internet, vous pouvez continuer à lancer des sessions et encaisser les paiements.'
      },
      {
        heading: 'Synchronisation automatique avec Supabase',
        content: 'Dès que votre téléphone retrouve une connexion internet, l\'application synchronise automatiquement vos modifications avec le serveur Supabase. Vos collègues reçoivent vos nouvelles données et vous recevez les leurs.'
      },
      {
        tip: 'Vous pouvez forcer une mise à jour immédiate à tout moment depuis les Paramètres en appuyant sur "Synchroniser maintenant".'
      }
    ]
  },
  {
    id: 'sessions',
    category: 'jeu',
    title: 'Gérer une Session de Jeu (Chronomètre & Arrêt)',
    subtitle: 'Démarrer, mettre en pause et facturer le temps de jeu',
    icon: PlayCircle,
    iconBg: 'bg-neon-green/20',
    iconColor: 'text-neon-green',
    sections: [
      {
        heading: 'Démarrer une session',
        content: 'Pour lancer un joueur sur une console :',
        bullets: [
          { title: 'Choisir la console', desc: 'Cliquez sur "Démarrer" sur la console libre de votre choix (ou utilisez le bouton violet +).' },
          { title: 'Sélectionner le joueur', desc: 'Choisissez un joueur enregistré ou laissez en anonyme.' },
          { title: 'Sélectionner le jeu et le tarif', desc: 'Indiquez à quel jeu il joue et appliquez le tarif convenu (ex: 30 min, 1h, etc.).' }
        ]
      },
      {
        heading: 'Pendant le jeu : Chronomètre & Pause',
        content: 'Le chronomètre tourne seconde par seconde. Si le joueur doit s\'absenter temporairement (pause, appel), vous pouvez cliquer sur "Pause". Le temps s\'arrête. Cliquez sur "Reprendre" quand il revient.'
      },
      {
        heading: 'Terminer la session et encaisser',
        content: 'Cliquez sur "Terminer". L\'application calcule le montant exact selon la durée jouée et ouvre la fenêtre de facturation prête à l\'encaissement.'
      }
    ]
  },
  {
    id: 'consoles',
    category: 'jeu',
    title: 'Consoles et Postes de Jeu',
    subtitle: 'Comprendre et changer les statuts des consoles',
    icon: Monitor,
    iconBg: 'bg-neon-violet/20',
    iconColor: 'text-neon-violet',
    sections: [
      {
        heading: 'Les 5 états d\'une console',
        bullets: [
          { title: 'Disponible (Vert)', desc: 'La console est libre, allumée et prête pour un nouveau joueur.' },
          { title: 'Occupée (Violet)', desc: 'Une partie est en cours sur cette console.' },
          { title: 'En pause (Orange)', desc: 'La session en cours a été mise en pause.' },
          { title: 'Maintenance (Jaune)', desc: 'Manette en charge, nettoyage ou mise à jour en cours.' },
          { title: 'Hors service (Rouge)', desc: 'Console en panne ou temporairement inutilisable.' }
        ]
      },
      {
        heading: 'Modifier le statut d\'une console',
        content: 'Dans le menu "Consoles", cliquez sur l\'icône de modification (crayon) pour changer son nom, son numéro de poste ou son état (ex: passer en Maintenance).'
      }
    ]
  },
  {
    id: 'joueurs',
    category: 'clients',
    title: 'Gestion des Joueurs & Profils',
    subtitle: 'Enregistrer vos clients et suivre leurs visites',
    icon: Users,
    iconBg: 'bg-neon-blue/20',
    iconColor: 'text-neon-blue',
    sections: [
      {
        heading: 'Créer un joueur',
        content: 'Rendez-vous dans "Joueurs" et cliquez sur "Ajouter". Indiquez son nom et son numéro de téléphone. Le numéro de téléphone permet d\'identifier le joueur rapidement lors de ses prochaines visites.'
      },
      {
        heading: 'Fiche joueur & solde de jetons',
        content: 'En cliquant sur un joueur, vous voyez l\'historique de toutes ses sessions passées, le montant total qu\'il a dépensé au lounge, et son solde de jetons de fidélité disponibles.'
      }
    ]
  },
  {
    id: 'jetons',
    category: 'clients',
    title: 'Jetons de Fidélité & Paiement par Jeton',
    subtitle: 'Récompenser les clients et payer des parties avec des jetons',
    icon: Coins,
    iconBg: 'bg-neon-yellow/20',
    iconColor: 'text-neon-yellow',
    sections: [
      {
        heading: 'Comment un joueur gagne des jetons ?',
        content: 'Selon les réglages définis par l\'administrateur dans les Paramètres :',
        bullets: [
          { title: 'Par temps de jeu', desc: 'Exemple : 1 jeton offert toutes les 60 minutes de jeu.' },
          { title: 'Par montant dépensé', desc: 'Exemple : 1 jeton offert pour chaque tranche de 5 000 FC payée.' }
        ]
      },
      {
        heading: 'Payer une session avec des jetons',
        content: 'Lors de l\'encaissement d\'une facture, choisissez le mode de paiement "Jetons". L\'application vérifie si le joueur a assez de jetons dans son solde, convertit le montant en nombre de jetons et les déduit automatiquement.'
      },
      {
        tip: 'Toutes les transactions de jetons (gains, dépenses et remboursements) sont enregistrées avec l\'heure et la raison pour éviter toute confusion.'
      }
    ]
  },
  {
    id: 'tarifs',
    category: 'finance',
    title: 'Tarifs et Prix des Sessions',
    subtitle: 'Définir les prix selon la durée et le type de console',
    icon: DollarSign,
    iconBg: 'bg-neon-green/20',
    iconColor: 'text-neon-green',
    sections: [
      {
        heading: 'Création des tarifs',
        content: 'L\'administrateur configure les tarifs dans le menu "Tarifs". Chaque tarif a un nom (ex: "30 Minutes PS5"), une durée en minutes (ex: 30), et un prix en Francs Congolais (FC).'
      },
      {
        heading: 'Application automatique',
        content: 'Quand une session démarre, le tarif choisi applique automatiquement le prix. Si la session dépasse la durée prévue ou se termine plus tôt, le montant s\'ajuste proprement.'
      }
    ]
  },
  {
    id: 'factures',
    category: 'finance',
    title: 'Factures et Encaissements',
    subtitle: 'Modes de paiement et clôture de session',
    icon: Receipt,
    iconBg: 'bg-neon-violet/20',
    iconColor: 'text-neon-violet',
    sections: [
      {
        heading: 'Modes de paiement acceptés',
        bullets: [
          { title: 'Espèces (Cash)', desc: 'Paiement direct en monnaie courante.' },
          { title: 'Mobile Money', desc: 'M-Pesa, Airtel Money, Orange Money.' },
          { title: 'Carte bancaire', desc: 'Terminal de paiement ou virement.' },
          { title: 'Jetons de fidélité', desc: 'Déduction directe sur la tirelire de jetons du joueur.' }
        ]
      },
      {
        heading: 'Modification de paiement en cas d\'erreur',
        content: 'Si vous avez sélectionné "Espèces" alors que le client a payé par "M-Pesa" ou "Jetons", vous pouvez modifier la facture dans "Paiements". Le système s\'occupe de recalculer les jetons si nécessaire.'
      }
    ]
  },
  {
    id: 'app_name',
    category: 'admin',
    title: 'Nom du Lounge (Changer le nom pour tout le monde)',
    subtitle: 'Personnaliser le nom de l\'application synchronisé sur tous les téléphones',
    icon: Settings,
    iconBg: 'bg-neon-violet/20',
    iconColor: 'text-neon-violet',
    sections: [
      {
        heading: 'Comment changer le nom du lounge ?',
        content: '1. Connectez-vous avec un compte Administrateur.\n2. Allez dans "Paramètres" (menu de gauche).\n3. Dans la section "Configuration générale", saisissez le nouveau nom dans la case "Nom de l\'application" (ex: "Play Zone Lubumbashi").\n4. Cliquez sur "Enregistrer".'
      },
      {
        heading: 'Synchronisation instantanée grâce à Supabase',
        content: 'Dès que vous enregistrez le nom, il est sauvegardé dans Supabase. Tous les autres téléphones et tablettes de l\'équipe qui synchronisent recevront automatiquement ce nouveau nom, qui apparaîtra sur la barre de menu, l\'écran de connexion et les titres !'
      }
    ]
  },
  {
    id: 'roles',
    category: 'admin',
    title: 'Rôles : Administrateur vs Employé',
    subtitle: 'Comprendre les accès et permissions de chaque profil',
    icon: Shield,
    iconBg: 'bg-neon-blue/20',
    iconColor: 'text-neon-blue',
    sections: [
      {
        heading: 'Le profil Employé (Gestion quotidienne)',
        bullets: [
          { title: 'Sessions & Consoles', desc: 'Démarrer, mettre en pause, arrêter les sessions.' },
          { title: 'Joueurs', desc: 'Créer de nouveaux profils et consulter les jetons.' },
          { title: 'Paiements', desc: 'Encaisser les factures et changer le mode de paiement.' },
          { title: 'Catalogue Jeux', desc: 'Voir les jeux disponibles pour renseigner les clients.' },
          { title: 'Messages', desc: 'Envoyer et lire les messages de l\'équipe.' }
        ]
      },
      {
        heading: 'Le profil Administrateur (Gestion complète)',
        content: 'En plus de toutes les fonctions de l\'employé, l\'administrateur peut :',
        bullets: [
          { title: 'Rapports & Chiffre d\'affaires', desc: 'Consulter les statistiques financières journalières et mensuelles.' },
          { title: 'Gestion des tarifs & consoles', desc: 'Ajouter, modifier ou archiver des consoles et des grilles tarifaires.' },
          { title: 'Gestion des utilisateurs', desc: 'Créer les comptes employés, modifier les mots de passe et les permissions (connexion internet requise : les comptes sont gérés en ligne).' },
          { title: 'Paramètres du lounge', desc: 'Changer le nom de l\'application et les règles de fidélité.' }
        ]
      }
    ]
  },
  {
    id: 'comptes',
    category: 'admin',
    title: 'Comptes du personnel : tout se passe en ligne',
    subtitle: 'Pourquoi le téléphone ne garde aucun mot de passe',
    icon: UserCheck,
    iconBg: 'bg-neon-violet/20',
    iconColor: 'text-neon-violet',
    sections: [
      {
        heading: 'Les comptes vivent sur le serveur, pas sur le téléphone',
        content: 'Les comptes du personnel sont enregistrés uniquement sur le serveur. Le téléphone garde seulement le nom, l\'email et le rôle de la personne connectée, pour pouvoir écrire \"session démarrée par Jean\". Aucun mot de passe n\'est gardé sur l\'appareil, même en secret. Si le téléphone est volé, il n\'y a aucun mot de passe à voler.'
      },
      {
        heading: 'Se connecter demande internet',
        content: 'Au moment de la connexion, c\'est le serveur qui vérifie le mot de passe. Il faut donc une connexion internet pour ouvrir une session. Une fois connecté, vous pouvez travailler sans réseau : la session reste ouverte.'
      },
      {
        heading: 'Créer, modifier ou supprimer un compte',
        content: 'Dans Administration > Utilisateurs, chaque action (créer un compte, changer un mot de passe, changer le rôle, archiver ou supprimer) part directement sur le serveur. S\'il n\'y a pas de réseau, l\'application le dit clairement et ne fait rien à moitié : vous réessayez quand internet revient.'
      },
      {
        heading: 'Quand un compte est supprimé',
        bullets: [
          { title: 'Sur l\'appareil de la personne', desc: 'L\'application s\'en aperçoit toute seule en moins d\'une minute (ou dès son ouverture).' },
          { title: 'Effacement automatique', desc: 'Toutes les données de l\'application sur cet appareil sont effacées : sessions, joueurs, factures, historique.' },
          { title: 'Retour au login', desc: 'L\'écran de connexion s\'affiche aussitôt, avec un message expliquant que le compte n\'est plus actif.' }
        ]
      },
      {
        tip: 'Se déconnecter volontairement ramène aussi tout de suite à l\'écran de connexion. Les données du lounge restent alors sur l\'appareil : seule la session est fermée.'
      }
    ]
  },
  {
    id: 'messages',
    category: 'general',
    title: 'Messagerie d\'Équipe',
    subtitle: 'Communiquer entre employés et gérants',
    icon: MessageSquare,
    iconBg: 'bg-neon-green/20',
    iconColor: 'text-neon-green',
    sections: [
      {
        heading: 'Passage de consignes',
        content: 'Le menu "Messages" permet de laisser des notes pour les prochains shifts (ex: "Penser à charger les manettes du poste 4", "Client VIP John est passé à 14h"). Tous les utilisateurs connectés peuvent voir et répondre.'
      }
    ]
  }
]

const filteredDocs = computed(() => {
  return docs.filter(item => {
    const matchesCat = activeCategory.value === 'all' || item.category === activeCategory.value
    if (!matchesCat) return false
    if (!searchQuery.value.trim()) return true
    const q = searchQuery.value.toLowerCase()
    return item.title.toLowerCase().includes(q) ||
           item.subtitle.toLowerCase().includes(q) ||
           item.sections.some(s => (s.heading && s.heading.toLowerCase().includes(q)) || (s.content && s.content.toLowerCase().includes(q)))
  })
})
</script>

<style scoped>
.expand-enter-active, .expand-leave-active {
  transition: all 0.25s ease-out;
  overflow: hidden;
}
.expand-enter-from, .expand-leave-to {
  opacity: 0;
  max-height: 0;
  transform: translateY(-4px);
}
.expand-enter-to, .expand-leave-from {
  opacity: 1;
  max-height: 1000px;
  transform: translateY(0);
}
</style>
