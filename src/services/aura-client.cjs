'use strict';

// GEKKO's AURA is CLOUD-first. Neither a running PC daemon nor Ollama is
// required for the normal hosted /api/chat experience. Optional local engines
// are tried only after the configured cloud is unreachable.
const DEFAULT_AURA_CLOUD_URL = 'https://antiquewhite-dolphin-780448.hostingersite.com';
const DEFAULT_AURA_URL = 'http://127.0.0.1:8787';
const DEFAULT_OLLAMA_URL = 'http://127.0.0.1:11434';
const DEFAULT_OLLAMA_MODEL = 'gemma3:12b';
const MAX_RESPONSE_BYTES = 1_048_576;
const LOOPBACK = new Set(['127.0.0.1','localhost','::1']);

function cleanBaseUrl(value, fallback, {cloud=false}={}) {
  const raw=String(value||fallback||'').trim();
  const parsed=new URL(raw);
  if(parsed.username||parsed.password||parsed.search||parsed.hash||
     (parsed.pathname!==''&&parsed.pathname!=='/'))throw Error('AURA : adresse de serveur invalide');
  const host=parsed.hostname.replace(/^\[|\]$/g,'').toLowerCase();
  const local=LOOPBACK.has(host);
  if(cloud){
    if(parsed.protocol!=='https:'&&!local)throw Error('AURA Cloud exige HTTPS');
  }else{
    if(!local)throw Error('Les moteurs IA locaux doivent être sur la boucle locale');
    if(!['http:','https:'].includes(parsed.protocol))throw Error('Protocole IA local invalide');
  }
  return parsed.origin;
}
function cleanError(error) {
  const value=String(error?.message||error||'échec inconnu');
  if(/fetch failed|ECONNREFUSED|ENOTFOUND|ETIMEDOUT|EHOSTUNREACH|ENETUNREACH/i.test(value))
    return 'serveur injoignable (réseau, DNS ou service arrêté)';
  if(/abort|timeout/i.test(value))return 'délai de réponse dépassé';
  return value.slice(0,240);
}
async function jsonRequest(fetcher,url,options,timeoutMs,onResponse=null){
  const controller=new AbortController();
  const timer=setTimeout(()=>controller.abort(),timeoutMs);
  timer.unref?.();
  try{
    const response=await fetcher(url,{...options,redirect:'error',signal:controller.signal});
    if(typeof onResponse==='function')onResponse(response);
    const text=await response.text();
    if(text.length>MAX_RESPONSE_BYTES)throw Error('Réponse AURA trop volumineuse');
    let data={};
    try{data=text?JSON.parse(text):{}}catch{throw Error('Réponse AURA non JSON')}
    if(!response.ok){
      const reason=String(data.error||data.detail||data.message||'').slice(0,180);
      throw Error('HTTP '+response.status+(reason?': '+reason:''));
    }
    return data;
  }finally{clearTimeout(timer)}
}
class QuanticAuraClient {
  constructor({
    fetchImpl=globalThis.fetch,
    cloudUrl=process.env.QUANTIC_AURA_CLOUD_URL||process.env.AURA_CLOUD_URL||DEFAULT_AURA_CLOUD_URL,
    auraUrl=process.env.QUANTIC_AURA_URL||DEFAULT_AURA_URL,
    ollamaUrl=process.env.QUANTIC_OLLAMA_URL||DEFAULT_OLLAMA_URL,
    ollamaModel=process.env.QUANTIC_OLLAMA_MODEL||DEFAULT_OLLAMA_MODEL,
    allowLocalFallback=true,
    cloudTimeoutMs=16000,
    localTimeoutMs=8000,
  }={}){
    if(typeof fetchImpl!=='function')throw Error('fetch indisponible');
    this.fetch=fetchImpl;
    this.cloudUrl=cleanBaseUrl(cloudUrl,DEFAULT_AURA_CLOUD_URL,{cloud:true});
    this.auraUrl=cleanBaseUrl(auraUrl,DEFAULT_AURA_URL);
    this.ollamaUrl=cleanBaseUrl(ollamaUrl,DEFAULT_OLLAMA_URL);
    this.ollamaModel=String(ollamaModel||DEFAULT_OLLAMA_MODEL).slice(0,180);
    this.allowLocalFallback=Boolean(allowLocalFallback);
    this.cloudTimeoutMs=Math.max(1000,Math.min(120000,Number(cloudTimeoutMs)||16000));
    this.localTimeoutMs=Math.max(1000,Math.min(120000,Number(localTimeoutMs)||8000));
    this.cloudSessionCookie=''; // in-memory, never persisted or sent to local services
  }
  async generate({
    prompt,system='',taskRole='auto',maxTokens=500,
    distributed=false,maxAgents=3
  }={}){
    const text=String(prompt||'').trim().slice(0,60000);
    if(!text)throw Error('Prompt vide');
    const tokens=Math.max(64,Math.min(Number(maxTokens||500),4000));
    let cloudError='';
    try{
      const data=await jsonRequest(this.fetch,this.cloudUrl+'/api/chat',{
        method:'POST',
        headers:{'content-type':'application/json','accept':'application/json','user-agent':'GEKKO/AURA-Cloud',...(this.cloudSessionCookie?{cookie:this.cloudSessionCookie}:{})},
        body:JSON.stringify({text,author:'Utilisateur GEKKO'})
      },this.cloudTimeoutMs,(response)=>{
        // AURA's public /api/chat uses a signed session cookie for continuity.
        // Scope to this HTTPS host; never persist it or reuse it for Ollama.
        const value=String(response.headers?.get?.('set-cookie')||'');
        const match=/(?:^|[,;\s])aura_chat_session=([^;\s,]+)/i.exec(value);
        if(match && match[1].length<=250)
          this.cloudSessionCookie='aura_chat_session='+match[1];
      });
      const answer=String(data.answer||'').trim();
      if(!answer)throw Error('AURA Cloud a renvoyé une réponse vide');
      return {answer,engine:'aura-cloud',model:String(data.model||'noyau natif'),
        role:String(data.role||taskRole||'auto'),distributed:false,moa:false,
        cloudUrl:this.cloudUrl,verification:data.verification||{},
        metrics:data.metrics||{}};
    }catch(error){
      cloudError=cleanError(error);
    }
    if(!this.allowLocalFallback){
      throw Error('AURA Cloud indisponible ('+cloudError+'). AURA locale désactivée.');
    }
    let localError='';
    try{
      const data=await jsonRequest(this.fetch,this.auraUrl+'/api/ai/generate',{
        method:'POST',
        headers:{'content-type':'application/json','accept':'application/json'},
        body:JSON.stringify({
          prompt:text,system_instruction:String(system||'').slice(0,12000),
          task_role:String(taskRole||'auto').slice(0,80),max_tokens:tokens,
          distributed:Boolean(distributed),max_agents:Math.max(2,Math.min(Number(maxAgents||3),3)),
          source:'gekko'
        })
      },this.localTimeoutMs);
      const answer=String(data.answer||'').trim();
      if(!answer)throw Error('AURA locale a renvoyé une réponse vide');
      return {answer,engine:String(data.engine||'aura-local'),model:String(data.model||''),
        role:String(data.role||taskRole||'auto'),distributed:Boolean(data.distributed),
        moa:Boolean(data.moa),cloudError};
    }catch(error){localError=cleanError(error)}
    try{
      const data=await jsonRequest(this.fetch,this.ollamaUrl+'/api/generate',{
        method:'POST',headers:{'content-type':'application/json'},
        body:JSON.stringify({
          model:this.ollamaModel,prompt:text,system:String(system||'').slice(0,12000),
          stream:false,options:{num_predict:Math.max(64,Math.min(tokens,1200))}
        })
      },this.localTimeoutMs);
      const answer=String(data.response||'').trim();
      if(!answer)throw Error('Ollama a renvoyé une réponse vide');
      return {answer,engine:'ollama-direct',model:this.ollamaModel,
        distributed:false,moa:false,fallbackFrom:'aura-cloud',cloudError,localError};
    }catch(error){
      throw Error('AURA Cloud indisponible ('+cloudError+'). '+
        'Secours local AURA : '+localError+'. '+
        'Ollama facultatif : '+cleanError(error)+'.');
    }
  }
}
module.exports={QuanticAuraClient,cleanBaseUrl,cleanError,
  DEFAULT_AURA_CLOUD_URL,DEFAULT_AURA_URL,DEFAULT_OLLAMA_URL};
