'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { EventEmitter } = require('node:events');
const read = (p) => fs.readFileSync(path.join(__dirname,'..',p),'utf8');

function makeManager() {
  class FakeWebContents extends EventEmitter {
    constructor() { super(); this.url=''; this.destroyed=false; }
    isDestroyed(){return this.destroyed}
    setWindowOpenHandler(fn){this.windowOpenHandler=fn}
    loadURL(url){this.url=url;return Promise.resolve()}
    close(){this.destroyed=true;this.emit('destroyed')}
  }
  class FakeWebContentsView {
    constructor(){this.webContents=new FakeWebContents();this.visible=false;this.bounds=null}
    setBackgroundColor(color){this.color=color}
    setVisible(value){this.visible=value}
    setBounds(value){this.bounds=value}
  }
  const module = {exports:{}};
  const source = read('src/services/sidestage-manager.cjs');
  const requireMock = (id) => {
    if (id === 'electron') return {WebContentsView:FakeWebContentsView,shell:{openExternal:()=>Promise.resolve()}};
    if (id === './sidestage.cjs') return require('../src/services/sidestage.cjs');
    throw Error('Unexpected dependency '+id);
  };
  const wrapper=vm.runInNewContext('(function(require,module,exports){\n'+source+'\n})',
    {setTimeout,clearTimeout,console,Buffer,URL,encodeURIComponent});
  wrapper(requireMock,module,module.exports);
  const win={isDestroyed:()=>false,contentView:{
    children:[],
    addChildView(view){this.children=this.children.filter(v=>v!==view);this.children.push(view)},
    removeChildView(view){this.children=this.children.filter(v=>v!==view)}
  }};
  const settings={enabled:true,open:false,collapsed:false,activeApp:'',width:420,
    pinnedApps:['youtube','quanticmail','quanticpulse']};
  const store={settings:()=>({sideStage:settings}),setSetting:(key,value)=>{
    assert.equal(key,'sideStage');Object.assign(settings,value);
  }};
  const manager=new module.exports.SideStageManager({
    store,session:{},getWindow:()=>win,openInMain:()=>{},onChange:()=>{}
  });
  return {manager,win,settings};
}

test('SOCIAL is the user-facing name but older profiles retain the persisted app id',()=>{
  const {SIDE_APPS,normalizeSideStage}=require('../src/services/sidestage.cjs');
  assert.equal(SIDE_APPS.quanticpulse.label,'SOCIAL');
  assert.equal(SIDE_APPS.quanticmail.label,'Quantic Mail');
  assert.equal(normalizeSideStage({activeApp:'quanticpulse'}).activeApp,'quanticpulse');
  const launcher=read('src/renderer/index.html');
  assert.match(launcher,/<strong>SOCIAL<\/strong>/);
  assert.doesNotMatch(launcher,/<strong>ZOON<\/strong>/);
});

test('Mail failure is visible even when local fallback page fires loading and stop events',()=>{
  const {manager}=makeManager();
  manager.action('toggle','quanticmail');
  const wc=manager.views.get('quanticmail').webContents;
  assert.equal(manager.status.get('quanticmail'),'loading');
  wc.emit('did-fail-load',{},-105,'ERR_NAME_NOT_RESOLVED','https://quanticmail.onrender.com/',true);
  assert.equal(manager.status.get('quanticmail'),'error');
  assert.match(wc.url,/^data:text\/html/);
  wc.emit('did-start-loading');
  wc.emit('did-stop-loading');
  assert.equal(manager.status.get('quanticmail'),'error');
  manager.action('toggle','quanticmail');
  assert.equal(manager.status.get('quanticmail'),'loading');
  assert.equal(wc.url,'https://quanticmail.onrender.com/');
  wc.emit('did-stop-loading');
  assert.equal(manager.status.get('quanticmail'),'ready');
  manager.destroyAll();
});

test('reader is the topmost native view, with no obsolete transparent focus overlay',()=>{
  const {manager,win}=makeManager();
  manager.action('toggle','youtube');
  const youtube=manager.views.get('youtube');
  const geometry={x:100,y:58,width:420,height:690,privateMode:false};
  manager.layout(geometry);
  assert.equal(youtube.visible,true);
  assert.equal(win.contentView.children.at(-1),youtube);
  assert.equal(manager.focusView,undefined);
  manager.action('toggle','quanticmail');
  manager.layout(geometry);
  assert.equal(youtube.visible,false);
  assert.equal(manager.views.get('quanticmail').visible,true);
  manager.action('close');
  manager.layout(geometry);
  assert.equal(manager.views.get('quanticmail').visible,false);
  manager.destroyAll();
});

test('focus belongs to normal webContents CSS and never blocks media viewer',()=>{
 const manager=read('src/services/sidestage-manager.cjs');
 const main=read('src/main.cjs');
 const renderer=read('src/renderer/renderer.js');
 const css=read('src/renderer/gekko-24-layout.css');
 assert.doesNotMatch(manager,/focusOverlay\(\)/);
 assert.match(main,/mediaFocus\.sync\(tab\?\.view\?\.webContents, focusEnabled\)/);
 assert.match(read('src/services/media-focus.cjs'),/desired\.insertCSS\(this\.style/);
 assert.match(read('src/services/media-focus.cjs'),/old\.removeInsertedCSS\(oldKey\)/);
 assert.match(main,/focusEnabled = Boolean/);
 assert.match(main,/setRailCollapsed\(false\)/);
 assert.match(main,/if \(stage\.open && !stage\.collapsed\) \{/);
 assert.match(renderer,/focus-reader-active/);
 assert.match(css,/body\.focus-reader-active #bottom-dock/);
 assert.match(css,/body\.focus-reader-active #sidestage-rail \.side-stage-app\.active/);
 assert.match(renderer,/button\.onauxclick = \(event\)/);
 assert.match(renderer,/window\.quantic\.togglePanelTab\(app\.url\)/);
});

