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

test('omnibox exposes GEKKO plus the extended persistent search palette', () => {
  for (const engine of ['gekko', 'duckduckgo', 'qwant', 'startpage', 'brave', 'searxng', 'tor']) {
    assert.equal(normalizeEngine(engine), engine);
  }
  assert.equal(normalizeEngine('unknown-engine'), 'gekko');

  const query = 'navigateur privé';
  assert.equal(resolveInput(query, 'gekko').value, 'quantic://search?q=navigateur%20priv%C3%A9');
  assert.equal(resolveInput(query, 'duckduckgo').value, 'https://duckduckgo.com/?q=navigateur%20priv%C3%A9');
  assert.equal(resolveInput(query, 'qwant').value, 'https://www.qwant.com/?q=navigateur%20priv%C3%A9&t=web');
  assert.equal(resolveInput(query, 'startpage').value, 'https://www.startpage.com/sp/search?query=navigateur%20priv%C3%A9');
  assert.equal(resolveInput(query, 'brave').value, 'https://search.brave.com/search?q=navigateur%20priv%C3%A9');
  assert.equal(resolveInput(query, 'searxng').value, 'https://searxng.website/search?q=navigateur%20priv%C3%A9');

  const tor = resolveInput(query, 'tor');
  assert.equal(tor.value, 'https://duckduckgo.com/?q=navigateur%20priv%C3%A9');
  assert.equal(tor.requiresTor, true);
});

test('search engine IPC accepts the complete palette and rejects unknown engines', () => {
  const validate = CHANNEL_VALIDATORS['set-setting'];
  for (const engine of ['gekko', 'duckduckgo', 'qwant', 'startpage', 'brave', 'searxng', 'tor']) {
    assert.equal(validate(['searchEngine', engine]), true, engine);
  }
  assert.equal(validate(['searchEngine', 'google']), false);
  assert.equal(validate(['searchEngine', 'unknown']), false);
});

test('new installs default to GEKKO while old valid choices are preserved', () => {
  assert.match(store, /searchEngine: 'gekko'/);
  assert.match(store, /compatibilityPolicyVersion: 4/);
  assert.match(store, /existingSettings\.searchEngine === 'quantic'/);
  assert.match(store, /'startpage', 'brave', 'searxng', 'tor'/);
});

test('Tor search route enters private mode before loading the query', () => {
  assert.match(main, /target\.requiresTor && !isPrivateMode\(\)/);
  assert.match(main, /await setNetworkMode\('private'\)/);
  assert.match(main, /return loadTab\(privateTab, raw\)/);
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

test('Google consent rejection still runs on dom-ready without hiding normal pages', () => {
  const domReady = main.slice(
    main.indexOf("wc.on('dom-ready'"),
    main.indexOf("wc.on('did-stop-loading'")
  );
  assert.match(domReady, /await installGoogleConsentRefusal\(wc\)/);
  assert.doesNotMatch(domReady, /installPageFadeIn|revealTabView/);
});





test('normal navigation never hides the WebContentsView behind a transition', () => {
  assert.doesNotMatch(main, /preparePageFadeOut|installPageFadeIn|releasePageFadeIn/);
  assert.doesNotMatch(main, /tab\.transitioning = true/);
  assert.doesNotMatch(main, /view\.setVisible\(false\);\s*\n\s*const ok = await ensureNetwork/);
});


test('generic cookie refusal complements Google-specific consent handling', () => {
  assert.match(main, /installCookieConsentRefusal/);
  assert.match(main, /installGoogleConsentRefusal/);
  const domReady = main.slice(
    main.indexOf("wc.on('dom-ready'"),
    main.indexOf("wc.on('did-stop-loading'")
  );
  assert.match(domReady, /await installCookieConsentRefusal\(wc\)/);
});
