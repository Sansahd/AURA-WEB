'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=(name)=>fs.readFileSync(path.join(__dirname,'..',name),'utf8');

test('selected engine always displays the matching offline SVG rather than a letter',()=>{
 const html=read('src/renderer/index.html');
 const ui=read('src/renderer/renderer.js');
 assert.match(html,/id="search-engine-mark-image"/);
 assert.doesNotMatch(html,/id="search-engine-mark-text"/);
 assert.match(ui,/const markSrc = key === 'gekko'/);
 assert.match(ui,/key === 'tor' \? 'torbrowser' : key/);
 assert.match(ui,/searchEngineMarkImage\.setAttribute\('src', markSrc\)/);
 assert.doesNotMatch(ui,/searchEngineMarkText\.textContent/);
 for(const name of ['duckduckgo','qwant','startpage','brave','searxng','torbrowser']){
   const icon=read('src/renderer/engine-icons/'+name+'.svg');
   assert.match(icon,/<svg/);
 }
});

test('redundant dock Discover/SideStage and rail chevrons are removed',()=>{
 const html=read('src/renderer/index.html');
 const ui=read('src/renderer/renderer.js');
 assert.doesNotMatch(html,/id="discover"/);
 assert.doesNotMatch(html,/id="sidestage"/);
 assert.doesNotMatch(html,/data-home-action="explore-more"/);
 assert.doesNotMatch(ui,/\$\('#discover'\)/);
 assert.doesNotMatch(ui,/const railToggle = el\(/);
 assert.doesNotMatch(ui,/const collapse = el\('button', 'side-stage-collapse'/);
 assert.match(ui,/sideStageRail\.append\(reveal\)/);
 assert.match(ui,/pinButton\.onclick = \(\) => fire\(window\.quantic\.toggleRailPin\(\)\)/);
 const css=read('src/renderer/gekko-24-layout.css');
 assert.match(css,/#sidestage-rail\.rail-minimized \.gekko-rail-reveal/);
});

test('wallpaper reaches the opaque GEKKO home scene and accent changes real chrome',()=>{
 const html=read('src/renderer/index.html');
 const ui=read('src/renderer/renderer.js');
 const css=read('src/renderer/gekko-24-layout.css');
 const persona=read('src/services/persona.cjs');
 assert.match(ui,/--quantic-wallpaper-image/);
 assert.match(ui,/--quantic-accent/);
 assert.match(ui,/persona-wallpaper-preview/);
 assert.match(html,/img-src 'self' data:/);
 assert.match(css,/body\.has-wallpaper \.gekko-home-scene/);
 assert.match(css,/var\(--quantic-wallpaper-image\)!important/);
 assert.match(css,/body\.has-wallpaper \.gekko-home-sky/);
 assert.match(css,/body\.has-wallpaper \.gekko-home-mountains/);
 assert.match(css,/\.persona-wallpaper-preview\{/);
 assert.match(css,/color-mix\(in srgb,var\(--quantic-accent\)/);
 assert.match(persona,/wallpaperVersion: Date\.now\(\)/);
});
