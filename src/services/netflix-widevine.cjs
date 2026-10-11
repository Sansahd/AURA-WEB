'use strict';
// Probe the standard Encrypted Media Extensions API on the official Netflix
// HTTPS origin. This neither extracts decryption keys nor bypasses any DRM.
const KEY_SYSTEM='com.widevine.alpha';
function isNetflixOrigin(url){
  try {
    const parsed=new URL(url);
    return parsed.protocol==='https:' &&
      (parsed.hostname==='netflix.com' || parsed.hostname.endsWith('.netflix.com'));
  }catch{return false}
}
const PROBE=`(async () => {
  if(typeof navigator.requestMediaKeySystemAccess!=='function') return false;
  const requested={
    initDataTypes:['cenc'],
    videoCapabilities:[{contentType:'video/mp4; codecs="avc1.42E01E"'}],
    audioCapabilities:[{contentType:'audio/mp4; codecs="mp4a.40.2"'}],
    distinctiveIdentifier:'optional',persistentState:'optional',
    sessionTypes:['temporary']
  };
  try{
    await navigator.requestMediaKeySystemAccess('com.widevine.alpha',[requested]);
    return true;
  }catch{return false}
})()`;
async function supportsEmbeddedWidevine(wc){
  if(!wc||wc.isDestroyed()||!isNetflixOrigin(wc.getURL()))return null;
  return Boolean(await wc.executeJavaScript(PROBE,true));
}
module.exports={KEY_SYSTEM,PROBE,isNetflixOrigin,supportsEmbeddedWidevine};
