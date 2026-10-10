'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const src=name=>fs.readFileSync(path.join(__dirname,'..',name),'utf8');
const css=src('src/renderer/gekko-24-layout.css');
const corner=css.split('/* GEKKO glass corner (beta 15)')[1];
const old=css.split('/* GEKKO 2.4 beta 14')[1];
const main=src('src/main.cjs');

test('glass corner joins bottom dock to right and left rails without full-width overlay',()=>{
 assert.ok(corner,'corner variant is installed last in the cascade');
 assert.match(corner,/body\[data-rail-position="right"\] #bottom-dock/);
 assert.match(corner,/body\[data-rail-position="left"\] #bottom-dock/);
 assert.match(corner,/width:min\(885px,calc\(100vw - 65px\)\)/);
 assert.match(corner,/right:51px!important/);
 assert.match(corner,/left:51px!important/);
 assert.match(corner,/bottom:6px!important/);
 assert.match(corner,/width:50px!important/);
 assert.match(main,/const SIDESTAGE_RAIL_W = 220/);
 assert.match(main,/const BOTTOM_DOCK_H = 0/);
});

test('search input stops monopolizing the dock but every app retains its actions',()=>{
 assert.match(corner,/#bottom-dock \.address-wrap/);
 assert.match(corner,/max-width:380px!important/);
 assert.match(corner,/min-width:155px!important/);
 assert.match(src('src/renderer/index.html'),/id="dock-drag-grip"/);
 assert.match(src('src/renderer/window-shell.css'),/#bottom-dock button,/);
 for(const name of ['dock-history','dock-primary','dock-actions','search-engine-button','address-go']){
  assert.ok(src('src/renderer/index.html').includes(name),name);
 }
 assert.match(old,/-webkit-app-region:drag!important/);
 assert.match(old,/-webkit-app-region:no-drag!important/);
});

test('ultra light glass has no opaque 90 percent wash and honors site layouts',()=>{
 assert.match(corner,/--gekko-chrome-alpha:clamp\(\.10/);
 assert.match(corner,/backdrop-filter:blur\(24px\)/);
 assert.match(corner,/rgba\(12,20,26,\.24\)/);
 assert.match(corner,/body\.rail-minimized\[data-rail-position="right"\]/);
 assert.match(corner,/body\.rail-minimized\[data-rail-position="left"\]/);
 assert.match(corner,/body\[data-rail-position="top"\] #bottom-dock/);
 assert.match(corner,/@media\(max-width:760px\)/);
});
