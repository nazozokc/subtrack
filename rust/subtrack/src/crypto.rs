//! Encryption — port of `@subtrack/lib/crypto.ts`.
//! AES-256-GCM with 16-byte IV; ciphertext layout is `iv(16) + tag(16) + ct`.

// NOTE: the TS implementation uses a 16-byte IV, which is larger than the
// GCM default 12-byte nonce — so we must use AesGcm<Aes256, U16>, NOT the
// Aes256Gcm alias (which would panic on a 16-byte Nonce::from_slice).
use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use aes_gcm::{AesGcm, Nonce};
use getrandom::getrandom as getrandom_fill;
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
        return std::path::PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home)
        .join(".config")
        .join("subtrack")
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
        return Ok(derive_key_from_passphrase(&passphrase));
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
                )));
            }
            Ok(key_content)
        }
        Err(_) => {
            // Key missing — generate, persist, and warn (port of getOrCreateKey).
            let dir = config_dir();
            std::fs::create_dir_all(&dir)?;
            let key = generate_key();
            write_secret_file(&key_path, &key)?;
            let integrity_path = dir.join(".key.sha256");
            let _ = write_secret_file(
                &integrity_path,
                format!("{}\n", sha256_hex(&key)).as_bytes(),
            );
            crate::logger::warn(&format!(
                "Encryption key created at: {}\n  \
                 !! BACKUP THIS KEY! If lost, your encrypted database cannot be recovered.\n  \
                 >> Copy the file to a secure location (e.g. password manager).\n  \
                 >> Or set SUBSC_CLI_DB_PASSPHRASE to derive the key from a passphrase instead.",
                key_path.display()
            ));
            Ok(key)
        }
    }
}

/// Cryptographically secure random key (32 bytes).
fn generate_key() -> Vec<u8> {
    let mut key = vec![0u8; KEY_LENGTH];
    // `getrandom::getrandom` delegates to the OS CSPRNG on every supported platform.
    getrandom_fill(&mut key).expect("OS CSPRNG is unavailable");
    key
}

/// Write a file with owner-only (0o600) permissions.
pub fn write_secret_file(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(data)
    }
    #[cfg(not(unix))]
    {
        let mut f = std::fs::File::create(path)?;
        f.write_all(data)
    }
}

/// scrypt(passphrase, salt, 32, N=2^17, r=8, p=1).
pub fn derive_key_from_passphrase(passphrase: &str) -> Vec<u8> {
    use scrypt::{Params, scrypt as scrypt_kdf};
    let salt_path = config_dir().join(".key.salt");
    let salt = std::fs::read(&salt_path).unwrap_or_else(|_| vec![0u8; SALT_LENGTH]);
    let params = Params::new(17, 8, 1, KEY_LENGTH).unwrap();
    let mut key = [0u8; KEY_LENGTH];
    scrypt_kdf(passphrase.as_bytes(), &salt, &params, &mut key).expect("scrypt failed");
    key.to_vec()
}

fn aesgcm(key: &[u8]) -> Aes256Gcm {
    Aes256Gcm::new_from_slice(key).expect("invalid key length")
}

/// `iv(16) + tag(16) + ct` — port of encryptBuffer.
pub fn encrypt_buffer(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let mut iv = [0u8; IV_LENGTH];
    getrandom_fill(&mut iv).expect("OS CSPRNG is unavailable");
    let cipher = aesgcm(key);
    // `Aead::encrypt` returns `ciphertext || tag`.
    let nonce = Nonce::<U16>::try_from(iv.as_slice()).expect("invalid nonce length");
    let sealed = cipher.encrypt(&nonce, plaintext).expect("encryption failed");
    let split = sealed.len() - TAG_LENGTH;
    let mut out = Vec::with_capacity(IV_LENGTH + TAG_LENGTH + split);
    out.extend_from_slice(&iv);
    out.extend_from_slice(&sealed[split..]); // tag
    out.extend_from_slice(&sealed[..split]); // ciphertext
    out
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
    let nonce = Nonce::<U16>::try_from(iv).expect("invalid nonce length");
    aesgcm(key).decrypt(&nonce, ct.as_slice()).ok()
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
        return Ok(raw);
    }
    if raw[..15].eq(CONFIG_MAGIC) {
        let key = get_or_create_key()?;
        return decrypt_buffer(&key, &raw[15..])
            .ok_or_else(|| std::io::Error::other("config decryption failed"));
    }
    if is_encrypted(&raw) && raw.len() > 32 {
        // DB file (no magic header) — decrypt when it is not plaintext SQLite.
        let key = get_or_create_key()?;
        return decrypt_buffer(&key, &raw)
            .ok_or_else(|| std::io::Error::other("database decryption failed"));
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CSPRNG-derived test keys only — literal key material (even one
    /// constant byte) is flagged by CodeQL as a hard-coded crypto value.
    fn generate_key() -> Vec<u8> {
        super::generate_key()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = "subtrack config payload \u{1f389}".as_bytes().to_vec();
        let sealed = encrypt_buffer(&key, &plaintext);

        // iv + tag + ciphertext layout (iv is random per call)
        assert!(sealed.len() > IV_LENGTH + TAG_LENGTH);
        assert_eq!(decrypt_buffer(&key, &sealed).unwrap(), plaintext);
        // a different key yields an authentication failure, not garbage
        let wrong_key = generate_key();
        assert_ne!(key, wrong_key);
        assert_eq!(decrypt_buffer(&wrong_key, &sealed), None);
        // too-short buffers cannot even carry iv+tag
        assert_eq!(decrypt_buffer(&key, &sealed[..12]), None);
    }

    #[test]
    fn encryption_uses_a_random_iv() {
        let key = generate_key();
        let a = encrypt_buffer(&key, b"x");
        let b = encrypt_buffer(&key, b"x");
        assert_ne!(a, b); // fresh non-deterministic iv per call
    }

    #[test]
    fn is_encrypted_mirrors_sqlite_magic_check() {
        assert!(!is_encrypted(b"SQLite format 3\0")); // plaintext SQLite
        assert!(!is_encrypted(b"tnenottuc")); // too short
        assert!(is_encrypted(&[0xffu8; 33])); // opaque non-SQLite blob
    }

    #[test]
    fn sha256_hex_matches_reference_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn derive_key_from_passphrase_is_deterministic() {
        let a = derive_key_from_passphrase("hello world");
        let b = derive_key_from_passphrase("hello world");
        assert_eq!(a.len(), KEY_LENGTH);
        assert_eq!(a, b);
        // never derive the null key
        assert_ne!(a, vec![0u8; KEY_LENGTH]);
    }

    #[cfg(unix)]
    #[test]
    fn write_secret_file_sets_owner_only_permissions() {
        let dir = std::env::temp_dir().join(format!(
            "subtrack-key-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".key");
        write_secret_file(&path, b"k").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
