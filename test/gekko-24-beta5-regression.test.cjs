'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const read = (p) => fs.readFileSync(path.join(__dirname, '..', p), 'utf8');

test('search selector is a glossy attached bubble, never a detached OS menu', () => {
  const main = read('src/main.cjs');
  const html = read('src/renderer/engine-picker.html');
  const picker = read('src/renderer/engine-picker.js');
  const preload = read('src/engine-picker-preload.cjs');
  const renderer = read('src/renderer/renderer.js');
  assert.match(main, /enginePopover\.loadFile\(/);
  assert.match(main, /parent: win, modal: false/);
  assert.match(main, /bounds\.y \+ Math\.round\(Number\(anchorY\)/);
  assert.match(main, /event\.sender !== enginePopover\.webContents/);
  assert.match(html, /class="bubble"/);
  assert.match(html, /border-radius:23px/);
  assert.match(html, /data-engine="tor"/);
  assert.match(picker, /window\.gekkoPicker\.choose/);
  assert.match(preload, /ipcRenderer\.send\('gekko-picker-choice', engine\)/);
  assert.match(renderer, /Math\.max\(0, Math\.round\(rect\.top\)\)/);
});

test('rail automatically contracts on leave and expands on right-edge hover', () => {
  const main = read('src/main.cjs');
  const renderer = read('src/renderer/renderer.js');
  assert.match(main, /let railCollapsed = true;/);
  assert.match(main, /function startRailHoverWatcher/);
  assert.match(main, /setInterval\(\(\) => \{/);
  assert.match(main, /setRailCollapsed\(false\)/);
  assert.match(main, /setRailCollapsed\(true\)/);
  assert.match(main, /railPinned = !railPinned/);
  assert.match(renderer, /state\.railPinned/);
});

test('window has three explicit sizes and keeps minimize and close working', () => {
  const main = read('src/main.cjs');
  const firewall = read('src/security/ipc-firewall.cjs');
  const renderer = read('src/renderer/renderer.js');
  const picker = read('src/renderer/size-picker.html');
  for(const size of ['small','medium','fullscreen']) assert.ok(picker.includes('data-size="' + size + '"'));
  assert.match(picker, /class="choices"/);
  assert.match(picker, /class="shape"/);
  assert.match(main, /function applyWindowSize\(choice\)/);
  assert.match(main, /win\.setFullScreen\(true\)/);
  assert.match(main, /function showWindowSizes/);
  assert.match(main, /if \(action === 'sizes'\) showWindowSizes\(\)/);
  assert.match(main, /if \(action === 'minimize'\) win\.minimize\(\)/);
  assert.match(main, /if \(action === 'close'\) win\.close\(\)/);
  assert.match(firewall, /'sizes'/);
  assert.match(renderer, /window\.quantic\.windowSizeMenu\(/);
});

test('Mail and ZOON never silently pretend an unavailable remote page is ready', () => {
  const manager = read('src/services/sidestage-manager.cjs');
  const definitions = read('src/services/sidestage.cjs');
  const renderer = read('src/renderer/renderer.js');
  assert.match(definitions, /quanticmail/);
  assert.match(definitions, /quanticpulse/);
  assert.match(definitions, /quanticmail\.onrender\.com/);
  assert.match(definitions, /xdsawyerlol\.github\.io\/QuanticSillage/);
  assert.match(definitions, /zoon\.html/);
  assert.match(read('src/renderer/index.html'), /quanticmail\.onrender\.com/);
  assert.match(read('src/renderer/index.html'), /QuanticSillage\/zoon\.html/);
  assert.match(manager, /showFailure/);
  assert.match(manager, /this\.status\.get\(appId\) !== 'error'/);
  assert.match(manager, /indisponible/);
  assert.match(manager, /Ouvrir dans un onglet/);
  assert.match(renderer, /app\.status === 'error'/);
});
