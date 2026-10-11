'use strict';
const {app,ipcMain}=require('electron');
const {installIpcFirewall}=require('./security/ipc-firewall.cjs');
const {registerQuanticUiScheme,SHELL_URL}=require('./services/ui-protocol.cjs');
const {prepareProtectedMedia}=require('./services/protected-media.cjs');

registerQuanticUiScheme();
app.enableSandbox();
installIpcFirewall(ipcMain,{shellUrl:SHELL_URL});

(async()=>{
  await app.whenReady();
  // Castlabs recommends completing CDM initialization before creating
  // BrowserWindow/WebContentsViews. Limit the startup wait so offline browsing
  // stays available even if Widevine's official update service is unreachable.
  const drm=await Promise.race([
    prepareProtectedMedia(),
    new Promise(resolve=>{
      const timer=setTimeout(()=>resolve({ok:false,reason:'startup_drm_timeout'}),15000);
      timer.unref?.();
    })
  ]);
  if(drm.ok)console.log('[drm] Widevine component stage initialized');
  else console.warn('[drm] protected playback pending:',drm.reason);
  require('./main.cjs');
})().catch(error=>{
  console.error('[bootstrap] Quantic startup failed:',error?.stack||error);
  app.quit();
});
