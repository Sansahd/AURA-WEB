'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {NativeMediaFocus,FOCUS_STYLE}=require('../src/services/media-focus.cjs');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
const settle=()=>new Promise(resolve=>setImmediate(resolve));

class FakeWebContents{
 constructor(){this.destroyed=false;this.inserted=[];this.removed=[];this.next=0}
 isDestroyed(){return this.destroyed}
 insertCSS(css,options){
  assert.equal(options.cssOrigin,'user');
  this.inserted.push(css);
  return Promise.resolve('style-'+(++this.next));
 }
 removeInsertedCSS(key){this.removed.push(key);return Promise.resolve()}
}

test('cinematic focus darkens active external tab and reverses after closing reader',async()=>{
 const focus=new NativeMediaFocus();
 const tab=new FakeWebContents();
 assert.match(FOCUS_STYLE,/brightness\(\.36\)/);
 focus.sync(tab,true);
 await settle();
 assert.equal(tab.inserted.length,1);
 assert.equal(tab.removed.length,0);
 focus.sync(tab,true);
 assert.equal(tab.inserted.length,1,'rerender must not inject duplicate CSS');
 focus.clear();
 await settle();
 assert.deepEqual(tab.removed,['style-1']);
 assert.equal(focus.webContents,null);
});

test('switching main tabs transfers focus without dimming inactive tabs',async()=>{
 const focus=new NativeMediaFocus();
 const a=new FakeWebContents(),b=new FakeWebContents();
 focus.sync(a,true);await settle();
 focus.sync(b,true);await settle();
 assert.deepEqual(a.removed,['style-1']);
 assert.equal(b.inserted.length,1);
 focus.clear();await settle();
 assert.deepEqual(b.removed,['style-1']);
});

test('late CSS insertion never leaves an invisible focus effect behind',async()=>{
 const focus=new NativeMediaFocus();
 let resolve;
 const a=new FakeWebContents();
 a.insertCSS=()=>new Promise(r=>{resolve=r});
 focus.sync(a,true);
 focus.clear();
 resolve('late-style');
 await settle();
 assert.deepEqual(a.removed,['late-style']);
 assert.equal(focus.webContents,null);
});

test('navigation reapplies dimmer and cleans up the previous style',async()=>{
 const focus=new NativeMediaFocus();
 const wc=new FakeWebContents();
 focus.sync(wc,true);await settle();
 focus.refresh(wc);await settle();
 assert.equal(wc.inserted.length,2);
 assert.deepEqual(wc.removed,['style-1']);
 focus.clear();await settle();
 assert.deepEqual(wc.removed,['style-1','style-2']);
});

test('SOCIAL and Mail use full GEKKO browser tabs instead of failing small WebContentsView',()=>{
 const main=read('src/main.cjs');
 const ui=read('src/renderer/renderer.js');
 const sidestage=read('src/services/sidestage-manager.cjs');
 assert.match(main,/\['quanticmail', 'quanticpulse'\]\.includes\(appId\)/);
 assert.match(main,/togglePanelTab\(appDefinition\(appId\)\.url\)/);
 assert.match(main,/normalized === 'https:\/\/quanticmail\.onrender\.com\/'/);
 assert.match(main,/const activated = activateTab\(existing\.id\)/);
 assert.match(main,/existing\.id === activeId\) return closeTab\(existing\.id\)/);
 assert.match(ui,/fullAppActive/);
 assert.match(ui,/second clic pour fermer/);
 assert.match(sidestage,/view\.setBounds\(\{ x, y, width/);
 assert.doesNotMatch(sidestage,/focusOverlay\(\)/);
});

test('media focus works on real external website content for all bar positions',()=>{
 const main=read('src/main.cjs');
 const ui=read('src/renderer/renderer.js');
 assert.match(main,/const focusEnabled = Boolean\(/);
 assert.match(main,/stageState\.apps\?\.some\(app => app\.id === stageState\.activeApp && app\.media\)/);
 assert.match(main,/mediaFocus\.sync\(tab\?\.view\?\.webContents, focusEnabled\)/);
 assert.match(main,/mediaFocus\.refresh\(wc\)/);
 assert.match(main,/const railPosition = store\?\.settings\(\)\.railPosition \|\| 'bottom'/);
 assert.match(ui,/focus-reader-active/);
});
