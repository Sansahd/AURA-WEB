'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const {EventEmitter}=require('node:events');
const {authenticateQuanticSocial,isSocialPage,TOKEN_KEY}=require('../src/services/quantic-social-sso.cjs');
function localBridge(){
 const reqs=[];
 const transport=(options,callback)=>{
  reqs.push(options);
  const req=new EventEmitter();
  req.write=()=>{};
  req.end=()=>queueMicrotask(()=>{
   const res=new EventEmitter();
   res.statusCode=200;
   res.resume=()=>{};
   callback(res);
   queueMicrotask(()=>{
    res.emit('data',Buffer.from(JSON.stringify(options.path==='/v1/status'
      ? {version:3,identityAvailable:true,unlocked:true,keyId:'qid_verified'}
      : {keyId:'qid_verified',signature:'sig',algorithm:'Ed25519',
         publicKey:{kty:'OKP',crv:'Ed25519',x:'x'.repeat(43)}})));
    res.emit('end');
   });
  });
  req.destroy=e=>req.emit('error',e);
  return req;
 };
 return {transport,reqs};
}
function mockedFetcher({accepted=true,userId='uid1'}={}){
 const called=[];
 const fetcher=async(url)=>{
  called.push(url);
  const base={ok:true, json:async()=>({})};
  if(url.endsWith('/auth/quantic/challenge'))
   return {...base,json:async()=>({challenge:'quantic-social:v1:'+'a'.repeat(43)})};
  if(url.endsWith('/auth/quantic/complete'))
   return {...base,json:async()=>({token:'t'.repeat(43),user:{id:'uid1'},expiresInMs:12000})};
  if(url.endsWith('/api/pulse/me'))
   return {...base,ok:accepted,json:async()=>({user:{id:userId}})};
  throw Error('unexpected endpoint '+url);
 };
 return{fetcher,called};
}
test('SOCIAL login requires a real authenticated /me response, not a recognized identity',async()=>{
 const b=localBridge(),api=mockedFetcher();
 const success=await authenticateQuanticSocial({fetcher:api.fetcher,transport:b.transport});
 assert.equal(success.ok,true);
 assert.equal(success.user.id,'uid1');
 assert.equal(api.called.length,3);
 assert.equal(api.called[2],'https://quanticminds.onrender.com/api/pulse/me');
 assert.equal(b.reqs[0].port,47621);
 assert.equal(b.reqs[1].path,'/v1/assert');
 assert.equal(TOKEN_KEY,'quantic_pulse_token');
 assert.equal(isSocialPage('https://mediumorchid-badger-314305.hostingersite.com/pulse/'),true);
});
test('do not install a signed-in user if /me rejects the token or maps a different user',async()=>{
 let b=localBridge(),api=mockedFetcher({accepted:false});
 const no=await authenticateQuanticSocial({fetcher:api.fetcher,transport:b.transport});
 assert.equal(no.ok,false);
 assert.equal(no.error,'social_session_not_authenticated');
 b=localBridge();api=mockedFetcher({userId:'wrong'});
 const mismatch=await authenticateQuanticSocial({fetcher:api.fetcher,transport:b.transport});
 assert.equal(mismatch.ok,false);
 assert.equal(mismatch.error,'social_session_user_mismatch');
});
test('Netflix M7701-1003 is not misrepresented as a functioning unlicensed mini-player',()=>{
 const main=require('node:fs').readFileSync(require('node:path').join(__dirname,'../src/main.cjs'),'utf8');
 assert.match(main,/appId==='netflix'/);
 assert.match(main,/shell\.openExternal\('https:\/\/www\.netflix\.com\/'\)/);
});
