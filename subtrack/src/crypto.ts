import { randomBytes, createCipheriv, createDecipheriv, scryptSync } from "node:crypto"
import { readFileSync, writeFileSync, existsSync, mkdirSync, statSync, chmodSync } from "node:fs"
import path from "node:path"
import { homedir } from "node:os"
import { consola } from "consola"

const ALGORITHM = "aes-256-gcm"
const KEY_LENGTH = 32
const IV_LENGTH = 16
const TAG_LENGTH = 16
const SALT_LENGTH = 32
const SQLITE_MAGIC = Buffer.from("SQLite format 3\x00")

function getConfigDir(): string {
  return process.env.SUBSC_CLI_DB_DIR ?? path.join(homedir(), ".config", "subtrack")
}

function getKeyPath(): string {
  return path.join(getConfigDir(), ".key")
}

function getSaltPath(): string {
  return path.join(getConfigDir(), ".key.salt")
}

// ---- Internal helpers ----

/** Verify that a sensitive file has restrictive permissions (owner-only). */
function verifySecureFilePermission(filePath: string, name: string): void {
  try {
    const st = statSync(filePath)
    // Check for group/others permissions (mode 077 = others rwx)
    if (st.mode & 0o077) {
      consola.warn(
        `${name} has overly permissive permissions (${(st.mode & 0o777).toString(8)}). Fixing to 600.`,
      )
      chmodSync(filePath, 0o600)
    }
    // Check ownership on non-Windows
    if (process.platform !== "win32" && typeof process.getuid === "function") {
      const uid = process.getuid()
      if (uid !== undefined && uid !== null && st.uid !== uid) {
        consola.warn(
          `${name} is not owned by current user (uid ${st.uid}). This may indicate tampering.`,
        )
      }
    }
  } catch {
    // skip if file not accessible
  }
}

function generateKey(): Buffer {
  return randomBytes(KEY_LENGTH)
}

function deriveKeyFromPassphrase(passphrase: string): Buffer {
  const saltPath = getSaltPath()
  let salt: Buffer
  if (existsSync(saltPath)) {
    verifySecureFilePermission(saltPath, "Key salt file")
    salt = readFileSync(saltPath)
  } else {
    salt = randomBytes(SALT_LENGTH)
    const dir = getConfigDir()
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true, mode: 0o700 })
    writeFileSync(saltPath, salt, { mode: 0o600 })
  }
  return scryptSync(passphrase, salt, KEY_LENGTH, { N: 131072, r: 8, p: 1 })
}

function getOrCreateKey(): Buffer {
  const passphrase = process.env.SUBSC_CLI_DB_PASSPHRASE
  if (passphrase) return deriveKeyFromPassphrase(passphrase)

  const keyPath = getKeyPath()
  if (existsSync(keyPath)) {
    verifySecureFilePermission(keyPath, "Encryption key file")
    return readFileSync(keyPath)
  }

  const dir = getConfigDir()
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true, mode: 0o700 })
  const key = generateKey()
  writeFileSync(keyPath, key, { mode: 0o600 })
  consola.warn(
    `Encryption key created at: ${keyPath}\n` +
    `  ⚠  BACKUP THIS KEY! If lost, your encrypted database cannot be recovered.\n` +
    `  💡  Copy the file to a secure location (e.g. password manager).\n` +
    `  🔑  Or set SUBSC_CLI_DB_PASSPHRASE to derive the key from a passphrase instead.`,
  )
  return key
}

// ---- Public API ----

export function encryptBuffer(plaintext: Buffer): Buffer {
  const key = getOrCreateKey()
  const iv = randomBytes(IV_LENGTH)
  const cipher = createCipheriv(ALGORITHM, key, iv)
  const encrypted = Buffer.concat([cipher.update(plaintext), cipher.final()])
  const tag = cipher.getAuthTag()
  return Buffer.concat([iv, tag, encrypted])
}

export function decryptBuffer(ciphertext: Buffer): Buffer {
  const minLength = IV_LENGTH + TAG_LENGTH
  if (ciphertext.length < minLength) {
    throw new Error(`Ciphertext too short: expected at least ${minLength} bytes, got ${ciphertext.length}`)
  }
  const key = getOrCreateKey()
  const iv = ciphertext.subarray(0, IV_LENGTH)
  const tag = ciphertext.subarray(IV_LENGTH, IV_LENGTH + TAG_LENGTH)
  const data = ciphertext.subarray(IV_LENGTH + TAG_LENGTH)
  const decipher = createDecipheriv(ALGORITHM, key, iv)
  decipher.setAuthTag(tag)
  return Buffer.concat([decipher.update(data), decipher.final()])
}

/**
 * Returns true if the buffer does NOT start with the SQLite magic header,
 * meaning it is (likely) an encrypted blob.
 */
export function isEncrypted(buffer: Buffer): boolean {
  if (buffer.length < 16) return false
  return !buffer.subarray(0, 16).equals(SQLITE_MAGIC)
}

export function hasEncryptionKey(): boolean {
  if (process.env.SUBSC_CLI_DB_PASSPHRASE) return true
  return existsSync(getKeyPath())
}
