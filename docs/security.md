# Security & Cryptography

NMessenger is designed with a strict zero-knowledge message privacy model. The server routes and persists message envelopes, but lacks the cryptographic material required to decrypt content, images, attachments, or voice notes.

---

## 1. End-to-End Encryption (E2EE)

All direct communications, voice messages, file transfers, and group chats are protected by client-side cryptography.

### Key Exchange & Cipher Primitives
- **Key Agreement**: Diffie-Hellman over Curve25519 (**X25519**).
- **Authentication & Signatures**: **Ed25519** digital signatures for authenticating device identity.
- **Symmetric Cipher**: **ChaCha20-Poly1305** AEAD (Authenticated Encryption with Associated Data).
- **Key Derivation**: **Argon2id** for deriving master keys from account passwords.

When a message is sent:
1. The sending client derives an ephemeral session key using the recipient's public identity key.
2. The payload (text, formatted style, attachment metadata) is encrypted with ChaCha20-Poly1305.
3. The server receives only ciphertext envelopes with authenticated metadata (sender ID, recipient ID, message ID, timestamp).

---

## 2. Local Identity Vault

Cryptographic keypairs are generated strictly on the client device and are never sent to the server.

- The private identity keys are stored inside the local identity vault in the application data directory (`NM_CONFIG_DIR`).
- The vault file is encrypted with a master key derived from the user's password using Argon2id with high memory and iteration parameters.
- If the server database is compromised, an attacker obtains only encrypted envelopes with zero access to private keys.

---

## 3. Multi-Device & Historical Message Recovery

Because keys are stored locally on each client device:

- A newly registered or secondary device generates its own device keypair.
- To decrypt historical messages on a new device, the user can export their cryptographic identity from an existing active device:
  1. Open **Options > Privacy > Recover older messages** on the active device.
  2. Export the encrypted identity backup file.
  3. Import the backup file on the new device, unlocking it with the account password.

---

## 4. Network Security & Transport Rules

- **Strict HTTPS / WSS Enforcement**:
  All connections to remote servers must use TLS (`https://` and `wss://`).
- **Loopback Exception**:
  Unencrypted HTTP and WS are permitted exclusively on loopback addresses (`127.0.0.1` and `localhost`) to facilitate local container development and automated testing.
- **Session Tokens**:
  Authentication bearer tokens are cryptographically randomized UUIDv4/hex strings stored in Redis with an enforced TTL (`SESSION_TTL_SECS`).

