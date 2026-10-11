'use strict';
const {getStatus,assertChallenge}=require('./quantic-id-bridge.cjs');
const API_ORIGIN='https://quanticminds.onrender.com';
const SOCIAL_ORIGIN='https://mediumorchid-badger-314305.hostingersite.com';
const SOCIAL_RE=/^https:\/\/mediumorchid-badger-314305\.hostingersite\.com\/pulse\/(?:[?#]|$)/i;
const TOKEN_KEY='quantic_pulse_token';
function isSocialPage(url){return SOCIAL_RE.test(String(url||''))}
async function postJson(path,payload,{fetcher=globalThis.fetch,bearer='',timeout=8500}={}){
  if(!['/api/pulse/auth/quantic/challenge','/api/pulse/auth/quantic/complete'].includes(path))
    throw Error('quantic_id_api_path');
  const controller=new AbortController();
  const timer=setTimeout(()=>controller.abort(),timeout);
  try{
    const response=await fetcher(API_ORIGIN+path,{
      method:'POST',redirect:'error',signal:controller.signal,
      headers:{'content-type':'application/json',...(bearer?{authorization:'Bearer '+bearer}:{})},
      body:JSON.stringify(payload||{})
    });
    const json=await response.json();
    if(!response.ok)throw Error(typeof json?.error==='string'?json.error:'quantic_id_server_'+response.status);
    return json;
  }finally{clearTimeout(timer)}
}
async function authenticateQuanticSocial({fetcher=globalThis.fetch,transport,existingToken=''}={}){
  const status=await getStatus({transport});
  if(!status.unlocked)return {ok:false,error:status.connected?'quantic_id_locked':'quantic_secure_offline'};
  try{
    const {challenge}=await postJson('/api/pulse/auth/quantic/challenge',{}, {fetcher});
    const assertion=await assertChallenge(challenge,{transport});
    if(status.keyId && assertion.keyId!==status.keyId)throw Error('identity_switched');
    const result=await postJson('/api/pulse/auth/quantic/complete',
      {challenge,...assertion},{fetcher,bearer:existingToken});
    if(typeof result.token!=='string'||result.token.length<24||
       !result.user?.id||Number(result.expiresInMs)<5000)throw Error('quantic_id_invalid_session');
    // A recognized identity is not sufficient: the Pulse /me endpoint must accept this token.
    const verify=await fetcher(API_ORIGIN+'/api/pulse/me',{
      method:'GET',redirect:'error',headers:{authorization:'Bearer '+result.token}
    });
    if(!verify.ok)throw Error('social_session_not_authenticated');
    const valid=await verify.json();
    if(!valid.user?.id||String(valid.user.id)!==String(result.user.id))throw Error('social_session_user_mismatch');
    return {ok:true,token:result.token,user:result.user,expiresInMs:Number(result.expiresInMs),keyId:assertion.keyId};
  }catch(error){return{ok:false,error:String(error?.message||'quantic_id_failure').slice(0,120)}}
}
async function installSocialSession(wc,session) {
  if(!wc||wc.isDestroyed()||!isSocialPage(wc.getURL())||!session?.ok)return false;
  const data=JSON.stringify({token:session.token,user:session.user});
  const script=`(async () => {
    if (location.origin !== 'https://mediumorchid-badger-314305.hostingersite.com' ||
        location.pathname !== '/pulse/') return false;
    const session=${data};
    localStorage.setItem('quantic_pulse_token',session.token);
    window.dispatchEvent(new CustomEvent('quantic-gekko-session',{detail:session}));
    // Updating the SPA's in-memory state is essential; localStorage alone is insufficient.
    try {
      const mod=await import('/pulse/session.js');
      if(typeof mod.applySession!=='function')throw Error('missing_session_module');
      mod.applySession(session);
      sessionStorage.removeItem('gekko_social_session_bootstrap');
    } catch {
      if(!sessionStorage.getItem('gekko_social_session_bootstrap')){
        sessionStorage.setItem('gekko_social_session_bootstrap','1');
        location.reload();
      }
    }
    return true;
  })()`;
  return Boolean(await wc.executeJavaScript(script,true));
}
async function revokeSocialSession(wc,token){
  if(!wc||wc.isDestroyed()||!isSocialPage(wc.getURL())||!token)return false;
  const script=`(async () => {
    if (location.origin !== 'https://mediumorchid-badger-314305.hostingersite.com' ||
        location.pathname !== '/pulse/') return false;
    if(localStorage.getItem('quantic_pulse_token')!==${JSON.stringify(token)})return false;
    localStorage.removeItem('quantic_pulse_token');
    sessionStorage.removeItem('gekko_social_session_bootstrap');
    window.dispatchEvent(new CustomEvent('quantic-gekko-session',{detail:{token:'',user:null}}));
    try {
      const mod=await import('/pulse/session.js');
      if(typeof mod.clearSession==='function')mod.clearSession();
    } catch {}
    return true;
  })()`;
  return Boolean(await wc.executeJavaScript(script,true));
}
module.exports={authenticateQuanticSocial,installSocialSession,revokeSocialSession,
  isSocialPage,postJson,TOKEN_KEY,API_ORIGIN,SOCIAL_ORIGIN};
