'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
const main=read('src/main.cjs');
const renderer=read('src/renderer/renderer.js');
const css=read('src/renderer/gekko-24-layout.css');
const fix=css.split('/* GEKKO 2.4 beta 17')[1];

test('real address input stays visible inside the single rail without needing a magnifier',()=>{
 assert.ok(fix);
 assert.match(renderer,/sideStageRail\.append\(unifiedDock\)/);
 assert.doesNotMatch(renderer,/sideStageRail\.append\(searchLaunch\)/);
 assert.match(fix,/#sidestage-rail #bottom-dock \.address-wrap/);
 assert.match(fix,/display:block!important/);
 assert.match(fix,/position:relative!important/);
 assert.match(fix,/order:-1!important/);
 assert.match(fix,/body\[data-rail-position="left"\] #sidestage-rail #bottom-dock \.address-wrap/);
 assert.match(fix,/body\[data-rail-position="top"\] #sidestage-rail #bottom-dock \.address-wrap/);
 assert.match(renderer,/onFocusAddress\(\(\) => openRailSearch\(\)\)/);
});

test('app buttons, tab list and window controls occupy separate nonoverlapping rows',()=>{
 assert.match(renderer,/const appRow = el\('div', 'gekko-side-app-row'\)/);
 assert.match(renderer,/const headerActions = el\('div', 'gekko-rail-header-actions'\)/);
 assert.match(renderer,/sideStageRail\.insertBefore\(headerActions, appRow\)/);
 assert.match(fix,/#sidestage-rail \.gekko-rail-header-actions/);
 assert.match(fix,/#sidestage-rail \.gekko-side-app-row/);
 assert.match(fix,/#sidestage-rail \.gekko-tab-shelf/);
 assert.match(fix,/#sidestage-rail \.gekko-rail-window-controls/);
 for(const slot of ['order:1!important','order:2!important','order:3!important','order:4!important','order:5!important']){
  assert.ok(fix.includes(slot),slot);
 }
 assert.match(fix,/overflow-y:auto!important/);
 assert.match(fix,/flex:1 1 auto!important/);
 assert.match(fix,/body:not\(\[data-rail-position="top"\]\) #sidestage-rail \.gekko-tab-shelf \.tab-title/);
});

test('full-width toolbar uses exactly its occupied 64px — no 220px column or empty footer',()=>{
 const horizontal=read('src/renderer/gekko-horizontal-bar.css');
 assert.match(main,/const BOTTOM_DOCK_H = 0/);
 assert.match(main,/const HORIZONTAL_RAIL_H = 64/);
 assert.match(main,/const bottomRail = railPosition === 'bottom'/);
 assert.match(main,/height - viewTop - bottom - bottomRail/);
 assert.match(main,/ipcMain\.handle\('chrome-overlay-height'/);
 assert.match(main,/chromeOverlayHeight = 0;/);
 assert.match(horizontal,/inset:0 0 64px 0!important/);
 assert.match(horizontal,/width:100%!important/);
 assert.match(horizontal,/scrollbar-width:none!important/);
});
