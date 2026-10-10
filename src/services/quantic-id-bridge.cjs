'use strict';
// Quantic Secure v1/v3 bridge: strictly localhost, no arbitrary URL, no private
// key export. The unlocked key stays on the USB/companion; GEKKO only receives
// public status and an explicit, nonce-bound cryptographic assertion.
const http=require('node:http');
const HOST='127.0.0.1',PORT=47621;
function readJson(path,method='GET',payload=null,{transport=http.request,timeout=1800}={}){
  return new Promise((resolve,reject)=>{
    const body=payload===null?'':JSON.stringify(payload);
    const req=transport({hostname:HOST,port:PORT,path,method,timeout,
      headers:{accept:'application/json',...(body?{'content-type':'application/json','content-length':Buffer.byteLength(body)}:{})}},
      res=>{
        if(res.statusCode!==200){res.resume?.();reject(new Error('quantic_secure_http_'+res.statusCode));return;}
        const chunks=[];let bytes=0;
        res.on('data',chunk=>{bytes+=chunk.length;if(bytes>8192){req.destroy(new Error('quantic_secure_oversize'));return;}chunks.push(chunk);});
        res.on('end',()=>{try{resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')))}catch{reject(new Error('quantic_secure_invalid_json'))}});
        res.on('error',reject);
      });
    req.on('timeout',()=>req.destroy(new Error('quantic_secure_timeout')));
    req.on('error',reject);
    if(body)req.write(body);
    req.end();
  });
}
function normalizeStatus(data){
  const version=Number(data?.version||data?.protocolVersion||0);
  if(!Number.isInteger(version)||version<1||version>3) return {connected:true,available:false,unlocked:false,reason:'unsupported_protocol',version};
  const available=data.identityAvailable===true || data.available===true || data.keyPresent===true;
  const unlocked=available&&(data.unlocked===true || data.identityUnlocked===true ||
    data.locked===false || data.state==='unlocked' || data.status==='unlocked');
  return {connected:true,available,unlocked,
    keyId:typeof data.keyId==='string'&&data.keyId.length<=120?data.keyId:'',
    label:typeof data.label==='string'&&data.label.length<=80?data.label:'Quantic ID',
    version,reason:!available?'no_identity':unlocked?'unlocked':'locked_or_unconfirmed'};
}
async function getStatus(options){
  try{return normalizeStatus(await readJson('/v1/status','GET',null,options));}
  catch{return{connected:false,available:false,unlocked:false,reason:'companion_offline'};}
}
async function assertChallenge(challenge,{transport=http.request}={}){
  if(typeof challenge!=='string'||!/^quantic-social:v1:[A-Za-z0-9_-]{32,80}$/.test(challenge))throw Error('invalid_challenge');
  const status=await getStatus({transport});
  if(!status.unlocked)throw Error('quantic_identity_locked');
  const assertion=await readJson('/v1/assert','POST',{
    challenge,relyingParty:'quanticminds.onrender.com',purpose:'social-login',
    origin:'https://xdsawyerlol.github.io'
  },{transport,timeout:3500});
  if(!assertion||typeof assertion.signature!=='string'||!assertion.signature||
    typeof assertion.keyId!=='string'||!assertion.keyId||
    !['Ed25519','ed25519'].includes(assertion.algorithm||'Ed25519'))throw Error('invalid_quantic_assertion');
  const key=assertion.publicKey||assertion.signingPublicKey||null;
  if(key && (typeof key!=='object'||key.kty!=='OKP'||key.crv!=='Ed25519'))throw Error('invalid_quantic_public_key');
  return {keyId:assertion.keyId,signature:assertion.signature,algorithm:'Ed25519',publicKey:key};
}
module.exports={getStatus,normalizeStatus,assertChallenge,readJson};
