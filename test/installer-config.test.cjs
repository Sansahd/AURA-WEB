const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
const bootstrap = fs.readFileSync(path.join(root, 'src', 'bootstrap.cjs'), 'utf8');
const main = fs.readFileSync(path.join(root, 'src', 'main.cjs'), 'utf8');
const uiProtocol = fs.readFileSync(path.join(root, 'src', 'services', 'ui-protocol.cjs'), 'utf8');

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


test('GEKKO does not wait for Widevine before creating the browser', () => {
  const mainIndex = bootstrap.indexOf("require('./main.cjs')");
  const mediaIndex = bootstrap.indexOf('void prepareProtectedMedia()');
  assert.ok(mainIndex >= 0, 'main.cjs must be loaded');
  assert.ok(mediaIndex > mainIndex, 'protected-media init must run after browser startup');
  assert.doesNotMatch(bootstrap, /await\s+prepareProtectedMedia\(\)/);
});


test('GEKKO normal startup does not await proxy initialization before createWindow', () => {
  const readyIndex = main.indexOf('app.whenReady().then');
  const proxyIndex = main.indexOf('const normalProxyReady', readyIndex);
  const windowIndex = main.indexOf('createWindow();', proxyIndex);
  const startupSlice = main.slice(readyIndex, windowIndex);

  assert.ok(readyIndex >= 0 && proxyIndex > readyIndex && windowIndex > proxyIndex);
  assert.doesNotMatch(startupSlice, /await\s+applyDirectProxy\(normalSession\)/);
  assert.match(startupSlice, /applyDirectProxy\(normalSession\)\.catch/);
  assert.match(main, /if \(isPrivateMode\(\)\) \{\s*await applyFailClosedProxy\(privateSession\)/s);
});


test('GEKKO packages the internal shell as physical resources outside ASAR', () => {
  const resources = pkg.build.extraResources || [];
  const ui = resources.find((entry) => entry.to === 'quantic-ui');
  assert.ok(ui, 'quantic-ui extraResources entry is required');
  assert.equal(ui.from, 'src');
  assert.ok(ui.filter.includes('renderer/**/*'));
  assert.ok(ui.filter.includes('assets/**/*'));
});

test('GEKKO internal protocol uses Electron net.fetch and a packaged resource root', () => {
  assert.match(uiProtocol, /const \{ app, protocol, net \} = require\('electron'\)/);
  assert.match(uiProtocol, /pathToFileURL/);
  assert.match(uiProtocol, /net\.fetch\(pathToFileURL\(filePath\)\.toString\(\)\)/);
  assert.match(uiProtocol, /path\.join\(process\.resourcesPath, 'quantic-ui'\)/);
  assert.match(uiProtocol, /supportFetchAPI:\s*true/);
});

test('GEKKO probes its quantic-ui shell before creating the browser window', () => {
  const installIndex = main.indexOf('await installQuanticUiProtocol()');
  const probeIndex = main.indexOf('await verifyQuanticUiShell()');
  const windowIndex = main.indexOf('createWindow();', probeIndex);
  assert.ok(installIndex >= 0);
  assert.ok(probeIndex > installIndex);
  assert.ok(windowIndex > probeIndex);
});
