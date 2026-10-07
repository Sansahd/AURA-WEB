const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const { resolveInput, normalizeEngine } = require('../src/core/navigation.cjs');
const { isGoogleConsentUrl, googleConsentRefusalSource } = require('../src/services/google-consent.cjs');
const { CHANNEL_VALIDATORS } = require('../src/security/ipc-firewall.cjs');

const main = fs.readFileSync(path.join(root, 'src', 'main.cjs'), 'utf8');
const store = fs.readFileSync(path.join(root, 'src', 'services', 'store.cjs'), 'utf8');

test('omnibox searches go directly to the persistent engine', () => {
  assert.equal(normalizeEngine('duckduckgo'), 'duckduckgo');
  assert.equal(normalizeEngine('qwant'), 'qwant');
  assert.equal(normalizeEngine('quantic'), 'duckduckgo');
  assert.equal(normalizeEngine('brave'), 'duckduckgo');

  assert.equal(
    resolveInput('navigateur privé', 'duckduckgo').value,
    'https://duckduckgo.com/?q=navigateur%20priv%C3%A9'
  );
  assert.equal(
    resolveInput('navigateur privé', 'qwant').value,
    'https://www.qwant.com/?q=navigateur%20priv%C3%A9&t=web'
  );
});

test('search engine IPC accepts only DuckDuckGo and Qwant', () => {
  const validate = CHANNEL_VALIDATORS['set-setting'];
  assert.equal(validate(['searchEngine', 'duckduckgo']), true);
  assert.equal(validate(['searchEngine', 'qwant']), true);
  assert.equal(validate(['searchEngine', 'quantic']), false);
  assert.equal(validate(['searchEngine', 'brave']), false);
});

test('legacy search settings migrate to DuckDuckGo', () => {
  assert.match(store, /searchEngine: 'duckduckgo'/);
  assert.match(store, /compatibilityPolicyVersion: 3/);
  assert.match(store, /!\['duckduckgo', 'qwant'\]\.includes\(existingSettings\.searchEngine\)/);
});

test('Google consent guard targets Google only and contains explicit reject actions', () => {
  assert.equal(isGoogleConsentUrl('https://www.google.com/search?q=test'), true);
  assert.equal(isGoogleConsentUrl('https://consent.google.com/m?continue=x'), true);
  assert.equal(isGoogleConsentUrl('https://www.google.fr/'), true);
  assert.equal(isGoogleConsentUrl('https://google.evil.example/'), false);
  assert.equal(isGoogleConsentUrl('https://duckduckgo.com/'), false);

  const source = googleConsentRefusalSource(900);
  assert.match(source, /tout refuser/);
  assert.match(source, /reject all/);
  assert.match(source, /MutationObserver/);
  assert.match(source, /node\.click\(\)/);
  assert.match(source, /Date\.now\(\) \+ 900/);
});

test('Google rejection runs before external page reveal', () => {
  const domReady = main.slice(
    main.indexOf("wc.on('dom-ready'"),
    main.indexOf("wc.on('did-stop-loading'")
  );
  const rejectIndex = domReady.indexOf('await installGoogleConsentRefusal(wc)');
  const revealIndex = domReady.indexOf('revealTabView(tab)');
  assert.ok(rejectIndex >= 0, 'Google consent rejection must run on dom-ready');
  assert.ok(revealIndex > rejectIndex, 'Google rejection must run before page reveal');
});

test('navigation transition completes instead of remaining permanently active', () => {
  assert.match(main, /MIN_NAV_TRANSITION_MS = 480/);
  assert.match(main, /tab\.transitionStartedAt = Date\.now\(\)/);
  assert.match(main, /tab\.transitioning = false/);
  assert.match(main, /finishRevealTabView/);
});
