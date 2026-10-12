const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { resolveInput } = require('../src/core/navigation.cjs');
const { QuanticStore } = require('../src/services/store.cjs');

test('GEKKO routes searches to results, without an intermediate engine selector', () => {
  assert.equal(resolveInput('pourquoi le ciel est bleu').value, 'quantic://search?q=pourquoi%20le%20ciel%20est%20bleu');
  assert.equal(resolveInput('youtube').value, 'https://www.youtube.com/');
  assert.equal(resolveInput('https://www.wikipedia.org').type, 'url');
  assert.match(resolveInput('bonjour', 'qwant').value, /^https:\/\/www\.qwant\.com/);
  const main=fs.readFileSync(path.join(__dirname,'../src/main.cjs'),'utf8');
  assert.match(main,/searchPublicWeb\(query/); assert.doesNotMatch(main,/gekkoSearchTarget/);
  assert.doesNotMatch(main,/navigate-home-search/);
  const renderer=fs.readFileSync(path.join(__dirname,'../src/renderer/renderer.js'),'utf8');
  assert.match(renderer,/if \(maybeInstantLaunch\(value\)\) return;/);
});

test('old default DuckDuckGo becomes GEKKO while future explicit choices persist',async()=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'gekko-test-'));
  try {
    fs.writeFileSync(path.join(dir,'quantic-state.json'),JSON.stringify({
      settings:{searchEngine:'duckduckgo',compatibilityPolicyVersion:4}
    }));
    const store=new QuanticStore({getPath:()=>dir});
    assert.equal(store.settings().searchEngine,'gekko');
    store.setSetting('searchEngine','qwant');
    await store.flush();
    const restored=new QuanticStore({getPath:()=>dir});
    assert.equal(restored.settings().searchEngine,'qwant');
    assert.equal(restored.settings().searchEngineExplicit,true);
    await restored.flush();
  }finally{fs.rmSync(dir,{recursive:true,force:true})}
});

test('Persona remains dynamic and the official user logo stays in the homepage',()=>{
  const html=fs.readFileSync(path.join(__dirname,'../src/renderer/index.html'),'utf8');
  const css=fs.readFileSync(path.join(__dirname,'../src/renderer/gekko-241-fixes.css'),'utf8');
  assert.ok(html.indexOf('gekko-241-fixes.css')>html.indexOf('gekko-home-persona.css'));
  assert.match(css,/--quantic-glass-opacity/);
  assert.match(css,/--quantic-accent/);
  assert.match(html,/gekko-icon\.png/);
});