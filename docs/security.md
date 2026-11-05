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

A newly registered device generates its own device keypair. On login, it merges
the account's encrypted history-key archive and publishes its own key into the
archive. The archive is protected using an OPAQUE export key and an authenticated
cipher; the server stores ciphertext and a revision number. Concurrent updates
are merged using compare-and-swap.

Existing devices must log in with a version that supports this archive before
their historical keys can be restored elsewhere. Keys already lost cannot be
reconstructed. An administrator password reset requires the previous password
to recover and re-encrypt the archive. The legacy registration and migration
paths still send the password to the server; therefore this is not yet a
zero-knowledge authentication system for every account lifecycle operation.
Manual identity export and import remain available for older devices.

---

## 4. Network Security & Transport Rules

- **Strict HTTPS / WSS Enforcement**:
  All connections to remote servers must use TLS (`https://` and `wss://`).
- **Loopback Exception**:
  Unencrypted HTTP and WS are permitted exclusively on loopback addresses (`127.0.0.1` and `localhost`) to facilitate local container development and automated testing.
- **Session Tokens**:
  Authentication bearer tokens are cryptographically randomized UUIDv4/hex strings stored in Redis with an enforced TTL (`SESSION_TTL_SECS`).
