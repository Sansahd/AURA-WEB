const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const main = fs.readFileSync(path.join(root, 'src', 'main.cjs'), 'utf8');
const pkg = require('../package.json');
const {
  GekkoShields,
  CUSTOM_NETWORK_RULES,
  CUSTOM_COSMETIC_RULES,
  EASYLIST_URL,
  EASYPRIVACY_URL,
  CACHE_MAX_AGE_MS
} = require('../src/services/shields.cjs');

test('GEKKO Shields dependencies are pinned and resolvable', () => {
  assert.equal(pkg.dependencies['@ghostery/adblocker'], '2.18.2');
  assert.equal(pkg.dependencies['@ghostery/adblocker-electron'], '2.18.2');
  assert.ok(require.resolve('@ghostery/adblocker'));
  assert.ok(require.resolve('@ghostery/adblocker-electron'));
});

test('GEKKO Shields ships EasyList/EasyPrivacy sources plus custom YouTube rules', () => {
  assert.equal(EASYLIST_URL, 'https://easylist.to/easylist/easylist.txt');
  assert.equal(EASYPRIVACY_URL, 'https://easylist.to/easylist/easyprivacy.txt');
  assert.ok(CUSTOM_NETWORK_RULES.some((rule) => rule.includes('youtube.com/pagead')));
  assert.ok(CUSTOM_NETWORK_RULES.some((rule) => rule.includes('doubleclick.net')));
  assert.ok(CUSTOM_NETWORK_RULES.some((rule) => rule.startsWith('@@||challenges.cloudflare.com')));
  assert.ok(CUSTOM_COSMETIC_RULES.some((rule) => rule.includes('ytp-ad-module')));
  assert.ok(CUSTOM_COSMETIC_RULES.some((rule) => rule.includes('ytd-ad-slot-renderer')));
});

test('GEKKO Shields cache is local and expires for refresh', () => {
  assert.equal(CACHE_MAX_AGE_MS, 72 * 60 * 60 * 1000);
  const fakeApp = { getPath: () => '/tmp/gekko-test' };
  const shields = new GekkoShields(fakeApp);
  const snap = shields.snapshot();
  assert.equal(snap.status, 'idle');
  assert.equal(snap.engine, 'ghostery-adblocker');
  assert.equal(snap.source, 'fallback');
});

test('normal browsing wires the full Shields engine while retaining local fallback', () => {
  assert.match(main, /const \{ GekkoShields \} = require\('\.\/services\/shields\.cjs'\)/);
  assert.match(main, /shields = new GekkoShields\(app/);
  assert.match(main, /shields\.install\(normalSession\)/);
  assert.match(main, /installPrivacyLayer\(targetSession\)/);
  assert.match(main, /shields: shields\?\.snapshot/);
});

test('private profile does not enable a second Ghostery Electron blocker instance', () => {
  assert.doesNotMatch(main, /shields\.install\(privateSession\)/);
});

test('tab lifecycle budget is tightened for Edge-like efficiency', () => {
  assert.match(main, /TAB_LIFECYCLE_SWEEP_MS = 30_000/);
  assert.match(main, /TAB_IDLE_SLEEP_MS = 8 \* 60_000/);
  assert.match(main, /TAB_PRESSURE_IDLE_MS = 2 \* 60_000/);
  assert.match(main, /MAX_LIVE_BACKGROUND_TABS = 4/);
});
