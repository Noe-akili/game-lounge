// Preload script for Electron
const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('electronAPI', {
  openExternal: (url) => ipcRenderer.send('open-external', url),
  showError: (message) => ipcRenderer.send('show-error', message),
  getUserDataPath: () => {
    return new Promise((resolve) => {
      ipcRenderer.send('get-user-data-path')
      ipcRenderer.once('user-data-path', (event, path) => resolve(path))
    })
  },
  getAppVersion: () => {
    return new Promise((resolve) => {
      ipcRenderer.send('get-app-version')
      ipcRenderer.once('app-version', (event, version) => resolve(version))
    })
  },
})