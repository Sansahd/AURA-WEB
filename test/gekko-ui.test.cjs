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
  assert.match(main, /BOTTOM_DOCK_H\s*=\s*66/);
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
  assert.match(main, /tab\.transitioning\s*=\s*false/);
  assert.match(main, /MIN_NAV_TRANSITION_MS\s*=\s*260/);
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


test('Discover actions navigate immediately', () => {
  const renderer = read('src/renderer/renderer.js');
  const matches = renderer.match(/window\.quantic\.navigate\('Découvrir le web'\)/g) || [];
  assert.ok(matches.length >= 2);
});


test('omnibox search palette exposes GEKKO, privacy engines and Tor', () => {
  const html = read('src/renderer/index.html');
  const renderer = read('src/renderer/renderer.js');
  const navigation = read('src/core/navigation.cjs');
  const store = read('src/services/store.cjs');

  for (const engine of ['gekko','duckduckgo','qwant','startpage','brave','searxng','tor']) {
    assert.match(html, new RegExp('data-search-engine="' + engine + '"'));
  }
  assert.match(html, /GEKKO Search/);
  assert.match(html, /Tor · Veil/);
  assert.match(html, /engine-menu-grid/);
  assert.match(html, /engine-route-pill/);
  assert.match(renderer, /SEARCH_ENGINES = Object\.freeze/);
  assert.match(renderer, /setSetting\('searchEngine', engine\)/);
  assert.match(navigation, /requiresTor: selected === 'tor'/);
  assert.match(store, /searchEngine: 'gekko'/);
});

test('search palette uses premium two-column engine cards', () => {
  const css = read('src/renderer/styles.css');
  assert.match(css, /\.search-engine-menu\{[^}]*width:492px/s);
  assert.match(css, /\.engine-menu-grid\{display:grid;grid-template-columns:repeat\(2,minmax\(0,1fr\)\)/);
  assert.match(css, /\.engine-option\.featured\{grid-column:1\/-1/);
  assert.match(css, /\.engine-option\.tor-option\{grid-column:1\/-1/);
  assert.match(css, /search-engine-button\[data-engine="gekko"\]/);
});

test('bottom dock uses round vector controls instead of text bars', () => {
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/styles.css');
  assert.match(html, /class="dock-control"/);
  assert.match(html, /<svg viewBox=/);
  assert.match(css, /\.dock-control\s*\{[^}]*border-radius:50%/s);
  assert.match(css, /\.address\s*\{[^}]*border-radius:23px/s);
});

test('cinematic page bridge uses directional curtains and light streaks', () => {
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/styles.css');
  assert.match(html, /cinema-curtain-left/);
  assert.match(html, /cinema-curtain-right/);
  assert.match(html, /cinema-streaks/);
  assert.match(css, /@keyframes cinema-left/);
  assert.match(css, /@keyframes cinema-right/);
  assert.match(css, /@keyframes streak-pass/);
});


test('premium control pass removes the legacy v2 dock collision', () => {
  const css = read('src/renderer/quantic-glide-brand.css');
  assert.doesNotMatch(css, /\/\* GEKKO v2 dock composition \*\//);
  assert.doesNotMatch(css, /\.dock-nav button\{/);
  assert.doesNotMatch(css, /\.dock-brand\{/);
  assert.match(css, /GEKKO premium controls/);
  assert.match(css, /#bottom-dock \.dock-control/);
});

test('home controls use vector icons and compact horizontal action tiles', () => {
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/quantic-glide-brand.css');

  const tools = html.slice(html.indexOf('class="gekko-home-tools"'), html.indexOf('class="gekko-home-brand"'));
  assert.match(tools, /class="home-tool-button"/);
  assert.match(tools, /<svg viewBox=/);
  assert.doesNotMatch(tools, />[⌂♢▦♙]</);

  const cards = html.slice(html.indexOf('class="gekko-home-cards"'), html.indexOf('class="gekko-home-caption"'));
  assert.match(cards, /class="home-card-icon"/);
  assert.match(cards, /class="home-card-copy"/);
  assert.match(cards, /class="home-card-arrow"/);
  assert.match(css, /grid-template-columns:42px minmax\(0,1fr\) 22px/);
  assert.match(css, /min-height:78px!important/);
  assert.match(css, /border:1px solid rgba\(210,255,249,.095\)!important/);
});


test('cinematic bridge holds a live loading scene instead of going blank', () => {
  const html = read('src/renderer/index.html');
  const css = read('src/renderer/quantic-glide-brand.css');
  const renderer = read('src/renderer/renderer.js');
  const main = read('src/main.cjs');

  assert.match(html, /id="cinema-target"/);
  assert.match(html, /class="cinema-loader"/);
  assert.match(html, /id="cinema-status"/);
  assert.match(css, /Cinematic bridge is now stateful/);
  assert.match(css, /cinema-hold-breathe/);
  assert.match(css, /cinema-loader-pass/);
  assert.match(renderer, /cinematicTargetFor/);
  assert.match(renderer, /Création du circuit privé/);
  assert.match(main, /tab\.transitioning = true/);
  assert.match(main, /setTimeout\(resolve, 32\)/);
});

test('omnibox includes a zero-network popular-site suggestion surface', () => {
  const html = read('src/renderer/index.html');
  const renderer = read('src/renderer/renderer.js');
  const css = read('src/renderer/quantic-glide-brand.css');

  assert.match(html, /id="site-suggestions"/);
  assert.match(html, /site-cache\.js/);
  assert.match(renderer, /GekkoSiteCache/);
  assert.match(renderer, /updateSiteSuggestions/);
  assert.match(renderer, /ArrowDown/);
  assert.match(renderer, /ArrowUp/);
  assert.match(css, /\.site-suggestions/);
  assert.match(css, /\.site-suggestion\.selected/);
});
