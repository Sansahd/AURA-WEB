'use strict';
const { contextBridge, ipcRenderer }=require('electron');
contextBridge.exposeInMainWorld('gekkoSize',{
 choose:(size)=>ipcRenderer.send('gekko-size-choice',size),
 dismiss:()=>ipcRenderer.send('gekko-size-dismiss')
});
