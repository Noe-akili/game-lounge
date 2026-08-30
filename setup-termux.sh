#!/bin/bash
# Game Lounge - Setup Termux
# Installe et configure tout pour l'app

echo "🎮 Game Lounge - Installation..."
echo ""

# Installer les dépendances système
pkg install -y nodejs-lts git

# Installer les dépendances npm
echo "📦 Installation des dépendances..."
cd "$(dirname "$0")"
npm install

# Construire le frontend
echo "🔨 Build du frontend..."
npm run build

# Initialiser la base de données
echo "🗄️ Base de données..."
npm run seed

echo ""
echo "✅ Installation terminée!"
echo ""
echo "Pour démarrer le serveur:"
echo "  cd $(pwd)"
echo "  npm run server"
echo ""
echo "L'app sera accessible sur http://localhost:3001"
echo "Ouvrez l'APK Game Lounge et elle se connectera automatiquement."
