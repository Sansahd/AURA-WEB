const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const preload = fs.readFileSync(path.join(__dirname, '..', 'src', 'web-preload.cjs'), 'utf8');

test('page preload exposes global privacy control and DNT', () => {
  assert.match(preload, /globalPrivacyControl/);
  assert.match(preload, /doNotTrack/);
  assert.match(preload, /hardwareConcurrency/);
  assert.match(preload, /deviceMemory/);
});

test('page preload standardizes high entropy UA and WebGL identity', () => {
  assert.match(preload, /getHighEntropyValues/);
  assert.match(preload, /platformVersion/);
  assert.match(preload, /WEBGL_debug_renderer_info/);
  assert.match(preload, /ANGLE \(Generic GPU\)/);
});

test('page preload does not disable WebGL capabilities wholesale', () => {
  assert.doesNotMatch(preload, /disableWebGL/);
  assert.doesNotMatch(preload, /WebGLRenderingContext\s*=\s*undefined/);
});
