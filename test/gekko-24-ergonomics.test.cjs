'use strict';
const assert=require('node:assert/strict');
const test=require('node:test');
const fs=require('node:fs');
const path=require('node:path');
const file=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
test('the simplified toolbar retains access to apps and rail options',()=>{
  const html=file('src/renderer/index.html'),ui=file('src/renderer/renderer.js'),css=file('src/renderer/gekko-24-ergonomics.css');
  assert.match(html,/gekko-24-ergonomics\.css/);
  for(const name of ['position','pin'])assert.match(html,new RegExp('data-rail-action="'+name+'"'));
  assert.match(html,/data-app-url="quantic:\/\/identity"/);
  assert.match(ui,/window\.quantic\.railPositionMenu\(\)/);
  assert.match(ui,/window\.quantic\.toggleRailPin\(\)/);
  assert.match(css,/html body\[data-rail-position\] #bottom-dock > #sidestage-rail > :is\(/);
  assert.match(css,/\.side-stage-app,\.side-stage-brand/);
});
test('the accent color changes toolbar surface, not only the border',()=>{
  const css=file('src/renderer/gekko-24-ergonomics.css');
  assert.match(css,/--gekko-toolbar-accent:\s*var\(--quantic-accent/);
  assert.match(css,/--gekko-toolbar-glass:\s*var\(--gekko-toolbar-color/);
  assert.match(css,/background:\s*var\(--gekko-toolbar-glass\)\s*!important/);
  assert.match(css,/--gekko-toolbar-opacity:\s*var\(--quantic-glass-opacity/);
  assert.match(css,/background-image:\s*none\s*!important/);
  assert.match(file('src/renderer/renderer.js'),/function updateToolbarColor\(accent, opacity\)/);
});
test('window control targets are at least 40px in all layouts',()=>{
  const css=file('src/renderer/gekko-24-ergonomics.css');
  assert.match(css,/width:42px!important;min-width:42px!important/);
  assert.match(css,/height:42px!important;min-height:42px!important/);
  assert.match(css,/height:40px!important;min-height:40px!important/);
  const html=file('src/renderer/index.html');
  for(const win of ['minimize','maximize','close'])assert.match(html,new RegExp('data-win="'+win+'"'));
});

test('tab labels, favicons and close actions are enlarged in horizontal layouts',()=>{
  const css=file('src/renderer/gekko-24-ergonomics.css');
  assert.match(css,/flex:0 0 clamp\(156px,13vw,190px\)!important/);
  assert.match(css,/min-width:156px!important;max-width:190px!important/);
  assert.match(css,/font-size:13px!important;font-weight:570!important/);
  assert.match(css,/height:42px!important;min-height:42px!important/);
  assert.match(css,/width:23px!important;height:25px!important/);
  assert.match(css,/overflow-x:auto!important;overscroll-behavior-x:contain!important/);
});
test('vertical tabs use a large square favicon instead of unreadable micro labels',()=>{
  const css=file('src/renderer/gekko-24-ergonomics.css');
  assert.match(css,/width:64px!important;min-width:64px!important;max-width:64px!important/);
  assert.match(css,/height:46px!important;min-height:46px!important/);
  assert.match(css,/width:26px!important;height:26px!important/);
  const smoke=file('src/services/ui-geometry-smoke.cjs');
  assert.match(smoke,/tab_too_small_/);
  assert.match(smoke,/snapshot\.tab\.width < 145/);
});
