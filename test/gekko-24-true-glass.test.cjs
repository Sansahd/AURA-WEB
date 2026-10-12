'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
const {edgeRect,previewLayout,captureEdge}=require('../src/services/page-edge-preview.cjs');

test('edge preview uses ONLY a 72px strip of visible native page (not desktop)',()=>{
 const b={width:1100,height:700};
 assert.deepEqual(edgeRect(b,'top'),{x:0,y:0,width:1100,height:72});
 assert.deepEqual(edgeRect(b,'bottom'),{x:0,y:628,width:1100,height:72});
 assert.deepEqual(edgeRect(b,'left'),{x:0,y:0,width:72,height:700});
 assert.deepEqual(edgeRect(b,'right'),{x:1028,y:0,width:72,height:700});
 assert.equal(edgeRect(b,'diagonal'),null);
 assert.deepEqual(previewLayout('top'),{position:'center top',size:'100% 60px'});
 assert.deepEqual(previewLayout('right'),{position:'right center',size:'96px 100%'});
});
test('edge preview is downscaled, transient, and is not read when the view is loading',async()=>{
 const calls=[];
 const photo={isEmpty:()=>false,resize:opts=>{calls.push(opts);return{toDataURL:()=> 'data:image/png;base64,AAAA'}}};
 const wc={isDestroyed:()=>false,isLoading:()=>false,capturePage:async rect=>{calls.push(rect);return photo}};
 const sample=await captureEdge(wc,{width:1200,height:800},'bottom');
 assert.equal(sample.image,'data:image/png;base64,AAAA');
 assert.deepEqual(calls[0],{x:0,y:728,width:1200,height:72});
 assert.equal(calls[1].width,520);
 assert.equal(await captureEdge({...wc,isLoading:()=>true},{width:1200,height:800},'bottom'),null);
 assert.equal(calls.length,2);
});
test('native bridge is disabled for private mode and media and never persists screenshots',()=>{
 const main=read('src/main.cjs'),preload=read('src/preload.cjs');
 const ui=read('src/renderer/renderer.js'),css=read('src/renderer/gekko-home-persona.css');
 assert.match(main,/!isPrivateMode\(\) && chromeVisible && !tab\.mediaPlaying/);
 assert.match(main,/!tab\.videoFocused/);
 assert.match(main,/win\.webContents\.send\('page-edge-preview',preview\)/);
 assert.match(main,/pageEdgeTimer=setInterval/);
 assert.match(preload,/onPageEdge: \(fn\)/);
 assert.match(ui,/window\.quantic\.onPageEdge\(paintPageEdge\)/);
 assert.match(ui,/shell\.classList\.add\('has-page-edge'\)/);
 assert.match(ui,/shell\.classList\.remove\('has-page-edge'\)/);
 assert.match(css,/#shell\.has-page-edge/);
 assert.match(css,/--gekko-page-edge-image/);
 assert.doesNotMatch(main,/writeFileSync\([^)]*pageEdge/);
});
test('photo search is a single pill without the old tinted blur rectangle',()=>{
 const css=read('src/renderer/gekko-home-persona.css');
 assert.match(css,/body\.has-wallpaper \.gekko-home-search/);
 assert.match(css,/backdrop-filter:none!important/);
 assert.match(css,/background-image:none!important/);
 assert.match(css,/border-radius:999px!important/);
 assert.match(css,/body\.has-wallpaper \.gekko-home-search input/);
 assert.match(css,/#shell\s*\{/);
 assert.match(css,/background-image:var\(--quantic-wallpaper-image,none\)!important/);
});
test('favorite icons are stored locally as PNG data URLs with hard size limits',()=>{
 const main=read('src/main.cjs'),store=read('src/services/store.cjs'),ui=read('src/renderer/renderer.js');
 assert.match(main,/favicon: tab\.favicon \|\| ''/);
 assert.match(store,/this\.data\.favorites\.unshift\(\{ title: title \|\| url, url, favicon:safeIcon/);
 assert.match(ui,/item\.favicon\.length<70000/);
 assert.match(ui,/img\.src=item\.favicon/);
 assert.doesNotMatch(ui,/favicon\\.ico/);
});
