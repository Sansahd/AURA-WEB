'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('homepage has one real center search connected to the existing navigation',()=>{
 const html=read('src/renderer/index.html'),js=read('src/renderer/renderer.js');
 assert.match(html,/id="home-search-form"/);
 assert.match(html,/id="home-search-input"/);
 assert.match(html,/role="search"/);
 assert.match(js,/homeSearch\.onsubmit=\(event\)=>/);
 assert.match(js,/fire\(window\.quantic\.navigate\(value\)\)/);
 assert.match(js,/onFocusHomeSearch\(\(\) =>/);
 assert.match(js,/input\.focus\(\);input\.select\(\)/);
});
test('favorites are the real locally saved bookmarks and update on any state refresh',()=>{
 const main=read('src/main.cjs'),html=read('src/renderer/index.html'),js=read('src/renderer/renderer.js');
 assert.match(main,/homeFavorites: store\?\.favorites\(12\) \|\| \[\]/);
 assert.match(html,/id="home-favorites-list"/);
 assert.match(js,/function renderHomeFavorites\(\)/);
 assert.match(js,/renderHomeFavorites\(\);/);
 assert.match(js,/window\.quantic\.navigate\(item\.url\)/);
 assert.match(js,/window\.quantic\.togglePanelTab\('quantic:\/\/favorites'\)/);
 assert.match(js,/item\.title\|\|host/);
 assert.doesNotMatch(js,/home-favorites-list.*innerHTML/);
});
test('slider sets opacity live and stores result, with safe lower bound',()=>{
 const persona=read('src/services/persona.cjs'),ui=read('src/renderer/renderer.js');
 const css=read('src/renderer/gekko-home-persona.css');
 assert.match(persona,/glassOpacity: 0\.22/);
 assert.match(persona,/clamp\(source\.glassOpacity, 0\.05, 0\.95/);
 assert.match(ui,/glass\.min='\.05'/);
 assert.match(ui,/glass\.oninput=\(\)=>/);
 assert.match(ui,/setProperty\('--quantic-glass-opacity',glass\.value\)/);
 assert.match(ui,/setSetting\('appearance',\{glassOpacity:Number\(glass\.value\)\}\)/);
 assert.match(css,/background:rgba\(9,23,29,var\(--quantic-glass-opacity,\.22\)\)!important/);
 assert.match(css,/#bottom-dock::before,#bottom-dock::after/);
 assert.match(css,/#bottom-dock \.nav-row/);
});
test('previous 72% opaque GEKKO default migrates once to 22% without overriding custom selections',()=>{
 const store=read('src/services/store.cjs');
 const persona=read('src/services/persona.cjs');
 assert.match(store,/oldAppearance\.glassMaterialVersion/);
 assert.match(store,/\.glassOpacity=\.22/);
 assert.match(store,/Math\.abs\(Number\(oldAppearance\.glassOpacity\)-\.72\)/);
 assert.match(persona,/glassMaterialVersion: 2/);
});
test('last-loaded Glass Corner CSS centers icons in all four bar locations',()=>{
 const html=read('src/renderer/index.html'),css=read('src/renderer/gekko-home-persona.css');
 assert.ok(html.indexOf('href="gekko-home-persona.css"')>html.indexOf('href="gekko-glass-wide.css"'));
 assert.match(css,/place-items:center!important/);
 assert.match(css,/#bottom-dock \.side-stage-icon/);
 assert.match(css,/#bottom-dock \.dock-control svg/);
 assert.match(css,/\.gekko-home-favorites-grid/);
 assert.match(css,/@media \(max-width:760px\)/);
});
