'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const read = name => fs.readFileSync(path.join(__dirname, '..', name), 'utf8');
const html = read('src/renderer/index.html');
const shellCss = read('src/renderer/window-shell.css');
const layoutCss = read('src/renderer/gekko-24-layout.css');
const glass = layoutCss.split('/* GEKKO 2.4 beta 14')[1];
const main = read('src/main.cjs');

test('bottom dock has a dedicated native window grip and no blanket no-drag class', () => {
  assert.ok(glass);
  assert.match(html, /id="bottom-dock" class="bottom-dock"/);
  assert.doesNotMatch(html, /id="bottom-dock" class="bottom-dock no-drag"/);
  assert.match(html, /id="dock-drag-grip" class="dock-drag-grip"/);
  assert.match(glass, /#bottom-dock \.dock-drag-grip/);
  assert.match(glass, /-webkit-app-region:drag!important/);
  assert.doesNotMatch(shellCss, /\.nav-row,\s*\n\.nav-row \*/);
});

test('all interactive dock controls stay clickable, searchable and non-draggable', () => {
  assert.match(glass, /#bottom-dock \.address-wrap \*/);
  assert.match(glass, /#bottom-dock button \*/);
  assert.match(glass, /#bottom-dock input/);
  assert.match(glass, /-webkit-app-region:no-drag!important/);
  assert.match(glass, /#bottom-dock input\{cursor:text!important\}/);
  assert.match(shellCss, /#bottom-dock button/);
});

test('compact frosted dock and vertical rail agree with native viewport bounds', () => {
  assert.match(main, /const BOTTOM_DOCK_H = 60;/);
  assert.match(main, /const SIDESTAGE_RAIL_W = 54;/);
  assert.match(glass, /height:54px!important/);
  assert.match(glass, /bottom:6px!important/);
  assert.match(glass, /width:50px!important/);
  assert.match(glass, /right:4px!important/);
  assert.match(glass, /left:4px!important/);
  assert.match(glass, /backdrop-filter: blur\(22px\)/);
  assert.match(glass, /background:linear-gradient/);
  assert.match(glass, /body\[data-rail-position="top"\] #sidestage-rail:not\(\.rail-minimized\)/);
});
