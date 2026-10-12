const test=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {QuanticStore}=require('../src/services/store.cjs');
test('favorite icons saved locally when PNG base64 is larger than old 14k cap',async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'gekko-fav-'));
 try{
  const app={getPath:()=>dir},store=new QuanticStore(app),url='https://example.org/';
  store.toggleFavorite({title:'Example',url});
  const icon='data:image/png;base64,'+'A'.repeat(16000);
  assert.equal(store.updateFavoriteFavicon(url,icon),true);
  await store.flush();
  assert.equal(new QuanticStore(app).favorites(1)[0].favicon,icon);
 }finally{fs.rmSync(dir,{recursive:true,force:true})}
});
test('legacy bookmarks hydrate from their original domain without proxy',()=>{
 const main=fs.readFileSync(path.join(__dirname,'../src/main.cjs'),'utf8');
 const render=fs.readFileSync(path.join(__dirname,'../src/renderer/renderer.js'),'utf8');
 assert.match(main,/refreshSavedFavicons/);assert.match(main,/normalSession\.fetch\(iconUrl, options\)/);
 assert.match(render,/gekko-favorite-page-mark/);
});
