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
  // GEKKO opens immediately while the licensed Widevine component initializes
  // through Castlabs' component service in the background. Netflix SideStage
  // waits on the very same promise; normal browsing never stalls.
  void prepareProtectedMedia().then(result=>{
    if(result.ok)console.log('[drm] component service ready; Netflix support requires additional verification.');
    else console.warn('[drm] unavailable:',result.reason);
  });
  require('./main.cjs');
})().catch(error=>{
  console.error('[bootstrap] Quantic startup failed:',error?.stack||error);
  app.quit();
});
