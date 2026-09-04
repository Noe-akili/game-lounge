#!/bin/bash
# Deploy backend to VPS
# Usage: ./deploy-backend.sh user@your-server.com

set -e

SERVER=$1
if [ -z "$SERVER" ]; then
  echo "Usage: $0 user@your-server.com"
  exit 1
fi

echo "📦 Building backend bundle..."
cd server
npm ci
npm run build
cd ..

echo "📤 Copying files to server..."
rsync -avz --exclude 'node_modules' --exclude '.git' --exclude 'dist' \
  server/ $SERVER:~/game-lounge/server/

echo "🐳 Deploying with Docker Compose..."
ssh $SERVER << 'EOF'
  cd ~/game-lounge
  docker-compose up -d --build
  docker-compose ps
EOF

echo "✅ Backend deployed!"
echo "🌐 API available at: http://your-server.com:3001"