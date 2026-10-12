'use strict';
const {fetchSmallIcon}=require('./favicon-cache.cjs');
const {safeMediaUrl,pngOf,readLimited}=require('./gekko-search-25.cjs');

const META_HOSTS=new Set(['imdb.com','www.imdb.com','allocine.fr','www.allocine.fr','marvel.com','www.marvel.com','fr.wikipedia.org','en.wikipedia.org']);
const IMAGE_HOSTS=['m.media-amazon.com','upload.wikimedia.org','i.ytimg.com','images.allocine.fr','fr.web.img6.acsta.net','fr.web.img5.acsta.net','media.marvel.com','cdn.marvelrivals.com'];
function trustedOrigin(raw){
 try{const u=new URL(raw);return ['https:','http:'].includes(u.protocol)&&!u.username&&!u.password &&
  !/^(localhost|127\.|10\.|192\.168\.|169\.254\.|0\.|172\.1[6-9]\.|172\.2\d\.|172\.3[01]\.)/i.test(u.hostname)&&
  !u.hostname.endsWith('.local')&&!u.hostname.endsWith('.internal') ? u:null;
 }catch{return null}
}
function youtubeThumb(raw){
 const u=trustedOrigin(raw);if(!u||u.protocol!=='https:')return '';
 if(!['youtube.com','www.youtube.com','m.youtube.com','youtu.be'].includes(u.hostname))return '';
 const id=u.hostname==='youtu.be'?u.pathname.split('/')[1]:
  u.pathname.startsWith('/shorts/')?u.pathname.split('/')[2]:u.searchParams.get('v');
 return /^[a-zA-Z0-9_-]{11}$/.test(id||'')?'https://i.ytimg.com/vi/'+id+'/mqdefault.jpg':'';
}
function metaImageHtml(html){
 const source=String(html||'');
 for(const m of source.matchAll(/<meta\b[^>]*>/gi)){
  const tag=m[0],property=tag.match(/\b(?:property|name)\s*=\s*["']([^"']+)["']/i)?.[1]?.toLowerCase();
  if(!['og:image','twitter:image'].includes(property))continue;
  const url=tag.match(/\bcontent\s*=\s*["']([^"']+)["']/i)?.[1];
  if(url)return url.replace(/&amp;/g,'&');
 }
 return '';
}
async function thumbnailFor(result,opts){
 const page=trustedOrigin(result?.url);if(!page||page.protocol!=='https:')return '';
 try{
  const youtube=youtubeThumb(page.href);
  if(youtube)return await pngOf(youtube,{...opts,allowedHosts:IMAGE_HOSTS},320);
  if(!META_HOSTS.has(page.hostname))return '';
  const html=(await readLimited(page.href,opts.fetch,'text/html')).toString('utf8');
  const image=metaImageHtml(html);
  const approved=safeMediaUrl(image,IMAGE_HOSTS);
  if(!approved)return '';
  return await pngOf(approved,{...opts,allowedHosts:IMAGE_HOSTS},330);
 }catch{return ''}
}
async function iconFor(result,opts){
 const page=trustedOrigin(result?.url);if(!page||page.protocol!=='https:')return '';
 return fetchSmallIcon({pageUrl:page.origin+'/',candidates:[page.origin+'/favicon.ico',page.origin+'/apple-touch-icon.png'],
  fetch:opts.fetch,nativeImage:opts.nativeImage,signal:AbortSignal.timeout(3000)}).catch(()=>'');
}
async function enrichSearchResults(results,opts){
 const list=Array.isArray(results)?results.slice(0,12):[];
 const settled=[];
 const concurrency=3;
 let cursor=0;
 await Promise.all(Array.from({length:Math.min(concurrency,list.length)},async()=>{
  while(cursor<list.length){
   const i=cursor++;const src=list[i];
   if(!trustedOrigin(src?.url)){settled[i]={...src};continue;}
   const [icon,thumbnail]=await Promise.all([
    iconFor(src,opts).catch(()=>''),i<8?thumbnailFor(src,opts).catch(()=>''):Promise.resolve('')
   ]);
   settled[i]={...src,icon,thumbnail};
  }
 }));
 return settled;
}
module.exports={trustedOrigin,youtubeThumb,metaImageHtml,iconFor,thumbnailFor,enrichSearchResults};
