'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=name=>fs.readFileSync(path.join(__dirname,'..',name),'utf8');
const main=read('src/main.cjs');
const store=read('src/services/store.cjs');
const renderer=read('src/renderer/renderer.js');
const html=read('src/renderer/index.html');
const css=read('src/renderer/gekko-horizontal-bar.css');

test('one full-width bottom toolbar by default, including upgrades from vertical layouts',()=>{
 assert.match(store,/railPosition: 'bottom'/);
 assert.match(store,/existingSettings\.railPosition === 'top' \? 'top' : 'bottom'/);
 assert.match(main,/let railCollapsed = false/);
 assert.match(main,/let railPinned = true/);
 assert.match(main,/const horizontal = railPosition === 'top' \|\| railPosition === 'bottom'/);
 assert.match(main,/const bottomRail = railPosition === 'bottom'/);
 assert.match(main,/height - viewTop - bottom - bottomRail/);
 assert.match(main,/const HORIZONTAL_RAIL_H = 64/);
 assert.match(renderer,/document\.body\.dataset\.railPosition = \['top', 'bottom'\]/);
 assert.match(html,/href="gekko-horizontal-bar\.css"/);
});

test('the actual same navigation controls, search, apps and tabs share a single horizontal rail',()=>{
 assert.match(renderer,/sideStageRail\.append\(unifiedDock\)/);
 assert.match(renderer,/sideStageRail\.append\(appRow\)/);
 assert.match(renderer,/sideStageRail\.append\(shelf\)/);
 assert.match(renderer,/sideStageRail\.append\(windowControls\)/);
 assert.match(css,/body\[data-rail-position="bottom"\] #sidestage-rail:not\(\.rail-minimized\)/);
 assert.match(css,/width:100%!important/);
 assert.match(css,/height:64px!important/);
 assert.match(css,/flex-direction:row!important/);
 assert.match(css,/#sidestage-rail #bottom-dock \.address-wrap/);
 assert.match(css,/display:block!important/);
 assert.match(css,/#sidestage-rail \.gekko-tab-shelf \.tabs/);
 assert.match(css,/#sidestage-rail \.gekko-rail-window-controls/);
 assert.match(css,/inset:0 0 64px 0!important/);
});

test('tabs are horizontal, app shortcuts prioritize Mail and SOCIAL, and no scrollbar appears',()=>{
 assert.match(renderer,/\['quanticmail','quanticpulse'\]/);
 assert.match(css,/flex-wrap:nowrap!important/);
 assert.match(css,/overflow-y:hidden!important/);
 assert.match(css,/scrollbar-width:none!important/);
 assert.match(css,/::-webkit-scrollbar/);
 assert.match(css,/gap:3px!important/);
 assert.match(css,/@media\(max-width:950px\)/);
 assert.match(css,/#sidestage-rail \.gekko-side-app-row/);
 assert.match(main,/En bas · largeur entière/);
 assert.doesNotMatch(store,/railPosition: 'right'/);
});

test('native external website is not narrowed by an unused 220px sidebar',()=>{
 assert.match(main,/const sideLeft = railPosition === 'left' \?/);
 assert.match(main,/const sideRight = railPosition === 'right' \?/);
 assert.match(main,/const horizontal = railPosition === 'top' \|\| railPosition === 'bottom'/);
 assert.match(main,/width - sideLeft - sideRight/);
 assert.match(css,/bottom:0!important/);
});
