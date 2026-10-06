const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
const bootstrap = fs.readFileSync(path.join(root, 'src', 'bootstrap.cjs'), 'utf8');

test('GEKKO installer has its own Windows identity', () => {
  assert.equal(pkg.build.appId, 'com.quantic.gekko.browser');
  assert.doesNotMatch(pkg.build.appId, /glide/i);
  assert.equal(pkg.build.productName, 'GEKKO Browser');
});

test('GEKKO installer is assisted, visible and launches GEKKO after finish', () => {
  assert.equal(pkg.build.nsis.oneClick, false);
  assert.equal(pkg.build.nsis.allowToChangeInstallationDirectory, true);
  assert.equal(pkg.build.nsis.allowElevation, true);
  assert.equal(pkg.build.nsis.perMachine, false);
  assert.equal(pkg.build.nsis.runAfterFinish, true);
  assert.equal(pkg.build.nsis.artifactName, 'GEKKO-Setup.${ext}');
});

test('GEKKO ships a portable Windows executable', () => {
  const targets = pkg.build.win.target.map((entry) => entry.target);
  assert.ok(targets.includes('nsis'));
  assert.ok(targets.includes('portable'));
  assert.equal(pkg.build.portable.artifactName, 'GEKKO-Portable.${ext}');
});

test('GEKKO does not wait for Widevine before creating the browser', () => {
  const mainIndex = bootstrap.indexOf("require('./main.cjs')");
  const mediaIndex = bootstrap.indexOf('void prepareProtectedMedia()');
  assert.ok(mainIndex >= 0, 'main.cjs must be loaded');
  assert.ok(mediaIndex > mainIndex, 'protected-media init must run after browser startup');
  assert.doesNotMatch(bootstrap, /await\s+prepareProtectedMedia\(\)/);
});
