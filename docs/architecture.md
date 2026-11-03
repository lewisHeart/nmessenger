# Architecture & System Design

NMessenger is engineered as a high-performance, modular messaging ecosystem structured as a unified Rust workspace.

```
nmessenger/
├── nm-protocol/          # Shared strongly-typed data structures, events, and API DTOs
├── nm-servo/             # Actix Web backend, database migrations, and WebSocket relay
├── nmessenger-client/    # Cross-platform desktop client (egui), crypto, and media pipelines
├── android/              # Native Android shell, JNI bridge, and manifest
├── scripts/              # Build automation, Android packager, release orchestrator
└── deploy/               # Caddyfile configuration, desktop entries, and icon assets
```

---

## 1. Crates & Responsibilities

### `nm-protocol`
The bedrock crate containing all shared types:
- **Models**: `Profile`, `Contact`, `Conversation`, `Message`, `Badge`, `Presence`, `UserSettings`.
- **Requests & Responses**: Authentication, contacts management, category updates, profile updates, and file exchange DTOs.
- **WebSocket Server Events**: Instant delivery notifications, nudges, typing status, presence changes, and calls.
- Pure Rust, zero unnecessary dependencies, fully serialized with `serde` and `serde_json`.

### `nm-servo`
The central backend server:
- Built on **Actix Web** and **Tokio**.
- **PostgreSQL** (via `sqlx`): Persistent relational store for users, profiles, friendships, categories, conversation metadata, offline message logs, and release webhooks.
- **Redis**: Real-time pub/sub bus for dispatching presence events, websocket broadcast channels, rate limiting, and ephemeral token sessions.
- **File Storage**: Configurable local or volume-backed attachment store (`UPLOAD_DIR`) enforcing per-user upload quotas.

### `nmessenger-client`
The client application:
- Built with **egui** and **eframe** for responsive, GPU-accelerated rendering across desktop platforms (Linux X11/Wayland, Windows, macOS).
- **Identity Vault**: Local encrypted storage of private device keys using Argon2 key derivation.
- **Networking**: Asynchronous HTTP client via `reqwest` and persistent bidirectional WebSocket via `tokio-tungstenite`.
- **Outbox System**: Reliable message delivery queue with automatic retry and local reconciliation.
- **Media**: ALSA / PulseAudio audio capture and playback, webcam streaming via `ffmpeg`, and X11 screen sharing.

### `android/`
The Android target runtime:
- Compiles `nmessenger-client` as a native C dynamic library (`libnmessenger_client.so`) using the Android NDK.
- Lightweight Java host activity (`MainActivity.java`) handles lifecycle, display surface, and audio focus without relying on Gradle or heavy dependencies.

---

## 2. Communication Lifecycle

```
[ Desktop / Mobile Client ]
          │  1. Authenticate (HTTP POST /v1/auth/login)
          ▼
     [ nm-servo ] ──( Verify Password & Identity )──> [ PostgreSQL ]
          │
          │  2. Connect WebSocket (/v1/events) with Bearer token
          ▼
[ Redis Pub/Sub Bus ] <─── Broadcast Presence & Message Alerts
          │
          ▼  3. Direct / Group Encrypted Delivery
[ Recipient Clients ]
```

1. **Authentication**: Client negotiates credentials with `/v1/auth/login` and receives an ephemeral bearer token.
2. **WebSocket Session**: Client connects to `/v1/events`, performing the initial handshake frame to register active sessions in Redis.
3. **Presence**: Client publishes state (`Online`, `Busy`, `Away`, `Offline`). Redis fans out presence updates to mutual contacts.
4. **Message Transport**:
   - The sender encrypts message payloads client-side with E2EE.
   - The ciphertext envelope is dispatched via HTTP or WebSocket.
   - The server stores the encrypted payload in PostgreSQL and dispatches a delivery event to recipient sockets.

