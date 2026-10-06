const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(root, file), 'utf8');

test('GEKKO branding replaces visible Glide branding', () => {
  const html = read('src/renderer/index.html');
  const logo = read('src/assets/quantic-glide-logo.svg');
  assert.match(html, /<title>GEKKO<\/title>/);
  assert.match(html, /BROWSE FREELY/);
  assert.match(logo, />GEKKO</);
  assert.match(logo, /BROWSE FREELY/);
});

test('primary chrome uses a bottom omnibox dock', () => {
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/styles.css');
  assert.match(html, /id="bottom-dock"/);
  assert.match(html, /id="address"/);
  const dockIndex = html.indexOf('id="bottom-dock"');
  const addressIndex = html.indexOf('id="address"');
  assert.ok(dockIndex >= 0 && addressIndex > dockIndex, 'address input must live inside/after bottom dock');
  assert.match(css, /#bottom-dock\s*\{[^}]*bottom:0/s);
});

test('GEKKO icon keeps a recognizable Earth gripped by the gecko', () => {
  const icon = read('src/assets/quantic-glide-icon.svg');
  assert.match(icon, /id="earth"/);
  assert.match(icon, /id="gecko"/);
  assert.match(icon, /aria-label="GEKKO/);
  assert.match(icon, /class="continent"/);
  assert.match(icon, /class="toe"/);
});

test('external web content reserves top tabs and bottom dock', () => {
  const main = read('src/main.cjs');
  assert.match(main, /TOP_CHROME_H\s*=\s*42/);
  assert.match(main, /BOTTOM_DOCK_H\s*=\s*60/);
  assert.match(main, /height\s*-\s*top\s*-\s*bottom/);
});

test('navigation transition is lifecycle driven and reduced-motion safe', () => {
  const main = read('src/main.cjs');
  const renderer = read('src/renderer/renderer.js');
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/styles.css');
  assert.match(main, /transitioning:\s*Boolean\(tab\.transitioning\)/);
  assert.match(main, /did-start-navigation/);
  assert.match(main, /tab\.transitioning\s*=\s*true/);
  assert.match(renderer, /navigation-transition/);
  assert.match(html, /id="navigation-transition"/);
  assert.match(css, /prefers-reduced-motion:reduce/);
});


test('new tab matches the approved cinematic GEKKO browser composition', () => {
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/quantic-glide-brand.css');

  for (const marker of [
    'gekko-home-scene',
    'gekko-home-brand',
    'gekko-home-cards',
    'gekko-home-card',
    'gekko-home-mountains',
    'gekko-home-aurora'
  ]) assert.match(html, new RegExp(marker));

  const cards = (html.match(/class="gekko-home-card"/g) || []).length;
  assert.equal(cards, 4);
  assert.match(html, /Private Search/);
  assert.match(html, /Secure Tabs/);
  assert.match(html, /Fast &amp; Light/);
  assert.match(html, /Explore More/);

  assert.match(css, /clip-path/);
  assert.match(css, /drop-shadow/);
  assert.match(css, /backdrop-filter/);
  assert.match(css, /radial-gradient/);
});

test('new tab visual controls stay functional', () => {
  const html = read('src/renderer/index.html');
  const renderer = read('src/renderer/renderer.js');
  assert.match(html, /data-home-action="private-search"/);
  assert.match(html, /data-home-action="secure-tabs"/);
  assert.match(html, /data-home-action="fast-light"/);
  assert.match(html, /data-home-action="explore-more"/);
  assert.match(renderer, /data-home-action/);
  assert.match(renderer, /private-search/);
  assert.match(renderer, /secure-tabs/);
});


test('Apps button opens a real Quantic launcher', () => {
  const html = read('src/renderer/index.html');
  const renderer = read('src/renderer/renderer.js');
  assert.match(html, /id="apps-panel"/);
  assert.match(html, /id="apps-button"/);
  assert.match(html, />AURA</);
  assert.match(html, />Mail</);
  assert.match(html, />ZOON</);
  assert.match(html, />News</);
  assert.match(html, />Providence</);
  assert.match(renderer, /data-app-url/);
  assert.match(renderer, /appsPanel/);
});
