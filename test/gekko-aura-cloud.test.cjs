'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {QuanticAuraClient,cleanBaseUrl,DEFAULT_AURA_CLOUD_URL}=require('../src/services/aura-client.cjs');

function reply(body,{status=200,cookie=''}={}){
 return {ok:status>=200&&status<300,status,
   headers:{get:(name)=>name==='set-cookie'?cookie:null},
   text:async()=>JSON.stringify(body)};
}
test('GEKKO uses the real AURA Hostinger public /api/chat without local AURA or Ollama',async()=>{
 const calls=[];
 const client=new QuanticAuraClient({fetchImpl:async(url,opt)=>{
  calls.push({url,opt});return reply({answer:'Bonjour, je suis AURA.'},{
    cookie:'aura_chat_session=abc.DEF.ghi; HttpOnly; Path=/; Secure; SameSite=Lax'});
 }});
 const first=await client.generate({prompt:'Salut'});
 assert.equal(first.answer,'Bonjour, je suis AURA.');
 assert.equal(first.engine,'aura-cloud');
 assert.equal(calls.length,1);
 assert.equal(calls[0].url,DEFAULT_AURA_CLOUD_URL+'/api/chat');
 assert.deepEqual(JSON.parse(calls[0].opt.body),{text:'Salut',author:'Utilisateur GEKKO'});
 assert.equal(calls[0].opt.headers.cookie,undefined);
 await client.generate({prompt:'Tu te souviens de moi ?'});
 assert.equal(calls[1].opt.headers.cookie,'aura_chat_session=abc.DEF.ghi');
 assert.equal(calls[1].url,DEFAULT_AURA_CLOUD_URL+'/api/chat');
});
test('cloud errors can fall back to optional local AURA, without trusting remote config for local engines',async()=>{
 const urls=[];
 const client=new QuanticAuraClient({cloudTimeoutMs:1000,localTimeoutMs:1000,
  fetchImpl:async url=>{
   urls.push(url);
   if(url.startsWith(DEFAULT_AURA_CLOUD_URL))throw new TypeError('fetch failed');
   return reply({answer:'Modèle local disponible',engine:'aura-local'});
  }});
 const res=await client.generate({prompt:'Bonjour'});
 assert.equal(res.answer,'Modèle local disponible');
 assert.equal(res.engine,'aura-local');
 assert.equal(urls.length,2);
 assert.equal(urls[1],'http://127.0.0.1:8787/api/ai/generate');
});
test('Ollama is optional last fallback, with intelligible diagnostics if all three endpoints fail',async()=>{
 const urls=[];
 const client=new QuanticAuraClient({cloudTimeoutMs:1000,localTimeoutMs:1000,
  fetchImpl:async url=>{urls.push(url);throw Error('fetch failed')}});
 await assert.rejects(client.generate({prompt:'Hello'}),
   error=>error.message.includes('AURA Cloud indisponible') &&
     error.message.includes('Ollama facultatif') &&
     error.message.includes('serveur injoignable'));
 assert.equal(urls.length,3);
 assert.deepEqual(urls,[
  DEFAULT_AURA_CLOUD_URL+'/api/chat',
  'http://127.0.0.1:8787/api/ai/generate',
  'http://127.0.0.1:11434/api/generate']);
});
test('local fallback may be disabled and remote endpoints require HTTPS',async()=>{
 const client=new QuanticAuraClient({allowLocalFallback:false,
  fetchImpl:async()=>{throw Error('fetch failed')}});
 await assert.rejects(client.generate({prompt:'Salut'}),/AURA locale désactivée/);
 assert.throws(()=>cleanBaseUrl('https://malicious.example','http://127.0.0.1:8787'),/boucle locale/);
 assert.throws(()=>cleanBaseUrl('http://untrusted.example','https://aura.example',{cloud:true}),/HTTPS/);
 assert.equal(cleanBaseUrl(DEFAULT_AURA_CLOUD_URL,'',{cloud:true}),DEFAULT_AURA_CLOUD_URL);
});
test('AURA Cloud route is visible in runtime label, and browser context requires explicit request',()=>{
 const fs=require('node:fs'),path=require('node:path');
 const main=fs.readFileSync(path.join(__dirname,'..','src','main.cjs'),'utf8');
 assert.match(main,/'aura-cloud': 'AURA · Cloud autonome'/);
 assert.match(main,/const refersToPage =/);
 assert.match(main,/text && refersToPage/);
 assert.doesNotMatch(main,/AURA 2\.0 est indisponible sur ce PC/);
});
