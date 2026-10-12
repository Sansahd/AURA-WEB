'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path');
const {youtubeThumb,metaImageHtml,thumbnailFor,enrichSearchResults}=require('../src/services/gekko-search-rich.cjs');
const {searchOpenverseImages,searchImageSources,isUuid}=require('../src/services/gekko-search-images.cjs');
const {searchKnowledge,wikidataFacts,searchCommonsImages}=require('../src/services/gekko-search-25.cjs');
function json(data){return new Response(JSON.stringify(data),{status:200,headers:{'content-type':'application/json'}})}
const nativeImage={createFromBuffer:()=>({isEmpty:()=>false,getSize:()=>({width:240,height:150}),resize:()=>({toPNG:()=>Buffer.from([137,80,78,71,1,2,3,4])})})};
test('media previews cannot load arbitrary websites or untrusted remote thumbnails',async()=>{
 assert.equal(youtubeThumb('https://www.youtube.com/watch?v=dQw4w9WgXcQ'),'https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg');
 assert.equal(youtubeThumb('https://example.net/watch?v=dQw4w9WgXcQ'),'');
 assert.equal(metaImageHtml('<meta property="og:image" content="https://m.media-amazon.com/poster.jpg">'),'https://m.media-amazon.com/poster.jpg');
 assert.equal(isUuid('4bc43a04-ef46-4544-a0c1-63c63f56e276'),true);
 assert.equal(isUuid('../etc/passwd'),false);
 const preview=await thumbnailFor({url:'https://www.youtube.com/watch?v=dQw4w9WgXcQ'},{
  nativeImage,fetch:async url=>{
   assert.match(url,/^https:\/\/i\.ytimg\.com\/vi\//);
   return new Response(Buffer.from([137,80,78,71]),{status:200,headers:{'content-type':'image/jpeg'}});
  }});
 assert.match(preview,/^data:image\/png;base64,/);
});
test('search results acquire actual icons and thumbnails; source fallback is safe',async()=>{
 const results=await enrichSearchResults([{url:'https://www.youtube.com/watch?v=dQw4w9WgXcQ',title:'Trailer'}],{
  nativeImage,fetch:async url=>new Response(Buffer.from([137,80,78,71]),{
   status:200,headers:{'content-type':url.endsWith('.ico')?'image/x-icon':'image/jpeg'}
  })
 });
 assert.equal(results.length,1);
 assert.match(results[0].icon,/^data:image\/png;base64,/);
 assert.match(results[0].thumbnail,/^data:image\/png;base64,/);
});
test('Openverse thumbnail proxy yields attributed, openly licensed results',async()=>{
 const id='4bc43a04-ef46-4544-a0c1-63c63f56e276',seen=[];
 const fetcher=async url=>{
  seen.push(url);
  if(url.includes('/v1/images/?'))return json({results:[{
   id,title:'Illustration originale',creator:'Artiste',license:'cc0',license_version:'1.0',
   foreign_landing_url:'https://commons.wikimedia.org/wiki/File:Example.jpg',mature:false
  }]});
  assert.equal(url,'https://api.openverse.org/v1/images/'+id+'/thumb/');
  return new Response(Buffer.from('png'),{status:200,headers:{'content-type':'image/png'}});
 };
 const out=await searchOpenverseImages('spiderman',{fetch:fetcher,nativeImage});
 assert.equal(out.status,'ok');assert.equal(out.results[0].license,'CC0 1.0');
 assert.equal(out.results[0].creator,'Artiste');
 assert.match(out.results[0].image,/^data:image\/png;base64,/);
 assert.equal(seen.length,2);
});
test('Wikidata facts show only returned factual claims, never invented film metadata',async()=>{
 const fetcher=async url=>{
  if(url.includes('ids=Q7903'))return json({entities:{Q7903:{claims:{
   P170:[{mainsnak:{snaktype:'value',datavalue:{value:{id:'Q123'}}}}],
   P31:[{mainsnak:{snaktype:'value',datavalue:{value:{id:'Q456'}}}}]
  }}}});
  if(url.includes('ids=Q123'))return json({entities:{Q123:{labels:{fr:{value:'Créateur public'}}},Q456:{labels:{fr:{value:'Personnage fictif'}}}}});
  return json({entities:{}});
 };
 const facts=await wikidataFacts('Q7903',{fetch:fetcher});
 assert.ok(facts.some(x=>x.label==='Créateurs'&&x.value==='Créateur public'));
 assert.ok(facts.some(x=>x.label==='Nature'&&x.value==='Personnage fictif'));
});
test('UI includes no fake AI-generated answer and source thumbnail components',()=>{
 const js=fs.readFileSync(path.join(__dirname,'../src/renderer/renderer.js'),'utf8');
 const css=fs.readFileSync(path.join(__dirname,'../src/renderer/gekko-search-25.css'),'utf8');
 const main=fs.readFileSync(path.join(__dirname,'../src/main.cjs'),'utf8');
 assert.match(js,/gk25-feature-media/);
 assert.match(js,/gk25-facts/);
 assert.match(js,/gk25-result-thumb/);
 assert.match(js,/gk25-picture-license/);
 assert.match(main,/enrichSearchResults/);
 assert.match(main,/searchImageSources/);
 assert.match(css,/gk25-link:hover/);
 assert.doesNotMatch(js,/Réponse générée par GEKKO/);
});
