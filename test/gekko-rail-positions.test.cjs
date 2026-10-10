'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('Glass Corner preferences migrate from side rail to one bottom-width bar',()=>{
 const store=read('src/services/store.cjs'),main=read('src/main.cjs');
 assert.match(store,/railPosition: 'bottom'/);
 assert.match(store,/railPosition: 'bottom', \/\/ Migrate/);
 assert.match(store,/key === 'railPosition' && value !== 'bottom'/);
 assert.match(main,/let railPinned = true/);
 assert.match(main,/let railCollapsed = false/);
 assert.match(main,/En bas · barre pleine largeur/);
 assert.match(main,/store\.setSetting\('railPosition', position\)/);
});

test('native website keeps full horizontal width and Glass Corner dock occupies only 60px',()=>{
 const main=read('src/main.cjs');
 const css=read('src/renderer/gekko-glass-wide.css');
 assert.match(main,/const BOTTOM_DOCK_H = 60/);
 assert.match(main,/const sideLeft = 0/);
 assert.match(main,/const sideRight = stageWidth \+ aiWidth/);
 assert.match(main,/const bounds = \{ x: sideLeft, y: viewTop, width: Math\.max\(1, width - sideLeft - sideRight\), height: availableHeight \}/);
 assert.match(main,/const stageX = Math\.max\(0, width - stageWidth\)/);
 assert.match(css,/width:calc\(100% - 16px\)!important/);
 assert.match(css,/inset:0 0 60px 0!important/);
 assert.match(css,/height:54px!important/);
 assert.match(css,/background:var\(--gekko-chrome-background\)!important/);
});

test('Glass Corner original sidebar controls really share the dock with permanent search',()=>{
 const renderer=read('src/renderer/renderer.js');
 const css=read('src/renderer/gekko-glass-wide.css');
 const html=read('src/renderer/index.html');
 assert.match(renderer,/glassDock\.append\(sideStageRail\)/);
 assert.match(renderer,/shelf\.append\(plusButton\)/);
 assert.match(renderer,/button\.dataset\.appId = app\.id/);
 assert.match(css,/#bottom-dock > #sidestage-rail/);
 assert.match(css,/#bottom-dock > #sidestage-rail \.tabs/);
 assert.match(css,/flex-direction:row!important/);
 assert.match(css,/scrollbar-width:none!important/);
 assert.match(css,/::-webkit-scrollbar\{display:none!important\}/);
 assert.match(css,/\[data-app-id="quanticmail"\]/);
 assert.match(css,/\[data-app-id="quanticpulse"\]/);
 assert.match(html,/href="gekko-24-layout.css"/);
 assert.match(html,/href="gekko-glass-wide.css"/);
});

test('all toolbar interactive children can be clicked without dragging the window',()=>{
 const css=read('src/renderer/gekko-glass-wide.css');
 const original=read('src/renderer/gekko-24-layout.css');
 assert.match(original,/-webkit-app-region:drag!important/);
 assert.match(original,/-webkit-app-region:no-drag!important/);
 assert.match(css,/#bottom-dock \.dock-control/);
 assert.match(css,/-webkit-app-region:no-drag!important/);
 assert.match(css,/#bottom-dock > #sidestage-rail \.gekko-rail-window-controls/);
});
