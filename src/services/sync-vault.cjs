'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');

const FORMAT = 'gekko-sync-v1';
const KDF = 'scrypt';
const ALGORITHM = 'aes-256-gcm';

function deriveKey(passphrase, salt) {
  const secret = String(passphrase || '');
  if (secret.length < 8) throw new Error('La phrase secrète doit contenir au moins 8 caractères');
  return crypto.scryptSync(secret, salt, 32, { N: 16384, r: 8, p: 1, maxmem: 64 * 1024 * 1024 });
}

function encryptSnapshot(snapshot, passphrase) {
  const salt = crypto.randomBytes(16);
  const iv = crypto.randomBytes(12);
  const key = deriveKey(passphrase, salt);
  const cipher = crypto.createCipheriv(ALGORITHM, key, iv, { authTagLength: 16 });
  const plaintext = Buffer.from(JSON.stringify(snapshot), 'utf8');
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  const tag = cipher.getAuthTag();

  return JSON.stringify({
    format: FORMAT,
    version: 1,
    cipher: ALGORITHM,
    kdf: KDF,
    salt: salt.toString('base64'),
    iv: iv.toString('base64'),
    tag: tag.toString('base64'),
    data: ciphertext.toString('base64')
  }, null, 2);
}

function decryptSnapshot(payload, passphrase) {
  const envelope = typeof payload === 'string' ? JSON.parse(payload) : payload;
  if (!envelope || envelope.format !== FORMAT || envelope.version !== 1) {
    throw new Error('Fichier de synchronisation GEKKO invalide');
  }
  if (envelope.cipher !== ALGORITHM || envelope.kdf !== KDF) {
    throw new Error('Format cryptographique non pris en charge');
  }

  const salt = Buffer.from(envelope.salt || '', 'base64');
  const iv = Buffer.from(envelope.iv || '', 'base64');
  const tag = Buffer.from(envelope.tag || '', 'base64');
  const encrypted = Buffer.from(envelope.data || '', 'base64');
  if (salt.length !== 16 || iv.length !== 12 || tag.length !== 16 || !encrypted.length) {
    throw new Error('Fichier de synchronisation corrompu');
  }

  const key = deriveKey(passphrase, salt);
  const decipher = crypto.createDecipheriv(ALGORITHM, key, iv, { authTagLength: 16 });
  decipher.setAuthTag(tag);
  const plaintext = Buffer.concat([decipher.update(encrypted), decipher.final()]).toString('utf8');
  return JSON.parse(plaintext);
}

function writeEncryptedFile(filePath, snapshot, passphrase) {
  fs.writeFileSync(filePath, encryptSnapshot(snapshot, passphrase), { encoding: 'utf8', mode: 0o600 });
}

function readEncryptedFile(filePath, passphrase) {
  return decryptSnapshot(fs.readFileSync(filePath, 'utf8'), passphrase);
}

module.exports = {
  FORMAT,
  KDF,
  ALGORITHM,
  encryptSnapshot,
  decryptSnapshot,
  writeEncryptedFile,
  readEncryptedFile
};
