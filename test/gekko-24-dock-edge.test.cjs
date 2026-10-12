'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
test('immersive rail reveals on the exact chosen edge rather than bottom only',()=>{
 const main=read('src/main.cjs');
 assert.match(main,/function nearDockEdge\(pointer, bounds, position/);
 for(const edge of ['top','bottom','left','right'])
   assert.match(main,new RegExp("position === '"+edge+"'"));
 assert.match(main,/const position = store\?\.settings\(\)\.railPosition \|\| 'bottom';/);
 assert.match(main,/const reveal = nearDockEdge\(pointer, bounds, position\)/);
 assert.match(main,/if \(reveal\) showChrome\(false\)/);
 assert.match(main,/SIDE_DOCK_W \+ 36/);
 assert.doesNotMatch(main,/if \(!chromeVisible && nearBottom\)/);
});
test('scrollbars hidden only in navigation and styled subtly in readable panels',()=>{
 const css=read('src/renderer/gekko-home-persona.css');
 assert.match(css,/#bottom-dock \.tabs::\-webkit-scrollbar/);
 assert.match(css,/scrollbar-width:none!important/);
 assert.match(css,/scrollbar-color:rgba\(167,251,229,\.24\) transparent/);
 assert.match(css,/border-radius:100px/);
});
