const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const main = fs.readFileSync(path.join(root, 'src', 'main.cjs'), 'utf8');
const { CHANNEL_VALIDATORS } = require('../src/security/ipc-firewall.cjs');

test('every ipcMain.handle channel has an IPC firewall contract', () => {
  const channels = [...main.matchAll(/ipcMain\.handle\(['"]([^'"]+)['"]/g)].map((match) => match[1]);
  const missing = channels.filter((channel) => typeof CHANNEL_VALIDATORS[channel] !== 'function');
  assert.deepEqual(missing, []);
});

test('AURA Career IPC contracts accept intended shapes and reject unsafe ones', () => {
  assert.equal(CHANNEL_VALIDATORS['career-status']([]), true);
  assert.equal(CHANNEL_VALIDATORS['career-status']([1]), false);

  assert.equal(CHANNEL_VALIDATORS['career-settings']([{}]), true);
  assert.equal(CHANNEL_VALIDATORS['career-settings']([{ autopilot: true, minScore: 80, maxOffers: 6, maxDaily: 12 }]), true);
  assert.equal(CHANNEL_VALIDATORS['career-settings']([{ minScore: 101 }]), false);
  assert.equal(CHANNEL_VALIDATORS['career-settings']([{ unknown: true }]), false);

  assert.equal(CHANNEL_VALIDATORS['career-run']([]), true);
  assert.equal(CHANNEL_VALIDATORS['career-run']([{}]), true);
  assert.equal(CHANNEL_VALIDATORS['career-run']([{ autoSubmit: false, minScore: 72, maxOffers: 5, maxDaily: 12 }]), true);
  assert.equal(CHANNEL_VALIDATORS['career-run']([{ autopilot: true }]), false);
  assert.equal(CHANNEL_VALIDATORS['career-run']([{ maxOffers: 1000 }]), false);

  for (const channel of ['career-stop', 'career-folder', 'career-open-folder', 'career-import']) {
    assert.equal(CHANNEL_VALIDATORS[channel]([]), true, channel);
    assert.equal(CHANNEL_VALIDATORS[channel](['unexpected']), false, channel);
  }
});


test('extension and sync IPC contracts reject malformed calls', () => {
  assert.equal(CHANNEL_VALIDATORS['extension-install']([]), true);
  assert.equal(CHANNEL_VALIDATORS['extension-install'](['unexpected']), false);

  assert.equal(CHANNEL_VALIDATORS['extension-remove'](['abcdefghijklmnopabcdefghijklmnop']), true);
  assert.equal(CHANNEL_VALIDATORS['extension-remove'](['']), false);
  assert.equal(CHANNEL_VALIDATORS['extension-remove']([42]), false);

  assert.equal(CHANNEL_VALIDATORS['sync-export'](['correct horse battery staple']), true);
  assert.equal(CHANNEL_VALIDATORS['sync-import'](['correct horse battery staple']), true);
  assert.equal(CHANNEL_VALIDATORS['sync-export'](['short']), false);
  assert.equal(CHANNEL_VALIDATORS['sync-import']([]), false);
});
