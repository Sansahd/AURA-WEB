'use strict';
const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('gekkoPicker', {
  choose: (engine) => ipcRenderer.send('gekko-picker-choice', engine),
  dismiss: () => ipcRenderer.send('gekko-picker-dismiss')
});
