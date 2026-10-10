'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {sampleRailColor}=require('../src/services/rail-tint.cjs');
const source=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('local edge sample retains one RGB value, not a page screenshot',async()=>{
 const requested=[];
 const fake={
  isDestroyed:()=>false,
  capturePage:async rect=>{
   requested.push(rect);
   return {isEmpty:()=>false,resize:({width,height})=>{
    assert.equal(width,1);assert.equal(height,1);
    return {toBitmap:()=>Buffer.from([45,110,185,255])};
   }};
  }
 };
 const bounds={width:960,height:800};
 assert.equal(await sampleRailColor(fake,bounds,'right'),'rgb(185, 110, 45)');
 assert.deepEqual(requested[0],{x:928,y:0,width:32,height:620});
 assert.equal(await sampleRailColor(fake,bounds,'left'),'rgb(185, 110, 45)');
 assert.equal(requested[1].x,0);
 assert.equal(await sampleRailColor(fake,bounds,'top'),null);
 assert.equal(await sampleRailColor({isDestroyed:()=>true},bounds,'right'),null);
});

test('private pages cannot be sampled and only tint value crosses trusted shell IPC',()=>{
 const main=source('src/main.cjs'),preload=source('src/preload.cjs');
 const renderer=source('src/renderer/renderer.js');
 assert.match(main,/if \(isPrivateMode\(\) \|\| !tab/);
 assert.match(main,/lastRailTint = rgb/);
 assert.match(main,/win\.webContents\.send\('rail-site-tint', rgb\)/);
 assert.match(preload,/onRailSiteTint/);
 assert.match(renderer,/onRailSiteTint\(\(rgb\) =>/);
 assert.doesNotMatch(preload,/capturePage/);
 assert.match(source('src/renderer/gekko-24-layout.css'),/body\.rail-site-tinted/);
});
