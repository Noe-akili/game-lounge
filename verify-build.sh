#!/bin/bash

# Test script to verify the standalone application build

echo "=== Testing Game Lounge Standalone Application ===\n"

# Check if frontend files exist
if [ ! -f "dist/index.html" ]; then
    echo "❌ Frontend not built - dist/index.html not found"
    exit 1
fi

echo "✅ Frontend built successfully"

# Check if backend files exist
if [ ! -f "server-dist/server.js" ]; then
    echo "❌ Backend not built - server-dist/server.js not found"
    exit 1
fi

echo "✅ Backend built successfully"

# Check if sql-wasm.wasm exists
if [ ! -f "dist/sql-wasm.wasm" ]; then
    echo "⚠️  Warning: sql-wasm.wasm not found in dist/"
fi

echo "✅ Essential files are present\n"

# Test if node is available
if command -v node >/dev/null 2>&1; then
    NODE_VERSION=$(node --version)
    echo "✅ Node.js found: $NODE_VERSION"
else
    echo "⚠️  Warning: Node.js not installed or not in PATH"
    echo "   The application will attempt to start Node.js on startup"
fi

echo "\n=== Build Verification Summary ==="
echo "Frontend: dist/ - Contains Vue.js application"
echo "Backend:  server-dist/ - Contains standalone Node.js server"
echo "Electron: electron/ - Contains Electron main and preload scripts"
echo "\nNext Steps:"
echo "1. Install Electron and dependencies: npm install --legacy-peer-deps"
echo "2. Build the Electron application: npm run build:electron"
echo "3. Test with: npm run build:electron"
echo "\nThe application will:"
echo "- Start the backend server automatically"
echo "- Open Microsoft Edge or default browser to http://127.0.0.1:3001"
echo "- Display the Game Lounge application"

echo "\n=== Verification Complete ==="
