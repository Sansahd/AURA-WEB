'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
test('Windows and homepage icons are generated from the selected monochrome origami artwork',()=>{
 const script=read('scripts/build-gekko-icons.cjs'),svg=read('src/assets/gekko-origami-noir.svg'),html=read('src/renderer/index.html');
 assert.match(script,/fs\.existsSync\(noirArtwork\) \? fs\.readFileSync\(noirArtwork\)/);
 assert.match(script,/sharp\(artwork\)\.resize\(size,size/);
 assert.match(svg,/<circle cx="256" cy="256" r="250" fill="#000"\/>/);
 assert.match(svg,/fill="#fff"/);
 assert.doesNotMatch(svg,/<linearGradient|filter=|rgba/);
 assert.match(html,/src="\.\.\/assets\/gekko-icon\.png"/);
});
test('one GEKKO NOIR stylesheet overrides all previous chrome skins',()=>{
 const html=read('src/renderer/index.html'),css=read('src/renderer/gekko-noir.css');
 assert.ok(html.indexOf('gekko-noir.css')>html.indexOf('gekko-24-ergonomics.css'));
 assert.match(css,/html body\[data-rail-position\] #bottom-dock/);
 assert.match(css,/background:#050505!important/);
 assert.match(css,/\.gekko-home-aurora/);
 assert.match(css,/\.gk25-title/);
 assert.match(css,/\.gekko-rail-window-controls button/);
 const main=read('src/main.cjs');
 assert.match(main,/backgroundColor: '#050505'/);
 assert.match(main,/view\.setBackgroundColor\('#050505'\)/);
});
test('mono default preserves explicit custom accent and supports one-time old profile migration',()=>{
 const persona=read('src/services/persona.cjs'),store=read('src/services/store.cjs');
 assert.match(persona,/accent: '#ffffff'/);
 assert.match(persona,/paletteVersion: 1/);
 assert.match(store,/if \(wasOldDefault\) this\.data\.settings\.appearance\.accent = '#ffffff'/);
 assert.match(store,/this\.data\.settings\.appearance\.paletteVersion = 1/);
});
