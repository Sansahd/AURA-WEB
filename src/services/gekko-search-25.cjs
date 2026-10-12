'use strict';
const LIMIT=900000;
function safeMediaUrl(input,hosts=['upload.wikimedia.org']){
 try{const u=new URL(String(input||''));return u.protocol==='https:'&&!u.username&&!u.password&&hosts.some(h=>u.hostname===h||u.hostname.endsWith('.'+h))?u.href:''}catch{return ''}
}
async function readLimited(url,fetcher,accept='application/json'){
 const r=await fetcher(url,{redirect:'error',headers:{accept},signal:AbortSignal.timeout(7500)});
 if(!r?.ok||Number(r.headers?.get?.('content-length')||0)>LIMIT||!r.body?.getReader)throw Error('upstream-unavailable');
 const chunks=[];let size=0,reader=r.body.getReader();
 try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>LIMIT)throw Error('upstream-too-large');chunks.push(Buffer.from(value))}}
 finally{try{reader.releaseLock()}catch{}}
 return Buffer.concat(chunks,size);
}
function clean(s,n=800){return String(s||'').replace(/[\u0000-\u001f\u007f]/g,' ').replace(/\s+/g,' ').trim().slice(0,n)}
function pages(j){return Object.values(j?.query?.pages||{}).filter(p=>p&&typeof p==='object'&&!p.missing)}
async function pngOf(url,{fetch:fetcher,nativeImage,allowedHosts=['upload.wikimedia.org']},max=440){
 const remote=safeMediaUrl(url,allowedHosts);if(!remote||!nativeImage?.createFromBuffer)return '';
 try{
  const bytes=await readLimited(remote,fetcher,'image/png,image/jpeg,image/webp');
  const img=nativeImage.createFromBuffer(bytes);if(img.isEmpty())return '';
  const d=img.getSize();const k=Math.min(1,max/d.width,max/d.height);
  const png=img.resize({width:Math.max(1,Math.round(d.width*k)),height:Math.max(1,Math.round(d.height*k)),quality:'best'}).toPNG();
  return png.length&&png.length<280000?'data:image/png;base64,'+png.toString('base64'):'';
 }catch{return ''}
}
async function wikidataFacts(itemId,opts){
 const id=String(itemId||'');
 if(!/^Q[1-9]\d{0,11}$/.test(id)||typeof opts?.fetch!=='function')return [];
 try{
  const u=new URL('https://www.wikidata.org/w/api.php');
  for(const [k,v] of Object.entries({action:'wbgetentities',ids:id,props:'claims',languages:'fr|en',format:'json'}))u.searchParams.set(k,v);
  const data=JSON.parse((await readLimited(u.href,opts.fetch)).toString('utf8'));
  const claims=data.entities?.[id]?.claims||{};
  const refs=new Set(),records=[];
  for(const [prop,label] of [['P170','Créateurs'],['P50','Auteurs'],['P571','Création'],['P577','Publication'],['P136','Genre'],['P31','Nature']]){
   const values=[];
   for(const claim of (claims[prop]||[]).slice(0,3)){
    if(claim?.mainsnak?.snaktype!=='value')continue;
    const v=claim.mainsnak.datavalue?.value;
    if(v?.id && /^Q\d+$/.test(v.id)){refs.add(v.id);values.push({id:v.id})}
    else if(v?.time)values.push({value:v.time.replace(/^\+/,'').slice(0,4)});
   }
   if(values.length)records.push({label,values});
  }
  let labels={};
  if(refs.size){
   const next=new URL('https://www.wikidata.org/w/api.php');
   for(const [k,v] of Object.entries({action:'wbgetentities',ids:[...refs].slice(0,12).join('|'),props:'labels',languages:'fr|en',format:'json'}))next.searchParams.set(k,v);
   try{labels=JSON.parse((await readLimited(next.href,opts.fetch)).toString('utf8')).entities||{}}catch{}
  }
  return records.map(r=>({label:r.label,value:r.values.map(v=>v.id?
    clean(labels[v.id]?.labels?.fr?.value||labels[v.id]?.labels?.en?.value||'',90):v.value).filter(Boolean).join(' · ')})).filter(r=>r.value).slice(0,5);
 }catch{return []}
}
async function searchKnowledge(query,opts){
 if(!clean(query)||typeof opts?.fetch!=='function')return null;
 const u=new URL('https://fr.wikipedia.org/w/api.php');
 for(const [k,v] of Object.entries({action:'query',generator:'search',gsrsearch:clean(query,150),gsrlimit:'1',prop:'pageimages|extracts|pageprops',exintro:'1',explaintext:'1',exchars:'900',pithumbsize:'480',format:'json',formatversion:'2'}))u.searchParams.set(k,v);
 try{
  const page=pages(JSON.parse((await readLimited(u.href,opts.fetch)).toString('utf8')))[0];
  if(!page?.title||!page.extract)return null;
  const record={title:clean(page.title,150),summary:clean(page.extract,950),source:'Wikipédia (FR)',
   url:'https://fr.wikipedia.org/wiki/'+encodeURIComponent(String(page.title).replace(/\s/g,'_')),
   image:'',imageSource:'',facts:[],media:[]};
  if(page.thumbnail?.source){
   record.image=await pngOf(page.thumbnail.source,opts,520);
   if(record.image)record.imageSource='Wikipédia';
  }
  const factsPromise=wikidataFacts(page.pageprops?.wikibase_item,opts);
  if(!record.image){
   const commons=await searchCommonsImages(page.title,opts);
   record.media=commons.results.slice(0,3);
   if(record.media[0]){
    record.image=record.media[0].image;
    record.imageSource='Wikimedia Commons';
   }
  }
  record.facts=await factsPromise;
  return record;
 }catch{return null}
}
async function searchCommonsImages(query,opts){
 if(!clean(query)||typeof opts?.fetch!=='function')return {status:'unavailable',results:[],source:'Wikimedia Commons'};
 const u=new URL('https://commons.wikimedia.org/w/api.php');
 for(const [k,v] of Object.entries({action:'query',generator:'search',gsrnamespace:'6',gsrsearch:clean(query,150),gsrlimit:'12',prop:'imageinfo',iiprop:'url',iiurlwidth:'440',format:'json',formatversion:'2'}))u.searchParams.set(k,v);
 try{
  const found=pages(JSON.parse((await readLimited(u.href,opts.fetch)).toString('utf8'))).filter(x=>/^File:/i.test(x.title)&&x.imageinfo?.[0]?.thumburl).slice(0,10);
  const results=(await Promise.all(found.map(async p=>{
   const media=p.imageinfo[0],url=safeMediaUrl(media.descriptionurl,['commons.wikimedia.org']),image=await pngOf(media.thumburl,opts,440);
   return url&&image?{url,image,title:clean(p.title.replace(/^File:/i,'').replace(/_/g,' '),150)}:null;
  }))).filter(Boolean);
  return {status:results.length?'ok':'unavailable',results,source:'Wikimedia Commons'};
 }catch{return {status:'unavailable',results:[],source:'Wikimedia Commons'}}
}
function related(query){
 const q=clean(query,120);
 return [['Définition',q+' définition'],['Histoire',q+' histoire'],['Images',q+' images'],['Vidéos',q+' vidéos'],['Actualités',q+' actualités'],['Chronologie',q+' chronologie']].map(([label,query])=>({label,query}));
}
module.exports={safeMediaUrl,readLimited,clean,pages,pngOf,searchKnowledge,searchCommonsImages,related,wikidataFacts};
