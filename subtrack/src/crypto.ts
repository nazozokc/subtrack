import { randomBytes, createCipheriv, createDecipheriv, scryptSync } from "node:crypto"
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs"
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

function generateKey(): Buffer {
  return randomBytes(KEY_LENGTH)
}

function deriveKeyFromPassphrase(passphrase: string): Buffer {
  const saltPath = getSaltPath()
  let salt: Buffer
  if (existsSync(saltPath)) {
    salt = readFileSync(saltPath)
  } else {
    salt = randomBytes(SALT_LENGTH)
    const dir = getConfigDir()
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true, mode: 0o700 })
    writeFileSync(saltPath, salt, { mode: 0o600 })
  }
  return scryptSync(passphrase, salt, KEY_LENGTH)
}

function getOrCreateKey(): Buffer {
  const passphrase = process.env.SUBSC_CLI_DB_PASSPHRASE
  if (passphrase) return deriveKeyFromPassphrase(passphrase)

  const keyPath = getKeyPath()
  if (existsSync(keyPath)) return readFileSync(keyPath)

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
