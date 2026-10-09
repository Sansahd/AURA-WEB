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

test('external web content starts at the top and reserves bottom dock plus tab rail', () => {
  const main = read('src/main.cjs');
  const css = read('src/renderer/gekko-24-polish.css');
  assert.match(main, /TOP_CHROME_H\s*=\s*0/);
  assert.match(main, /BOTTOM_DOCK_H\s*=\s*66/);
  assert.match(main, /height\s*-\s*top\s*-\s*bottom/);
  assert.match(main, /const rail = SIDESTAGE_RAIL_W/);
  assert.match(css, /#chrome\s*\{display:none!important\}/);
  assert.match(css, /#sidestage-rail \.gekko-rail-window-controls/);
});

test('navigation has no cinematic or fade overlay', () => {
  const main = read('src/main.cjs');
  const renderer = read('src/renderer/renderer.js');
  const html = read('src/renderer/index.html');
  assert.doesNotMatch(html, /id="navigation-transition"/);
  assert.doesNotMatch(renderer, /navigationTransition/);
  assert.doesNotMatch(main, /preparePageFadeOut|installPageFadeIn|releasePageFadeIn/);
  assert.doesNotMatch(main, /MIN_NAV_TRANSITION_MS|PAGE_FADE_OUT_MS|PAGE_FADE_IN_MS/);
  assert.match(main, /did-start-navigation/);
  assert.match(main, /view\.setVisible\(true\)/);
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


test('omnibox auto-launches reserved site aliases without Enter', () => {
  const renderer = read('src/renderer/renderer.js');
  const preload = read('src/preload.cjs');
  const main = read('src/main.cjs');
  const firewall = read('src/security/ipc-firewall.cjs');

  assert.match(renderer, /maybeInstantLaunch/);
  assert.match(renderer, /instantSite/);
  assert.match(renderer, /maybePrewarmPopularSite/);
  assert.match(renderer, /prewarmSite/);
  assert.match(preload, /prewarmSite: \(u\) => ipcRenderer\.invoke\('prewarm-site', u\)/);
  assert.match(main, /normalSession\.preconnect/);
  assert.match(main, /ipcMain\.handle\('prewarm-site'/);
  assert.match(firewall, /'prewarm-site'/);
});


test('YouTube video pages proactively enter GEKKO Direct before ad playback', () => {
  const main = read('src/main.cjs');
  assert.match(main, /function scheduleYouTubeDirect/);
  assert.match(main, /scheduleYouTubeDirect\(tab, 0\)/);
  assert.match(main, /scheduleYouTubeDirect\(tab, 60\)/);
  assert.match(main, /extractYouTubeVideoId/);
});





test('search engine picker is a compact native popup and never opens a full-width gap', () => {
  const polish = read('src/renderer/gekko-24-polish.css');
  const renderer = read('src/renderer/renderer.js');
  const main = read('src/main.cjs');
  const preload = read('src/preload.cjs');
  const firewall = read('src/security/ipc-firewall.cjs');

  assert.match(polish, /#search-engine-menu\{display:none!important\}/);
  assert.match(renderer, /window\.quantic\.searchEngineMenu\(/);
  assert.doesNotMatch(renderer, /engineOpen \? 326/);
  assert.match(main, /function showCompactSearchEngineMenu/);
  assert.match(main, /Menu\.buildFromTemplate\(engines\.map/);
  assert.match(main, /menu\.popup\(\{/);
  assert.match(preload, /searchEngineMenu: \(x,y\) => ipcRenderer\.invoke\('search-engine-menu',x,y\)/);
  assert.match(firewall, /'search-engine-menu': \(args\)/);
  assert.match(main, /BOTTOM_DOCK_H \+ chromeOverlayHeight/);
});

test('omnibox hypercache can prewarm several likely destinations on the first keystroke', () => {
  const renderer = read('src/renderer/renderer.js');
  assert.match(renderer, /prewarmSites\?\.\(query, 3\)/);
  assert.match(renderer, /prewarmKeys = new Set/);
});


test('favorites use the heart-eyes control requested for bookmarking', () => {
  const html = read('src/renderer/index.html');
  const renderer = read('src/renderer/renderer.js');
  const css = read('src/renderer/quantic-glide-brand.css');
  assert.match(html, /class="favorite-heart-eyes"[^>]*>😍<\/span>/);
  assert.doesNotMatch(html.slice(html.indexOf('id="fav"'), html.indexOf('id="sidestage"')), /icon-star/);
  assert.match(renderer, /fav\.setAttribute\('aria-label', fav\.title\)/);
  assert.match(css, /favorite-heart-eyes/);
  assert.match(css, /#fav\.active/);
});

test('immersive dock snaps browser bounds and cannot uncover an animated empty gutter', () => {
  const main = read('src/main.cjs');
  const polish = read('src/renderer/gekko-24-polish.css');
  assert.match(main, /function setTabViewBounds/);
  assert.match(main, /function showChrome\(focusAddress = false\)/);
  assert.doesNotMatch(main, /layout\(\{ animateChrome: true \}\)/);
  assert.match(main, /function startImmersionWatcher/);
  assert.match(polish, /#bottom-dock\{[^}]*transition:none!important/s);
  assert.match(polish, /body\.chrome-hidden #bottom-dock\{[^}]*transition:none!important/s);
});

test('YouTube Direct is primed before DOM-ready when navigation starts', () => {
  const main = read('src/main.cjs');
  assert.match(main, /function primeYouTubeDirect/);
  assert.match(main, /primeYouTubeDirect\(_url\)/);
  assert.match(main, /youtubeDirectResolver\.resolve\(videoId\)/);
  assert.match(main, /scheduleYouTubeDirect\(tab, 60\)/);
});
