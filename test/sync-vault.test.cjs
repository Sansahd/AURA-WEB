const test = require('node:test');
const assert = require('node:assert/strict');

const { encryptSnapshot, decryptSnapshot, ALGORITHM, KDF } = require('../src/services/sync-vault.cjs');

test('GEKKO sync uses AES-256-GCM and scrypt', () => {
  assert.equal(ALGORITHM, 'aes-256-gcm');
  assert.equal(KDF, 'scrypt');
});

test('GEKKO encrypted sync round-trips without exposing plaintext', () => {
  const snapshot = {
    version: 1,
    favorites: [{ title: 'GEKKO', url: 'https://example.com' }],
    settings: { searchEngine: 'gekko' }
  };
  const encrypted = encryptSnapshot(snapshot, 'correct horse battery staple');
  assert.doesNotMatch(encrypted, /https:\/\/example\.com/);
  assert.deepEqual(decryptSnapshot(encrypted, 'correct horse battery staple'), snapshot);
});

test('GEKKO encrypted sync rejects the wrong secret', () => {
  const encrypted = encryptSnapshot({ version: 1 }, 'correct horse battery staple');
  assert.throws(() => decryptSnapshot(encrypted, 'wrong secret value'));
});

test('GEKKO sync refuses short passphrases', () => {
  assert.throws(() => encryptSnapshot({ version: 1 }, 'short'));
});
