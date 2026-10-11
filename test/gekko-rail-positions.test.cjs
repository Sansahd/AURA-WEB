'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('Glass Corner persists all four dock positions and exposes an actual chooser',()=>{
 const store=read('src/services/store.cjs'),main=read('src/main.cjs'),ipc=read('src/security/ipc-firewall.cjs');
 assert.match(store,/railPosition: 'bottom'/);
 assert.match(store,/\['top','bottom','left','right'\]\.includes\(existingSettings\.railPosition\)/);
 assert.match(store,/\['top','bottom','left','right'\]\.includes\(value\)/);
 assert.match(ipc,/\['top', 'bottom', 'left', 'right'\]\.includes\(value\)/);
 for(const pos of ["top","bottom","left","right"]){
   assert.ok(main.includes("chooseRailPosition('"+pos+"')"),pos);
 }
 assert.match(main,/store\.setSetting\('railPosition', position\)/);
});

test('native pages reserve only the chosen edge and retain reader support',()=>{
 const main=read('src/main.cjs'),css=read('src/renderer/gekko-glass-wide.css');
 assert.match(main,/const topRail = !chromeHidden && railPosition === 'top'/);
 assert.match(main,/const bottom = !chromeHidden && railPosition === 'bottom'/);
 assert.match(main,/const leftRail = railPosition === 'left'/);
 assert.match(main,/const rightRail = railPosition === 'right'/);
 assert.match(main,/const sideLeft = leftRail/);
 assert.match(main,/const sideRight = rightRail \+ stageWidth \+ aiWidth/);
 assert.match(main,/const bounds = \{ x: sideLeft, y: viewTop, width: Math\.max\(1, width - sideLeft - sideRight\), height: availableHeight \}/);
 assert.match(css,/height:54px!important/);
 assert.match(css,/body\[data-rail-position="top"\] #bottom-dock/);
 assert.match(css,/body\[data-rail-position="left"\] #bottom-dock/);
 assert.match(css,/body\[data-rail-position="right"\] #bottom-dock/);
 assert.match(css,/width:80px!important/);
 assert.match(css,/height:calc\(100vh - 16px\)!important/);
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
