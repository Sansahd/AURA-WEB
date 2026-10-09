'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const read = (p) => fs.readFileSync(path.join(__dirname, '..', p), 'utf8');

test('rail collapse preserves a clickable 18px handle and gives width back to web content', () => {
  const main = read('src/main.cjs');
  const renderer = read('src/renderer/renderer.js');
  const css = read('src/renderer/gekko-24-polish.css');
  const firewall = read('src/security/ipc-firewall.cjs');
  const preload = read('src/preload.cjs');

  assert.match(main, /const SIDESTAGE_RAIL_MIN_W = 18;/);
  assert.match(main, /const rail = railCollapsed \? SIDESTAGE_RAIL_MIN_W : SIDESTAGE_RAIL_W;/);
  assert.match(main, /railCollapsed = shouldCollapse/);
  assert.match(main, /const shouldCollapse = railPinned \? false : Boolean\(next\)/);
  assert.match(main, /function startRailHoverWatcher/);
  assert.match(main, /railLastHover >= 1000/);
  assert.match(main, /railPinned = !railPinned/);
  assert.match(main, /sideStage\?\.hideAll\(\)/);
  assert.match(main, /emitState\(true\)/);
  assert.doesNotMatch(renderer, /const railToggle = el\(/);
  assert.match(renderer, /sideStageRail\.append\(reveal\)/);
  assert.match(renderer, /pinButton\.onclick = \(\) => fire\(window\.quantic\.toggleRailPin\(\)\)/);
  assert.match(renderer, /if \(minimized\) \{/);
  assert.match(css, /#sidestage-rail\.rail-minimized/);
  assert.match(css, /width:18px!important/);
  assert.match(css, /body\.rail-minimized \.home/);
  assert.match(preload, /toggleRailCollapse: \(\) => ipcRenderer\.invoke\('toggle-rail-collapse'\)/);
  assert.match(preload, /toggleRailPin: \(\) => ipcRenderer\.invoke\('toggle-rail-pin'\)/);
  assert.match(firewall, /'toggle-rail-collapse': noArgs/);
  assert.match(firewall, /'toggle-rail-pin': noArgs/);
});

test('whole unused sidebar surface is draggable but controls remain clickable', () => {
  const css = read('src/renderer/gekko-24-polish.css');
  assert.match(css, /#sidestage-rail:not\(\.rail-minimized\)/);
  assert.match(css, /-webkit-app-region:drag!important/);
  assert.match(css, /#sidestage-rail button,/);
  assert.match(css, /-webkit-app-region:no-drag!important/);
  assert.match(css, /#sidestage-rail \.gekko-rail-window-controls/);
});

test('engine selection no longer reserves 326px across the browser width', () => {
  const renderer = read('src/renderer/renderer.js');
  const main = read('src/main.cjs');
  assert.doesNotMatch(renderer, /engineOpen \? 326/);
  assert.match(renderer, /window\.quantic\.searchEngineMenu\(/);
  assert.match(main, /showCompactSearchEngineMenu/);
  assert.match(main, /enginePopover\.loadFile\(/);
  assert.match(main, /path\.join\(__dirname, 'renderer', 'engine-picker\.html'\)/);
});
