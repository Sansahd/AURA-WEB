'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=n=>fs.readFileSync(path.join(__dirname,'..',n),'utf8');

test('Glass Corner native Stage cannot cover GEKKO home or the bottom dock',()=>{
 const main=read('src/main.cjs');
 assert.match(main,/const homeOrInternal = !tab \|\| !isExternal\(tab\.url\)/);
 assert.match(main,/privateMode: isPrivateMode\(\) \|\| railCollapsed \|\| homeOrInternal/);
 assert.match(main,/createTab\(appDefinition\(appId\)\.url, true\)/);
 assert.match(main,/const sideLeft = 0/);
 assert.match(main,/const BOTTOM_DOCK_H = 60/);
});

test('GLASS-wide layout fills the true window without compressing address search',()=>{
 const css=read('src/renderer/gekko-glass-wide.css');
 assert.match(css,/#shell\s*\{/);
 assert.match(css,/width:100vw!important/);
 assert.match(css,/width:calc\(100vw - 16px\)!important/);
 assert.match(css,/#bottom-dock \.search-engine-label/);
 assert.match(css,/#bottom-dock \.address input/);
 assert.match(css,/flex:1 1 58%!important/);
 assert.match(css,/body\[data-rail-position="bottom"\] #home/);
 assert.match(css,/right:0!important/);
 assert.match(css,/max-width:none!important/);
});

test('Windows CI measures the actual Electron rectangle before building installer',()=>{
 const workflow=read('.github/workflows/gekko-24-windows-preview.yml');
 const smoke=read('src/services/ui-geometry-smoke.cjs');
 assert.match(workflow,/Test actual Electron layout on Windows/);
 assert.match(workflow,/npm run smoke:ui/);
 assert.match(smoke,/dock_does_not_span_window/);
 assert.match(smoke,/omnibox_not_visible/);
 assert.match(smoke,/home/);
 assert.match(smoke,/win\.capturePage\(\)/);
});
