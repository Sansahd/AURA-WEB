'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=(p)=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('engine picker shows seven offline brand SVGs in two rows, no scrolling',()=>{
 const html=read('src/renderer/engine-picker.html');
 const main=read('src/main.cjs');
 assert.match(html,/grid-template-columns:repeat\(8,minmax\(0,1fr\)\)/);
 assert.match(html,/\.engine:nth-child\(7\)\{grid-column:6\/span 2\}/);
 assert.match(html,/html,body\{[^}]*overflow:hidden/s);
 assert.match(html,/\.symbol\{[^}]*border-radius:50%/s);
 assert.match(main,/width: 276, height: 216/);
 for(const engine of ['duckduckgo','qwant','startpage','brave','searxng','torbrowser']){
   assert.ok(html.includes('engine-icons/'+engine+'.svg'),engine+' logo missing');
   const svg=read('src/renderer/engine-icons/'+engine+'.svg');
   assert.match(svg,/<svg/);
   assert.match(svg,/<path/);
 }
 assert.match(html,/quantic-glide-mark\.svg/);
});

test('three visual window sizes are routed through a sandboxed child window',()=>{
 const main=read('src/main.cjs');
 const renderer=read('src/renderer/renderer.js');
 const html=read('src/renderer/size-picker.html');
 const js=read('src/renderer/size-picker.js');
 const preload=read('src/size-picker-preload.cjs');
 const trusted=read('src/preload.cjs');
 const firewall=read('src/security/ipc-firewall.cjs');
 for(const choice of ['small','medium','fullscreen'])assert.ok(html.includes('data-size="'+choice+'"'));
 assert.match(html,/grid-template-columns:repeat\(3,minmax\(0,1fr\)\)/);
 assert.match(html,/class="shape"/);
 assert.match(html,/overflow:hidden/);
 assert.match(main,/function applyWindowSize\(choice\)/);
 assert.match(main,/win\.setFullScreen\(true\)/);
 assert.match(main,/sizePopover\.loadFile\(/);
 assert.match(main,/event\.sender !== sizePopover\.webContents/);
 assert.match(js,/gekkoSize\.choose/);
 assert.match(preload,/ipcRenderer\.send\('gekko-size-choice',size\)/);
 assert.match(trusted,/windowSizeMenu: \(x, y\)/);
 assert.match(firewall,/'window-size-menu': \(args\)/);
 assert.match(renderer,/window\.quantic\.windowSizeMenu\(/);
});

test('auto-retract pin and chevron are independent, preserving drag in motion',()=>{
 const main=read('src/main.cjs');
 const renderer=read('src/renderer/renderer.js');
 const css=read('src/renderer/gekko-24-layout.css');
 assert.match(main,/ipcMain\.handle\('toggle-rail-pin'/);
 assert.match(main,/ipcMain\.handle\('toggle-rail-collapse'/);
 assert.match(main,/const shouldCollapse = railPinned \? false : Boolean\(next\)/);
 assert.match(main,/railManualUntil/);
 assert.match(main,/railDragUntil/);
 assert.match(main,/let railAwaitPointerExit = false/);
 assert.match(main,/if \(inside && railAwaitPointerExit\) return/);
 assert.match(main,/railAwaitPointerExit = next/);
 assert.match(main,/win\.on\('move', \(\) => \{ railDragUntil/);
 assert.match(renderer,/gekko-rail-pin/);
 assert.doesNotMatch(renderer,/const railToggle = el\(/);
 assert.match(renderer,/gekko-rail-reveal/);
 assert.match(renderer,/pinButton\.onclick/);
 assert.match(css,/#sidestage-rail \.gekko-rail-pin\.is-pinned/);
});

test('bar and search get layered mint/orchid/peach ambient gradients',()=>{
 const css=read('src/renderer/gekko-24-layout.css');
 assert.match(css,/--gekko-mint-wash/);
 assert.match(css,/--gekko-orchid-wash/);
 assert.match(css,/--gekko-peach-wash/);
 assert.match(css,/#bottom-dock\{/);
 assert.match(css,/#sidestage-rail:not\(\.rail-minimized\)/);
});
