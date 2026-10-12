'use strict';
// Free sources, on-device result rendering, no API key, no query persistence.
const entities={amp:'&',lt:'<',gt:'>',quot:'"',apos:"'",nbsp:' '};
function decode(s){return String(s||'').replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi,(raw,k)=>{
 if(k[0]==='#'){const n=k[1].toLowerCase()==='x'?parseInt(k.slice(2),16):parseInt(k.slice(1),10);return n>0&&n<=0x10ffff?String.fromCodePoint(n):''}
 return entities[k.toLowerCase()]??raw;
});}
function plain(s){return decode(String(s||'').replace(/<[^>]*>/g,' ').replace(/\s+/g,' ')).trim().slice(0,500);}
function safeUrl(raw){
 try{let u=new URL(decode(raw));if(/(^|\.)duckduckgo\.com$/i.test(u.hostname)&&u.pathname.startsWith('/l/')){const next=u.searchParams.get('uddg');if(next)u=new URL(next)}
 if(!['https:','http:'].includes(u.protocol)||u.username||u.password||
 /^(localhost|127\.|10\.|192\.168\.|0\.)/i.test(u.hostname)||u.hostname.endsWith('.local'))return '';
 return u.href;}catch{return ''}
}
function unique(a){let seen=new Set;return a.filter(x=>{if(!x.url||seen.has(x.url))return false;seen.add(x.url);return true}).slice(0,15)}
function parseRss(xml){const a=[];for(const m of String(xml||'').matchAll(/<item(?:\s[^>]*)?>([\s\S]*?)<\/item>/gi)){
 const field=t=>m[1].match(new RegExp('<'+t+'(?:\\s[^>]*)?>([\\s\\S]*?)<\\/'+t+'>','i'))?.[1]||'';
 const url=safeUrl(plain(field('link'))),title=plain(field('title')),snippet=plain(field('description'));
 if(url&&title)a.push({url,title,snippet});if(a.length>=15)break;
}return unique(a)}
function parseDuckHtml(html){const a=[],s=String(html||'');for(const m of s.matchAll(/<a\b([^>]*class\s*=\s*["'][^"']*\bresult__a\b[^"']*["'][^>]*)>([\s\S]*?)<\/a>/gi)){
 const url=safeUrl(m[1].match(/\bhref\s*=\s*["']([^"']+)["']/i)?.[1]||''),title=plain(m[2]);
 const context=s.slice(m.index+m[0].length,m.index+m[0].length+2000);
 const snippet=plain(context.match(/class\s*=\s*["'][^"']*\bresult__snippet\b[^"']*["'][^>]*>([\s\S]*?)<\/(?:a|span|div)>/i)?.[1]||'');
 if(url&&title)a.push({url,title,snippet});if(a.length>=15)break;
}return unique(a)}
async function readBounded(url,fetcher){
 const res=await fetcher(url,{redirect:'error',signal:AbortSignal.timeout(8000)});
 if(!res?.ok||Number(res.headers?.get?.('content-length')||0)>850000||!res.body?.getReader)throw Error('source-unavailable');
 let chunks=[],total=0;const reader=res.body.getReader();
 try{while(true){const {done,value}=await reader.read();if(done)break;total+=value.byteLength;
 if(total>850000)throw Error('source-too-large');chunks.push(Buffer.from(value))}}
 finally{try{reader.releaseLock()}catch{}}
 return Buffer.concat(chunks,total).toString('utf8');
}
async function searchPublicWeb(query,{fetch:fetcher=fetch}={}){
 const q=String(query||'').trim().slice(0,220);if(!q)return {status:'unavailable',results:[],source:''};
 for(const row of [
  {url:'https://www.bing.com/search?format=rss&q='+encodeURIComponent(q),parse:parseRss,source:'Bing RSS'},
  {url:'https://html.duckduckgo.com/html/?q='+encodeURIComponent(q),parse:parseDuckHtml,source:'DuckDuckGo HTML'}]){
  try{const results=row.parse(await readBounded(row.url,fetcher));if(results.length)return {status:'ok',results,source:row.source}}catch{}
 }
 return {status:'unavailable',results:[],source:''};
}
module.exports={parseRss,parseDuckHtml,safeUrl,searchPublicWeb};
