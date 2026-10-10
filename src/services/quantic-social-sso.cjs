'use strict';
const {getStatus,assertChallenge}=require('./quantic-id-bridge.cjs');
const API_ORIGIN='https://quanticminds.onrender.com';
const SOCIAL_RE=/^https:\/\/xdsawyerlol\.github\.io\/QuanticSillage\/zoon(?:\.html)?(?:[?#]|$)/i;
const TOKEN_KEY='zoon_token';
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
       !result.user?.id||Number(result.expiresInMs)!==12000)throw Error('quantic_id_invalid_session');
    return {ok:true,token:result.token,user:result.user,expiresInMs:12000,keyId:assertion.keyId};
  }catch(error){return{ok:false,error:String(error?.message||'quantic_id_failure').slice(0,120)}}
}
async function installSocialSession(wc,session) {
  if(!wc||wc.isDestroyed()||!isSocialPage(wc.getURL())||!session?.ok)return false;
  const data=JSON.stringify({token:session.token,user:session.user});
  // No token in URLs, navigation, logs, or the shell renderer.
  const script=`(() => {
    if (location.origin !== 'https://xdsawyerlol.github.io' ||
        !/^\\/QuanticSillage\\/zoon(?:\\.html)?$/.test(location.pathname)) return false;
    const session=${data};
    localStorage.setItem('zoon_token',session.token);
    window.dispatchEvent(new CustomEvent('quantic-gekkko-session',{detail:session}));
    return true;
  })()`;
  return Boolean(await wc.executeJavaScript(script,true));
}
async function revokeSocialSession(wc,token){
  if(!wc||wc.isDestroyed()||!isSocialPage(wc.getURL())||!token)return false;
  const script=`(() => {
    if (location.origin !== 'https://xdsawyerlol.github.io' ||
        !/^\\/QuanticSillage\\/zoon(?:\\.html)?$/.test(location.pathname)) return false;
    if(localStorage.getItem('zoon_token')!==${JSON.stringify(token)})return false;
    localStorage.removeItem('zoon_token');
    window.dispatchEvent(new CustomEvent('quantic-gekkko-session',{detail:{token:'',user:null}}));
    return true;
  })()`;
  return Boolean(await wc.executeJavaScript(script,true));
}
module.exports={authenticateQuanticSocial,installSocialSession,revokeSocialSession,
  isSocialPage,postJson,TOKEN_KEY,API_ORIGIN};
