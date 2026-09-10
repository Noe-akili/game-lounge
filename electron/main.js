// @ts-nocheck
const { app, BrowserWindow, ipcMain, shell, dialog } = require('electron')
const path = require('path')
const { spawn } = require('child_process')
const fs = require('fs')

let mainWindow = null
let serverProcess = null

function createWindow() {
  const isDev = !app.isPackaged

  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1000,
    minHeight: 700,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
    title: 'Game Lounge',
    show: false,
  })

  // Load the built frontend
  const indexPath = path.join(__dirname, '../dist/index.html')
  mainWindow.loadFile(indexPath)

  if (isDev) {
    mainWindow.webContents.openDevTools()
  }

  mainWindow.once('ready-to-show', () => {
    mainWindow.show()
  })

  mainWindow.on('closed', () => {
    mainWindow = null
  })

  return mainWindow
}

function startBackendServer() {
  const serverPath = path.join(__dirname, '../dist/server.js')

  if (!fs.existsSync(serverPath)) {
    console.error('[Electron] Server file not found at:', serverPath)
    dialog.showErrorBox(
      'Erreur de démarrage',
      'Le fichier du backend serveur est introuvable. Veuillez réinstaller l\'application.'
    )
    return
  }

  const env = {
    ...process.env,
    DATADIR: path.join(app.getPath('userData'), 'data'),
    PORT: '3001',
  }

  serverProcess = spawn(process.execPath, [serverPath], {
    env,
    detached: true,
    stdio: 'pipe',
  })

  serverProcess.stdout.on('data', (data) => {
    console.log(`[Backend] ${data.toString().trim()}`)
  })

  serverProcess.stderr.on('data', (data) => {
    console.error(`[Backend Error] ${data.toString().trim()}`)
  })

  serverProcess.on('error', (err) => {
    console.error('[Electron] Failed to start backend:', err)
    dialog.showMessageBox(mainWindow, {
      type: 'error',
      title: 'Backend Error',
      message: 'Impossible de démarrer le serveur backend.',
      detail: err.message,
    })
  })

  serverProcess.on('exit', (code) => {
    console.log(`[Electron] Backend server exited with code ${code}`)
  })
}

function openDefaultBrowser() {
  const url = 'http://127.0.0.1:3001'
  // Wait a moment for server to start, then open browser
  setTimeout(() => {
    shell.openExternal(url, { app: 'Microsoft Edge' }).catch(() => {
      // Try default if Edge not available
      shell.openExternal(url)
    })
  }, 2000)
}

app.whenReady().then(() => {
  createWindow()

  // Start backend server
  startBackendServer()

  // Open default browser
  openDefaultBrowser()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  // Kill backend server
  if (serverProcess) {
    serverProcess.kill()
    serverProcess = null
  }
  if (mainWindow) {
    mainWindow.destroy()
    mainWindow = null
  }
  app.quit()
})

// IPC handlers
ipcMain.on('open-external', (event, url) => {
  shell.openExternal(url)
})

ipcMain.on('show-error', (event, message) => {
  dialog.showMessageBox(mainWindow, {
    type: 'error',
    title: 'Error',
    message,
  })
})

ipcMain.on('get-user-data-path', (event) => {
  event.reply('user-data-path', app.getPath('userData'))
})

ipcMain.on('get-app-version', (event) => {
  event.reply('app-version', app.getVersion())
})