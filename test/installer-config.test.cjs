const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));

test('GEKKO installer has its own Windows identity', () => {
  assert.equal(pkg.build.appId, 'com.quantic.gekko.browser');
  assert.doesNotMatch(pkg.build.appId, /glide/i);
  assert.equal(pkg.build.productName, 'GEKKO Browser');
});

test('GEKKO installer is assisted and user-recoverable', () => {
  assert.equal(pkg.build.nsis.oneClick, false);
  assert.equal(pkg.build.nsis.allowToChangeInstallationDirectory, true);
  assert.equal(pkg.build.nsis.perMachine, false);
  assert.equal(pkg.build.nsis.runAfterFinish, false);
});
