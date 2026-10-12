'use strict';
// Copyright-aware image results. Openverse provides legitimately catalogued
// Creative Commons / public-domain works and offers its own thumbnail proxy.
// Never contact arbitrary third-party image origins from search metadata.
const {readLimited,pngOf,clean,searchCommonsImages}=require('./gekko-search-25.cjs');
function isUuid(value){return /^[\da-f]{8}-[\da-f]{4}-[1-8][\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/i.test(String(value||''))}
function safeLanding(raw){
 try {
  const u=new URL(String(raw||''));
  if(u.protocol!=='https:'||u.username||u.password||
   /^(localhost|127\.|10\.|192\.168\.|169\.254\.|172\.1[6-9]\.|172\.2\d\.|172\.3[01]\.|0\.)/i.test(u.hostname) ||
   u.hostname.endsWith('.local')||u.hostname.endsWith('.internal'))return '';
  return u.href;
 } catch{return ''}
}
async function searchOpenverseImages(query,opts,{limit=18}={}){
 const q=clean(query,150);
 if(!q||!opts?.nativeImage||typeof opts?.fetch!=='function')
  return {status:'unavailable',results:[],source:'Openverse'};
 const u=new URL('https://api.openverse.org/v1/images/');
 u.searchParams.set('q',q);
 u.searchParams.set('page_size',String(Math.max(1,Math.min(18,Math.round(limit)||18))));
 try {
  const raw=JSON.parse((await readLimited(u.href,opts.fetch)).toString('utf8'));
  const items=Array.isArray(raw.results)?raw.results.filter(x=>!x.mature&&isUuid(x.id)).slice(0,Math.max(1,Math.min(18,Math.round(limit)||18))):[];
  let cursor=0;const output=new Array(items.length);
  await Promise.all(Array.from({length:Math.min(4,items.length)},async()=>{
   while(cursor<items.length){
    const i=cursor++,record=items[i];
    // The fixed /thumb/ endpoint avoids URL-based thumbnail SSRF.
    const thumb='https://api.openverse.org/v1/images/'+record.id+'/thumb/';
    const image=await pngOf(thumb,{...opts,allowedHosts:['api.openverse.org']},360);
    const url=safeLanding(record.foreign_landing_url)||safeLanding(record.detail_url);
    if(image && url)output[i]={
     title:clean(record.title||'Illustration libre',120),image,url,
     creator:clean(record.creator||'',85),
     license:clean((record.license||'').toUpperCase()+' '+(record.license_version||''),30),
     source:'Openverse'
    };
   }
  }));
  const results=output.filter(Boolean);
  return {status:results.length?'ok':'unavailable',results,source:'Openverse (licences ouvertes)'};
 }catch{return {status:'unavailable',results:[],source:'Openverse'}}
}
async function searchImageSources(query,opts){
 const commons=await searchCommonsImages(query,opts);
 // When Wikimedia is sparse, append openly licensed Openverse results.
 if(commons.results.length>=10)return commons;
 const other=await searchOpenverseImages(query,opts);
 const seen=new Set(commons.results.map(x=>x.url));
 const results=[...commons.results];
 for(const row of other.results){if(results.length>=18)break;if(seen.has(row.url))continue;seen.add(row.url);results.push(row)}
 return {status:results.length?'ok':'unavailable',results,
  source:other.results.length?'Wikimedia Commons + Openverse':commons.source};
}
module.exports={isUuid,safeLanding,searchOpenverseImages,searchImageSources};
