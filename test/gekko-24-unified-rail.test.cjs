'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
const main=read('src/main.cjs'),ui=read('src/renderer/renderer.js');
const css=read('src/renderer/gekko-24-layout.css');
const unified=css.split('/* GEKKO 2.4 beta 16')[1];

test('one actual rail contains the old dock DOM, navigation commands and search launcher',()=>{
 assert.ok(unified);
 assert.match(ui,/const unifiedDock = \$\('#bottom-dock'\)/);
 assert.match(ui,/sideStageRail\.append\(unifiedDock\)/);
 assert.match(ui,/sideStageRail\.append\(unifiedDock\)/);
 assert.match(ui,/headerActions\.append\(toolsToggle\)/);
 assert.match(unified,/#sidestage-rail #bottom-dock/);
 assert.match(unified,/position:static!important/);
 assert.match(unified,/background:transparent!important/);
 assert.match(unified,/flex-direction:column!important/);
});

test('no bottom bar reserves screen space from websites; search is a transient rail popover',()=>{
 assert.match(main,/const BOTTOM_DOCK_H = 0;/);
 assert.match(main,/const bottom = chromeHidden \? 0 : BOTTOM_DOCK_H \+ chromeOverlayHeight;/);
 assert.match(unified,/body\.rail-search-open #sidestage-rail:not\(\.rail-minimized\) #bottom-dock \.address-wrap/);
 assert.match(unified,/display:none!important/);
 assert.match(unified,/display:block!important/);
 assert.match(unified,/width:min\(410px,calc\(100vw - 86px\)\)/);
 assert.match(ui,/window\.quantic\.onFocusAddress\(\(\) => openRailSearch\(\)\)/);
});

test('keyboard search keeps rail open and all original button handlers are retained',()=>{
 assert.match(main,/railSearchFocused = Boolean\(locked\)/);
 assert.match(main,/if \(railPinned \|\| railSearchFocused\)/);
 assert.match(ui,/if \(state\.railCollapsed\) fire\(window\.quantic\.toggleRailCollapse\(\)\)/);
 for(const marker of ['id="back"','id="forward"','id="reload"','id="home-button"','id="bookmarks"','id="apps-button"','id="fav"','id="ai"','id="downloads"','id="persona"','id="menu"']){
   assert.ok(read('src/renderer/index.html').includes(marker),marker);
 }
 assert.match(unified,/-webkit-app-region:no-drag!important/);
});

test('single rail is lightly tinted, with no artificial assertion of real native website blur',()=>{
 assert.match(unified,/background:rgba\(12,18,24,\.07\)!important/);
 assert.match(unified,/backdrop-filter:blur\(17px\)/);
 assert.match(unified,/native page currently stops at the 54px rail reservation/);
});
