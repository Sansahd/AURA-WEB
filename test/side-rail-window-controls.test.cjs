'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const source = (file) => fs.readFileSync(path.join(root, file), 'utf8');

test('the top bar is removed without losing native window controls', () => {
  const css = source('src/renderer/gekko-24-polish.css');
  const renderer = source('src/renderer/renderer.js');
  const main = source('src/main.cjs');
  const html = source('src/renderer/index.html');

  assert.match(css, /#chrome\s*\{\s*display:none!important\s*\}/);
  assert.match(main, /const TOP_CHROME_H = 0;/);
  assert.match(main, /const top = chromeHidden \? 0 : TOP_CHROME_H;/);
  assert.match(renderer, /sideStageRail\.append\(windowControls\);/);
  assert.match(renderer, /sideStageRail\.append\(grip\);/);
  assert.match(css, /#sidestage-rail \.gekko-window-grip\s*\{[^}]*-webkit-app-region:drag;/s);
  assert.match(css, /#sidestage-rail \.gekko-rail-window-controls\s*\{[^}]*-webkit-app-region:no-drag;/s);
  for (const command of ['minimize', 'maximize', 'close']) {
    assert.ok(html.includes('data-win="' + command + '"'), command + ' button must remain available');
    assert.ok(main.includes("action === '" + command + "'"), command + ' action must remain wired');
  }
});

test('maximized state is synced to the right-side restore button', () => {
  const main = source('src/main.cjs');
  const renderer = source('src/renderer/renderer.js');
  assert.match(main, /windowMaximized: Boolean\(win && !win\.isDestroyed\(\) && win\.isMaximized\(\)\)/);
  assert.match(renderer, /const maximized = Boolean\(state\.windowMaximized\);/);
  assert.match(renderer, /maximizeControl\.setAttribute\('aria-label', label\);/);
});

test('quick search input and its instant launch path remain intact', () => {
  const renderer = source('src/renderer/renderer.js');
  assert.match(renderer, /address\.oninput = \(\) => \{/);
  assert.match(renderer, /if \(maybeInstantLaunch\(value\)\) return;/);
});
