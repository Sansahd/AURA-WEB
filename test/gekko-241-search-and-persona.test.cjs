const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { resolveHomeSearch } = require('../src/core/navigation.cjs');
const { QuanticStore } = require('../src/services/store.cjs');

test('homepage searches aliases as GEKKO Search, never as browsers or sites', () => {
  assert.deepEqual(resolveHomeSearch('firefox'), {
    type: 'search', engine: 'gekko', value: 'quantic://search?q=firefox', requiresTor: false
  });
  assert.equal(resolveHomeSearch('google').value, 'quantic://search?q=google');
  assert.equal(resolveHomeSearch('https://www.wikipedia.org').type, 'url');
  assert.match(resolveHomeSearch('bonjour', 'qwant').value, /^https:\/\/www\.qwant\.com/);
});

test('legacy implicitly DuckDuckGo profile returns to GEKKO and explicit choice persists', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gekko-persona-'));
  try {
    fs.writeFileSync(path.join(dir, 'quantic-state.json'), JSON.stringify({
      settings: { searchEngine: 'duckduckgo', compatibilityPolicyVersion: 4 }
    }));
    const store = new QuanticStore({ getPath: () => dir });
    assert.equal(store.settings().searchEngine, 'gekko');
    store.setSetting('searchEngine', 'qwant');
    await store.flush();
    const restored = new QuanticStore({ getPath: () => dir });
    assert.equal(restored.settings().searchEngine, 'qwant');
    assert.equal(restored.settings().searchEngineExplicit, true);
    await restored.flush();
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('latest Persona stylesheet is wired after earlier rules and uses live settings', () => {
  const html = fs.readFileSync(path.join(__dirname,'../src/renderer/index.html'),'utf8');
  const css = fs.readFileSync(path.join(__dirname,'../src/renderer/gekko-241-fixes.css'),'utf8');
  assert.ok(html.indexOf('gekko-241-fixes.css') > html.indexOf('gekko-home-persona.css'));
  assert.match(css,/--quantic-glass-opacity/);
  assert.match(css,/--quantic-accent/);
  assert.match(html,/gekko-home-symbol\.svg/);
});
