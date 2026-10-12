'use strict';
// Non-blocking external smoke: distinguish an unavailable deployed service
// from a GEKKO client integration problem. Never send page content or tokens.
const {DEFAULT_AURA_CLOUD_URL}=require('../src/services/aura-client.cjs');
(async()=>{
 const controller=new AbortController(),timeout=setTimeout(()=>controller.abort(),10000);
 try{
  const response=await fetch(DEFAULT_AURA_CLOUD_URL+'/healthz',{
   method:'GET',redirect:'error',headers:{accept:'application/json'},
   signal:controller.signal
  });
  const body=await response.text();
  const status=(()=>{
   try{const j=JSON.parse(body);return {ready:j.ready??null,status:j.status??null,db:j.db??null}}catch{return {}}
  })();
  console.log('[aura-cloud-health]',JSON.stringify({origin:DEFAULT_AURA_CLOUD_URL,http:response.status,...status}));
  if(!response.ok||status.ready===false)
   console.warn('[aura-cloud-health] Hostinger is not ready; GEKKO cannot repair the remote deployment.');
 }catch(error){
  console.warn('[aura-cloud-health]',String(error?.message||error).slice(0,140));
 }finally{clearTimeout(timeout)}
})();
