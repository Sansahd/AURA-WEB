'use strict';
// Electron for Content Security supports Widevine component installation.
// No manual CDM extraction, sideloading, third-party playback or DRM bypass.
// Both bootstrap and the SideStage Netflix action share this ONE startup task.
const { components } = require('electron');
let installation = null;
function drmAvailable() {
  return Boolean(components && typeof components.whenReady === 'function');
}
function prepareProtectedMedia() {
  if (installation) return installation;
  if (!drmAvailable()) return Promise.resolve({
    ok:false,reason:'widevine_runtime_unavailable',status:null
  });
  installation = Promise.resolve().then(async () => {
    try {
      await components.whenReady();
      const status=typeof components.status==='function'?components.status():null;
      // This confirms the component update stage, not Netflix certification.
      return {ok:true,reason:'component_stage_ready',status};
    } catch (error) {
      return {ok:false,reason:String(error?.message||'widevine_installation_failed').slice(0,150),status:null};
    }
  });
  return installation;
}
async function awaitProtectedMedia(timeout=25000){
  let timer;
  try{
    return await Promise.race([
      prepareProtectedMedia(),
      new Promise(resolve=>{timer=setTimeout(()=>resolve({ok:false,reason:'widevine_component_timeout',status:null}),timeout);timer.unref?.();})
    ]);
  }finally{clearTimeout(timer)}
}
module.exports={drmAvailable,prepareProtectedMedia,awaitProtectedMedia};
