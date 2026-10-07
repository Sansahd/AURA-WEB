const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const { cookieConsentRejectSource } = require('../src/services/cookie-consent.cjs');
const main = fs.readFileSync(path.join(root, 'src', 'main.cjs'), 'utf8');

test('cookie guard looks only for refusal/necessary-cookie actions', () => {
  const source = cookieConsentRejectSource();
  assert.match(source, /tout refuser/);
  assert.match(source, /reject all/);
  assert.match(source, /only necessary/);
  assert.match(source, /onetrust-reject-all-handler/);
  assert.match(source, /CybotCookiebotDialogBodyButtonDecline/);
  assert.match(source, /didomi-notice-disagree-button/);
  assert.match(source, /globalPrivacyControl/);
  assert.doesNotMatch(source, /accept all|tout accepter/i);
});

test('cookie guard is installed on initial documents and later navigations', () => {
  const hits = main.match(/installCookieConsentRefusal\(wc\)/g) || [];
  assert.ok(hits.length >= 2);
});
