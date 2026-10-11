'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
const {KEY_SYSTEM,PROBE,isNetflixOrigin,supportsEmbeddedWidevine}=
 require('../src/services/netflix-widevine.cjs');

test('Castlabs runtime installed and packaged, not official Electron binary',()=>{
 const pkg=JSON.parse(read('package.json'));
 assert.equal(pkg.devDependencies.electron,
   'https://github.com/castlabs/electron-releases#v44.5.1+wvcus');
 assert.equal(pkg.build.electronDist,'node_modules/electron/dist');
 const boot=read('src/bootstrap.cjs');
 const media=read('src/services/protected-media.cjs');
 assert.ok(boot.indexOf('prepareProtectedMedia()')<boot.indexOf("require('./main.cjs')"));
 assert.match(media,/components\.whenReady\(\)/);
 assert.match(media,/component_stage_ready/);
});

test('Netflix is always integrated SideStage; CDM errors stay within the same view',()=>{
 const main=read('src/main.cjs');
 const manager=read('src/services/sidestage-manager.cjs');
 assert.doesNotMatch(main,/shell\.openExternal\('https:\/\/www\.netflix\.com/);
 assert.match(main,/awaitProtectedMedia\(30000\)/);
 assert.match(main,/sideStage\?\.showProtectedMediaError/);
 assert.match(main,/appId !== 'netflix'/);
 assert.match(main,/stageState\.activeApp==='netflix'/);
 assert.match(manager,/showProtectedMediaError\(message=/);
 assert.match(manager,/supportsEmbeddedWidevine\(wc\)/);
 assert.match(manager,/wc\.on\('did-finish-load'/);
 assert.match(manager,/this\.failures\.add\('netflix'\)/);
});

test('Widevine request is a standards-based EME probe bound to official Netflix HTTPS origin',async()=>{
 assert.equal(KEY_SYSTEM,'com.widevine.alpha');
 assert.match(PROBE,/navigator\.requestMediaKeySystemAccess/);
 assert.match(PROBE,/sessionTypes:\['temporary'\]/);
 assert.equal(isNetflixOrigin('https://www.netflix.com/watch/123'),true);
 assert.equal(isNetflixOrigin('https://netflix.com/'),true);
 assert.equal(isNetflixOrigin('http://www.netflix.com/'),false);
 assert.equal(isNetflixOrigin('https://netflix.com.evil.com/'),false);
 const calls=[];
 const wc={
  isDestroyed:()=>false,
  getURL:()=> 'https://www.netflix.com/browse',
  executeJavaScript:async(src)=>{calls.push(src);return true}
 };
 assert.equal(await supportsEmbeddedWidevine(wc),true);
 assert.equal(calls.length,1);
 const notNetflix={...wc,getURL:()=> 'https://evil.example'};
 assert.equal(await supportsEmbeddedWidevine(notNetflix),null);
 assert.equal(calls.length,1);
});
