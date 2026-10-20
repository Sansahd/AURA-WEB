'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');

test('only one meaningful new-tab navigation, no redundant homepage marketing tiles',()=>{
 const html=read('src/renderer/index.html');
 const js=read('src/renderer/renderer.js');
 assert.match(html,/id="home-search-form"/);
 assert.match(html,/id="home-favorites-list"/);
 assert.doesNotMatch(html,/class="gekko-home-tools"/);
 assert.doesNotMatch(html,/class="gekko-home-cards"/);
 assert.doesNotMatch(html,/data-home-action=/);
 assert.doesNotMatch(js,/\$\('#home-tool-home'\)\.onclick/);
});

test('reload and stop SVGs never render together, including while resizing',()=>{
 const html=read('src/renderer/index.html');
 const css=read('src/renderer/gekko-home-persona.css');
 const js=read('src/renderer/renderer.js');
 assert.match(html,/id="reload"/);
 assert.match(html,/class="icon-reload"/);
 assert.match(html,/class="icon-stop"/);
 assert.match(css,/#bottom-dock #reload:not\(\.loading\) \.icon-stop\{[\s\S]*?display:none!important/);
 assert.match(css,/#bottom-dock #reload\.loading \.icon-reload\{[\s\S]*?display:none!important/);
 assert.match(css,/#bottom-dock #reload\.loading \.icon-stop\{/);
 assert.match(js,/reload\.classList\.toggle\('loading', Boolean\(tab\?\.loading\)\)/);
});

test('branded Windows binary and all shortcuts use canonical GEKKO artwork',()=>{
 const pkg=JSON.parse(read('package.json'));
 const main=read('src/main.cjs');
 const mark=read('src/assets/quantic-glide-mark.svg');
 const icon=read('src/assets/quantic-glide-icon.svg');
 assert.equal(pkg.build.win.icon,'build/gekko.ico');
 assert.equal(pkg.build.nsis.installerIcon,'build/gekko.ico');
 assert.equal(pkg.build.nsis.uninstallerIcon,'build/gekko.ico');
 assert.equal(pkg.build.appId,'com.quantic.gekko.browser');
 assert.match(pkg.scripts.dist,/build:icons/);
 assert.match(pkg.devDependencies.sharp,/\^0\.34\./);
 assert.match(main,/icon: path\.join\(__dirname, 'assets', 'gekko-icon\.png'\)/);
 assert.match(main,/app\.setAppUserModelId\('com\.quantic\.gekko\.browser'\)/);
 for(const source of [mark,icon]){
  assert.match(source,/id="gecko"/);
  assert.match(source,/id="earth"/);
  assert.match(source,/class="toe"/);
  assert.match(source,/class="continent"/);
 }
 const generated=path.join(__dirname,'..','build','gekko.ico');
 assert.ok(fs.existsSync(generated),'multi-resolution ICO must be generated before testing');
 const ico=fs.readFileSync(generated);
 assert.equal(ico.readUInt16LE(2),1);
 assert.ok(ico.readUInt16LE(4)>=5);
});

test('native window and web view have matching dark resize backing',()=>{
 const main=read('src/main.cjs');
 assert.match(main,/backgroundColor: '#050505'/);
 assert.match(main,/view\.setBackgroundColor\('#050505'\)/);
 assert.match(main,/win\.on\('resize', \(\) => \{/);
 assert.match(main,/win\.setBackgroundColor\('#050505'\)/);
 assert.match(main,/layout\(\)/);
});

test('user-uploaded transparent gecko globe is used for homepage and Windows icons',()=>{
 const html=read('src/renderer/index.html');
 const script=read('scripts/build-gekko-icons.cjs');
 const raw=read('src/assets/gekko-user-logo.webp.b64').trim();
 const source=Buffer.from(raw,'base64');
 assert.match(html,/src="\.\.\/assets\/gekko-icon\.png"/);
 assert.doesNotMatch(html,/src="\.\.\/assets\/gekko-home-symbol\.svg"/);
 assert.equal(source.toString('ascii',0,4),'RIFF');
 assert.equal(source.toString('ascii',8,12),'WEBP');
 assert.equal(source.length,source.readUInt32LE(4)+8);
 assert.ok(source.length>4000);
 assert.match(script,/gekko-user-logo\.webp\.b64/);
 assert.match(script,/sharp\(artwork\)\.resize\(size,size/);
 const out=path.join(__dirname,'..','src','assets','gekko-icon.png');
 assert.ok(fs.existsSync(out));
 assert.deepEqual(Array.from(fs.readFileSync(out).subarray(0,8)),
   [137,80,78,71,13,10,26,10]);
});
