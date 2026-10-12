'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path');
const read=p=>fs.readFileSync(path.join(__dirname,'..',p),'utf8');
test('real edge hit testing never reveals the dock from the opposite border',()=>{
 const vm=require('node:vm');
 const main=read('src/main.cjs');
 const begin=main.indexOf('function nearDockEdge(');
 const end=main.indexOf('\nfunction startImmersionWatcher()',begin);
 assert.ok(begin>=0 && end>begin,'dock edge helper must exist');
 const near=vm.runInNewContext(main.slice(begin,end)+'\nnearDockEdge',{EDGE_TRIGGER:12});
 const box={x:100,y:200,width:1200,height:700};
 const p=(x,y)=>({x,y});
 const tests=[
  ['top',p(700,202),p(700,896)],
  ['bottom',p(700,898),p(700,202)],
  ['left',p(102,530),p(1298,530)],
  ['right',p(1298,530),p(102,530)]
 ];
 for(const [edge,correct,wrong] of tests){
   assert.equal(near(correct,box,edge),true,'must reveal '+edge+' on its own edge');
   assert.equal(near(wrong,box,edge),false,'must not reveal '+edge+' on opposite edge');
 }
 assert.equal(near(p(99,201),box,'left'),false,'outside window cannot reveal');
 assert.equal(near(p(700,500),box,'bottom'),false,'center cannot reveal');
});
test('viewport scrollbar is discreet without altering third-party widgets or forced-colors accessibility',()=>{
 const preload=read('src/web-preload.cjs');
 assert.match(preload,/function installGekkoViewportScrollbar/);
 assert.match(preload,/scrollbar-width: thin/);
 assert.match(preload,/forced-colors: active/);
 assert.match(preload,/contextBridge\.executeInMainWorld\(\{ func: installGekkoViewportScrollbar \}\)/);
});
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
