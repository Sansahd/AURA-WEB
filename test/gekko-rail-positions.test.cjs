'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=(p)=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('the shell offers a real persisted top/right/left rail position preference',()=>{
 const main=read('src/main.cjs');
 const store=read('src/services/store.cjs');
 const renderer=read('src/renderer/renderer.js');
 const firewall=read('src/security/ipc-firewall.cjs');
 const preload=read('src/preload.cjs');
 assert.match(store,/railPosition: 'right'/);
 assert.match(store,/railPosition: \['top', 'left', 'right'\]\.includes\(existingSettings\.railPosition\)/);
 assert.match(store,/key === 'railPosition'/);
 assert.match(main,/function chooseRailPosition\(position\)/);
 assert.match(main,/store\.setSetting\('railPosition', position\)/);
 assert.match(main,/function showRailPositionMenu\(\)/);
 for(const position of ['top','left','right']) assert.match(main,new RegExp(`selected === '${position}'`));
 assert.match(main,/ipcMain\.handle\('rail-position-menu'/);
 assert.match(preload,/railPositionMenu: \(\) => ipcRenderer\.invoke\('rail-position-menu'\)/);
 assert.match(firewall,/'rail-position-menu': noArgs/);
 assert.match(firewall,/key === 'railPosition'/);
 assert.match(renderer,/positionButton\.onclick = \(\) => fire\(window\.quantic\.railPositionMenu\(\)\)/);
 assert.match(renderer,/document\.body\.dataset\.railPosition/);
});

test('native external-page and SideStage geometry respects the position',()=>{
 const main=read('src/main.cjs');
 assert.match(main,/const horizontal = railPosition === 'top'/);
 assert.match(main,/const topRail = horizontal \?/);
 assert.match(main,/const sideLeft = railPosition === 'left' \? rail \+ stageWidth : 0/);
 assert.match(main,/const sideRight = railPosition === 'right' \?/);
 assert.match(main,/const bounds = \{ x: sideLeft, y: viewTop, width: Math\.max\(1, width - sideLeft - sideRight\), height: availableHeight \}/);
 assert.match(main,/const stageX = railPosition === 'left' \? rail/);
 assert.match(main,/sideStage\?\.layout\(\{/);
 assert.match(main,/position === 'top'/);
 assert.match(main,/position === 'left'/);
 assert.match(main,/railPosition === 'right'/);
});

test('position-specific top/left/right rail CSS protects page geometry',()=>{
 const css=read('src/renderer/gekko-24-layout.css');
 const html=read('src/renderer/index.html');
 assert.match(html,/gekko-24-layout\.css/);
 assert.match(css,/body\[data-rail-position="top"\]/);
 assert.match(css,/body\[data-rail-position="left"\]/);
 assert.match(css,/body\[data-rail-position="top"\] #sidestage-rail \.tab-title/);
 assert.match(css,/body\[data-rail-position="left"\]\.rail-minimized \.home/);
 assert.match(css,/body\[data-rail-position="top"\]\.rail-minimized \.home/);
 assert.match(css,/flex-direction:row!important/);
 assert.match(css,/flex-direction:row!important/);
 assert.match(css,/#sidestage-rail \.gekko-rail-position/);
});

test('dragging blank areas is supported while actionable children remain clickable',()=>{
 const css=read('src/renderer/gekko-24-layout.css');
 const renderer=read('src/renderer/renderer.js');
 assert.match(css,/#sidestage-rail:not\(\.rail-minimized\) \.tabs/);
 assert.match(css,/#sidestage-rail:not\(\.rail-minimized\) \.gekko-rail-window-controls/);
 assert.match(css,/-webkit-app-region:drag!important/);
 assert.match(css,/#sidestage-rail:not\(\.rail-minimized\) button \*/);
 assert.match(css,/-webkit-app-region:no-drag!important/);
 assert.match(renderer,/sideStageRail\.append\(grip\)/);
 assert.match(renderer,/sideStageRail\.append\(windowControls\)/);
});

test('color treatment blends mint, amber, rose and lilac without web-page filters',()=>{
 const css=read('src/renderer/gekko-24-layout.css');
 for(const value of ['--gekko-teal','--gekko-amber','--gekko-lilac','--gekko-rose'])assert.ok(css.includes(value));
 assert.match(css,/radial-gradient/);
 assert.match(css,/#bottom-dock/);
 assert.match(css,/#sidestage-rail:not\(\.rail-minimized\)/);
 assert.doesNotMatch(css,/webview|\.web-content|\.browser-view/);
});
