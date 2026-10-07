const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const source = fs.readFileSync(path.join(__dirname, '..', 'src', 'services', 'extensions.cjs'), 'utf8');
const main = fs.readFileSync(path.join(__dirname, '..', 'src', 'main.cjs'), 'utf8');

test('GEKKO uses the Electron 44 extensions namespace', () => {
  assert.match(source, /this\.session\?\.extensions/);
  assert.match(source, /api\?\.loadExtension/);
  assert.match(source, /api\?\.removeExtension/);
  assert.match(source, /api\.getAllExtensions/);
  assert.doesNotMatch(source, /session\.loadExtension/);
});

test('GEKKO reloads configured local extensions at startup', () => {
  assert.match(source, /async restore\(\)/);
  assert.match(source, /extensionPaths/);
  assert.match(main, /await extensionManager\.restore\(\)/);
});

test('GEKKO never loads local extensions into private in-memory sessions', () => {
  assert.match(main, /isPrivateMode\(\).*extensions_unavailable_in_private_mode/);
});
