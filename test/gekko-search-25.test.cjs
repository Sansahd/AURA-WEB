'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {searchKnowledge,searchCommonsImages,safeMediaUrl,related}=require('../src/services/gekko-search-25.cjs');
const {searchPublicWeb}=require('../src/services/gekko-search.cjs');
const root=path.join(__dirname,'..');
function response(body,type='application/json'){
 return new Response(body,{status:200,headers:{'content-type':type}});
}
test('GEKKO 2.5 encyclopedia data is sourced, bounded and optional',async()=>{
 const fetcher=async url=>{
  assert.match(url,/fr\.wikipedia\.org\/w\/api\.php/);
  return response(JSON.stringify({query:{pages:[{title:'Spider-Man',extract:'Personnage fictif créé pour les comics.'}]}}));
 };
 const knowledge=await searchKnowledge('spiderman',{fetch:fetcher});
 assert.equal(knowledge.title,'Spider-Man');
 assert.equal(knowledge.source,'Wikipédia (FR)');
 assert.match(knowledge.url,/fr\.wikipedia\.org/);
 assert.match(knowledge.summary,/Personnage fictif/);
 assert.equal(knowledge.image,'');
 assert.equal(safeMediaUrl('http://evil.com/a.png'),'');
 assert.equal(safeMediaUrl('https://upload.wikimedia.org/x.png'),'https://upload.wikimedia.org/x.png');
});
test('absence of real images never invents picture URLs',async()=>{
 const fetcher=async()=>response(JSON.stringify({query:{pages:[]}}));
 const out=await searchCommonsImages('spiderman',{fetch:fetcher});
 assert.deepEqual(out.results,[]);
 assert.equal(out.status,'unavailable');
});
test('video and news filters alter actual provider request URLs',async()=>{
 const seen=[];
 const fetcher=async url=>{seen.push(url);return response('<rss><channel><item><title>Video</title><link>https://example.org/vid</link><description>Extrait</description></item></channel></rss>','application/rss+xml')};
 const v=await searchPublicWeb('spiderman',{fetch:fetcher,mode:'videos',fresh:'week',region:'fr'});
 assert.equal(v.status,'ok');
 assert.match(decodeURIComponent(seen[0]),/site:youtube.com/);
 assert.match(decodeURIComponent(seen[0]),/after:/);
 assert.match(seen[0],/cc=FR/);
 seen.length=0;
 const n=await searchPublicWeb('spiderman',{fetch:fetcher,mode:'news',region:'world'});
 assert.equal(n.status,'ok');
 assert.match(seen[0],/bing.com\/news/);
 assert.doesNotMatch(seen[0],/cc=FR/);
});
test('GEKKO 2.5 UI is integrated and accessible without loading remote HTML',()=>{
 const ui=fs.readFileSync(path.join(root,'src/renderer/renderer.js'),'utf8');
 const page=fs.readFileSync(path.join(root,'src/renderer/index.html'),'utf8');
 const main=fs.readFileSync(path.join(root,'src/main.cjs'),'utf8');
 const css=fs.readFileSync(path.join(root,'src/renderer/gekko-search-25.css'),'utf8');
 for(const term of ['gk25-header','gk25-form','gk25-result','gk25-side','gk25-tab','gk25-assist']){
   assert.match(ui,new RegExp(term));
 }
 assert.match(ui,/toggleAi/);
 assert.match(ui,/aria-current/);
 assert.match(main,/searchKnowledge\(query,opts\)/);
 assert.match(main,/searchCommonsImages\(query,opts\)/);
 assert.match(page,/gekko-search-25\.css/);
 assert.match(css,/@media\(max-width:700px\)/);
 assert.doesNotMatch(ui,/innerHTML\s*=/);
 assert.deepEqual(related('spiderman').map(x=>x.label).slice(0,2),['Définition','Histoire']);
});
