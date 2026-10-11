//! Encryption — port of `@subtrack/lib/crypto.ts`.
//! AES-256-GCM with 16-byte IV; ciphertext layout is `iv(16) + tag(16) + ct`.

// NOTE: the TS implementation uses a 16-byte IV, which is larger than the
// GCM default 12-byte nonce — so we must use AesGcm<Aes256, U16>, NOT the
// Aes256Gcm alias (which would panic on a 16-byte Nonce::from_slice).
use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use aes_gcm::{AesGcm, Key, Nonce};
use sha2::{Digest, Sha256};

type Aes256Gcm = AesGcm<Aes256, U16>;

pub const IV_LENGTH: usize = 16;
pub const TAG_LENGTH: usize = 16;
pub const KEY_LENGTH: usize = 32;
pub const SALT_LENGTH: usize = 32;
pub const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
pub const CONFIG_MAGIC: &[u8] = b"SUBCCFG\0\0\0\0\0\0\0\0";

/// DB/config directory: SUBSC_CLI_DB_DIR override, else ~/.config/subtrack.
pub fn config_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("SUBSC_CLI_DB_DIR") {
        return std::path::PathBuf::from(dir)
    }
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".config").join("subtrack")
}

/// $HOME (scanner paths are resolved relative to it).
pub fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// True when an encryption key is available (passphrase or `.key` file).
pub fn has_encryption_key() -> bool {
    std::env::var("SUBSC_CLI_DB_PASSPHRASE").is_ok() || config_dir().join(".key").exists()
}

/// Raw encryption key: from the passphrase via scrypt, else from `.key`.
pub fn get_or_create_key() -> std::io::Result<Vec<u8>> {
    if let Ok(passphrase) = std::env::var("SUBSC_CLI_DB_PASSPHRASE") {
        return Ok(derive_key_from_passphrase(&passphrase))
    }
    let key_path = config_dir().join(".key");
    match std::fs::read(&key_path) {
        Ok(key_content) => {
            if key_content.len() != KEY_LENGTH {
                return Err(std::io::Error::other(format!(
                    "Encryption key has unexpected size ({} bytes, expected {}). \
                     The key file may be corrupted.",
                    key_content.len(),
                    KEY_LENGTH
                )))
            }
            Ok(key_content)
        }
        Err(e) => Err(std::io::Error::other(format!(
            "No encryption key found at {}: {}. Set SUBSC_CLI_DB_PASSPHRASE \
             or create the key file.",
            key_path.display(),
            e
        ))),
    }
}

/// scrypt(passphrase, salt, 32, N=2^17, r=8, p=1).
pub fn derive_key_from_passphrase(passphrase: &str) -> Vec<u8> {
    use scrypt::{Params, scrypt as scrypt_kdf};
    let salt_path = config_dir().join(".key.salt");
    let salt = std::fs::read(&salt_path).unwrap_or_else(|_| vec![0u8; SALT_LENGTH]);
    let params = Params::new(17, 8, 1, 256 * 1024 * 1024).unwrap();
    let mut key = [0u8; KEY_LENGTH];
    scrypt_kdf(passphrase.as_bytes(), &salt, &params, &mut key).expect("scrypt failed");
    key.to_vec()
}

fn aesgcm(key: &[u8]) -> Aes256Gcm {
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key))
}

/// `iv(16) + tag(16) + ct(16)` — port of encryptBuffer.
pub fn encrypt_buffer(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let mut iv = [0u8; IV_LENGTH];
    getrandom(&mut iv);
    let cipher = aesgcm(key);
    let ct = cipher
        .encrypt(Nonce::from_slice(&iv), plaintext)
        .expect("encryption failed");
    let mut out = Vec::with_capacity(IV_LENGTH + TAG_LENGTH + ct.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ct[..TAG_LENGTH]);
    out.extend_from_slice(&ct[TAG_LENGTH..]);
    out
}

fn getrandom(buf: &mut [u8]) {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom").expect("no /dev/urandom");
    f.read_exact(buf).expect("urandom read failed")
}

/// Port of decryptBuffer — ciphertext layout `iv(16) + tag(16) + ct`.
pub fn decrypt_buffer(key: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let min_length = IV_LENGTH + TAG_LENGTH;
    if ciphertext.len() < min_length {
        return None;
    }
    let iv = &ciphertext[..IV_LENGTH];
    let tag = &ciphertext[IV_LENGTH..IV_LENGTH + TAG_LENGTH];
    let data = &ciphertext[IV_LENGTH + TAG_LENGTH..];
    let mut ct = Vec::with_capacity(data.len() + TAG_LENGTH);
    ct.extend_from_slice(data);
    ct.extend_from_slice(tag);
    aesgcm(key).decrypt(Nonce::from_slice(iv), ct.as_slice()).ok()
}

/// True when the buffer does NOT start with the SQLite magic header.
pub fn is_encrypted(buffer: &[u8]) -> bool {
    if buffer.len() < 16 {
        return false;
    }
    !buffer[..16].eq(SQLITE_MAGIC)
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Decrypt a file if it carries the config magic header (or the DB magic).
pub fn decrypt_file_if_needed(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let raw = std::fs::read(path)?;
    if raw.len() < 15 {
        return Ok(raw)
    }
    if raw[..15].eq(CONFIG_MAGIC) {
        let key = get_or_create_key()?;
        return decrypt_buffer(&key, &raw[15..])
            .ok_or_else(|| std::io::Error::other("config decryption failed"))
    }
    if is_encrypted(&raw) && raw.len() > 32 {
        // DB file (no magic header) — decrypt when it is not plaintext SQLite.
        let key = get_or_create_key()?;
        return decrypt_buffer(&key, &raw)
            .ok_or_else(|| std::io::Error::other("database decryption failed"))
    }
    Ok(raw)
}