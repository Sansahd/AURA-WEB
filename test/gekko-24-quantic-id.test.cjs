'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {PassThrough}=require('node:stream');
const {EventEmitter}=require('node:events');
const {CHANNEL_VALIDATORS}=require('../src/security/ipc-firewall.cjs');
const {normalizeStatus,getStatus,assertChallenge}=require('../src/services/quantic-id-bridge.cjs');
const {isSocialPage,installSocialSession,revokeSocialSession}=require('../src/services/quantic-social-sso.cjs');

function mockBridge(responses){
  const seen=[];
  const transport=(opts,callback)=>{
    seen.push(opts);
    const req=new EventEmitter();
    let body='';
    req.write=data=>{body+=data};
    req.end=()=>{queueMicrotask(()=>{
      const entry=responses.shift()||{};
      if(entry.error){req.emit('error',new Error(entry.error));return}
      const res=new PassThrough();
      res.statusCode=entry.status||200;
      res.resume=()=>{};
      callback(res);
      res.end(JSON.stringify(entry.data||{}));
    })};
    req.destroy=error=>req.emit('error',error);
    req.received=()=>body;
    return req;
  };
  return{transport,seen};
}

test('real Mail and SOCIAL sidebar buttons are accepted by shell firewall, unknown apps denied',()=>{
 const check=CHANNEL_VALIDATORS['set-setting'];
 assert.equal(check(['sideStageAction','toggle:quanticmail']),true);
 assert.equal(check(['sideStageAction','toggle:quanticpulse']),true);
 assert.equal(check(['sideStageAction','select:quanticmail']),true);
 assert.equal(check(['sideStageAction','reload:quanticpulse']),true);
 assert.equal(check(['sideStageAction','toggle:unknown']),false);
 assert.equal(check(['sideStageAction','toggle:quanticmail:extra']),false);
});

test('Quantic Secure status is compatible with v1 and v3 but never assumes mere USB presence means unlocked',async()=>{
 assert.equal(normalizeStatus({version:3,identityAvailable:true,unlocked:true,keyId:'qid_1'}).unlocked,true);
 assert.equal(normalizeStatus({version:3,identityAvailable:true,locked:false}).unlocked,true);
 assert.equal(normalizeStatus({version:1,identityAvailable:true}).unlocked,false);
 assert.equal(normalizeStatus({version:3,identityAvailable:true,locked:true}).unlocked,false);
 assert.equal(normalizeStatus({version:6,identityAvailable:true,unlocked:true}).unlocked,false);
 const bridge=mockBridge([{data:{version:3,identityAvailable:true,unlocked:true,keyId:'qid_123'}}]);
 const result=await getStatus({transport:bridge.transport});
 assert.equal(result.unlocked,true);
 assert.equal(bridge.seen[0].hostname,'127.0.0.1');
 assert.equal(bridge.seen[0].port,47621);
 assert.equal(bridge.seen[0].path,'/v1/status');
 const offline=await getStatus({transport:mockBridge([{error:'ECONNREFUSED'}]).transport});
 assert.equal(offline.connected,false);
});

test('local assertion is only requested after unlocked status, with precise SOCIAL audience',async()=>{
 const challenge='quantic-social:v1:'+'a'.repeat(43);
 const publicKey={kty:'OKP',crv:'Ed25519',x:'x'.repeat(43)};
 const bridge=mockBridge([
  {data:{version:3,identityAvailable:true,unlocked:true,keyId:'qid_good'}},
  {data:{keyId:'qid_good',signature:'signed',algorithm:'Ed25519',publicKey}}
 ]);
 const signed=await assertChallenge(challenge,{transport:bridge.transport});
 assert.equal(signed.keyId,'qid_good');
 assert.equal(signed.publicKey.crv,'Ed25519');
 assert.equal(bridge.seen[1].path,'/v1/assert');
 assert.equal(bridge.seen[1].method,'POST');
 const locked=mockBridge([{data:{version:3,identityAvailable:true,locked:true}}]);
 await assert.rejects(assertChallenge(challenge,{transport:locked.transport}),/locked/);
 assert.equal(locked.seen.length,1,'must not sign a challenge while locked');
});

test('Quantic ID session is only injected into the exact official SOCIAL page',async()=>{
 assert.equal(isSocialPage('https://xdsawyerlol.github.io/QuanticSillage/zoon.html'),true);
 assert.equal(isSocialPage('https://xdsawyerlol.github.io/QuanticSillage/zoon.html.evil'),false);
 assert.equal(isSocialPage('https://phishing.example/QuanticSillage/zoon.html'),false);
 const scripts=[];
 const wc={
  isDestroyed:()=>false,
  getURL:()=> 'https://xdsawyerlol.github.io/QuanticSillage/zoon.html',
  executeJavaScript:async code=>{scripts.push(code);return true}
 };
 const result=await installSocialSession(wc,{ok:true,token:'t'.repeat(42),user:{id:'user1'}});
 assert.equal(result,true);
 assert.equal(scripts.length,1);
 assert.ok(scripts[0].includes('quantic-gekkko-session'));
 assert.ok(scripts[0].includes("location.origin !== 'https://xdsawyerlol.github.io'"));
 assert.equal(await revokeSocialSession(wc,'t'.repeat(42)),true);
 const evil={...wc,getURL:()=> 'https://phishing.example/QuanticSillage/zoon.html'};
 assert.equal(await installSocialSession(evil,{ok:true,token:'t'.repeat(42),user:{id:'user1'}}),false);
 assert.equal(scripts.length,2);
});
