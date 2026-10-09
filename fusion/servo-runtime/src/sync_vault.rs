use std::fs;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use getrandom::getrandom;
use scrypt::{scrypt, Params};
use serde::{Deserialize, Serialize};

const FORMAT: &str = "gekko-native-sync-v1";
const CIPHER: &str = "aes-256-gcm";
const KDF: &str = "scrypt";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    format: String,
    version: u8,
    cipher: String,
    kdf: String,
    salt: String,
    nonce: String,
    data: String,
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    if passphrase.chars().count() < 8 {
        return Err("La phrase secrète doit contenir au moins 8 caractères".into());
    }
    let params = Params::new(14, 8, 1, 32).map_err(|e| format!("scrypt params: {e:?}"))?;
    let mut key = [0_u8; 32];
    scrypt(passphrase.as_bytes(), salt, &params, &mut key)
        .map_err(|e| format!("scrypt: {e:?}"))?;
    Ok(key)
}

pub fn encrypt_bytes(plain: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    getrandom(&mut salt).map_err(|e| format!("random salt: {e}"))?;
    getrandom(&mut nonce).map_err(|e| format!("random nonce: {e}"))?;

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher init: {e:?}"))?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|e| format!("encryption: {e:?}"))?;

    let envelope = Envelope {
        format: FORMAT.into(),
        version: 1,
        cipher: CIPHER.into(),
        kdf: KDF.into(),
        salt: B64.encode(salt),
        nonce: B64.encode(nonce),
        data: B64.encode(encrypted),
    };
    serde_json::to_vec_pretty(&envelope).map_err(|e| format!("sync envelope: {e}"))
}

pub fn decrypt_bytes(payload: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let envelope: Envelope =
        serde_json::from_slice(payload).map_err(|_| "Fichier de synchronisation invalide".to_string())?;
    if envelope.format != FORMAT || envelope.version != 1 || envelope.cipher != CIPHER || envelope.kdf != KDF {
        return Err("Format de synchronisation GEKKO non pris en charge".into());
    }

    let salt = B64.decode(envelope.salt).map_err(|_| "Sel invalide".to_string())?;
    let nonce = B64.decode(envelope.nonce).map_err(|_| "Nonce invalide".to_string())?;
    let encrypted = B64.decode(envelope.data).map_err(|_| "Données chiffrées invalides".to_string())?;
    if salt.len() != SALT_LEN || nonce.len() != NONCE_LEN || encrypted.is_empty() {
        return Err("Fichier de synchronisation corrompu".into());
    }

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher init: {e:?}"))?;
    cipher
        .decrypt(Nonce::from_slice(&nonce), encrypted.as_ref())
        .map_err(|_| "Phrase secrète incorrecte ou fichier corrompu".into())
}

pub fn write_file(path: &Path, plain: &[u8], passphrase: &str) -> Result<(), String> {
    let payload = encrypt_bytes(plain, passphrase)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Création du dossier: {e}"))?;
    }
    fs::write(path, payload).map_err(|e| format!("Écriture sync: {e}"))
}

pub fn read_file(path: &Path, passphrase: &str) -> Result<Vec<u8>, String> {
    let payload = fs::read(path).map_err(|e| format!("Lecture sync: {e}"))?;
    decrypt_bytes(&payload, passphrase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_encrypted() {
        let plain = br#"{"favorites":["https://example.com"]}"#;
        let payload = encrypt_bytes(plain, "correct horse battery staple").unwrap();
        assert!(!String::from_utf8_lossy(&payload).contains("https://example.com"));
        assert_eq!(decrypt_bytes(&payload, "correct horse battery staple").unwrap(), plain);
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let payload = encrypt_bytes(b"gekko", "correct horse battery staple").unwrap();
        assert!(decrypt_bytes(&payload, "incorrect secret").is_err());
    }

    #[test]
    fn short_secret_is_rejected() {
        assert!(encrypt_bytes(b"gekko", "short").is_err());
    }
}
