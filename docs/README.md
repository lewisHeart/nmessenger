# NMessenger Documentation

Welcome to the **NMessenger** documentation. Here you will find comprehensive guides on architecture, security, local development, server deployment, release packaging, and bot integration.

## Documentation Index

| Guide | Description |
| :--- | :--- |
| [Architecture & System Design](architecture.md) | Client-server architecture, workspace crates, real-time protocols, and state management. |
| [Security & Cryptography](security.md) | End-to-end encryption (E2EE), identity vault, cryptographic keys, and privacy guarantees. |
| [Development Guide](development.md) | Prerequisites, local setup, running tests, graphical smoke tests, and coding guidelines. |
| [Operations & Deployment](operations.md) | Production server setup, Docker Compose, Caddy TLS reverse proxy, backups, and maintenance. |
| [Building & Releases](releases.md) | Multi-platform build matrix, packaging scripts, AppImage, Flatpak, Windows installers, macOS bundles, and Android APKs. |
| [Bot SDK & Ecosystem](bots.md) | Registering bots, API authentication, sending and receiving end-to-end encrypted messages via the SDK. |

---

## Quick Reference

- **Desktop Client**: Written in Rust using [egui](https://github.com/emilk/egui) with a classic nostalgic Windows Live Messenger interface.
- **Backend Server (`nm-servo`)**: Asynchronous [Actix Web](https://actix.rs) service with PostgreSQL persistence and Redis pub/sub.
- **Shared Protocol (`nm-protocol`)**: Strongly typed data models, authentication payloads, WebSocket events, and RPC definitions.
- **Mobile Client**: Native Android APK built with ARM64/x86_64 Rust cdylib and Java JNI runtime, requiring no Gradle.

