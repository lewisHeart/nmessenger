# Development Guide

This guide covers building, testing, and debugging NMessenger on your local workstation.

---

## 1. Prerequisites

### Rust Toolchain
Install Rust 1.85+ with `rustup`:
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### System Libraries (Linux)
Building the desktop client requires native OpenGL, ALSA audio, and X11/Wayland windowing libraries:

On Debian / Ubuntu:
```sh
sudo apt update && sudo apt install -y \
  build-essential pkg-config libasound2-dev libudev-dev \
  libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev
```

On Fedora / RHEL:
```sh
sudo dnf install -y \
  gcc gcc-c++ pkgconf-pkg-config alsa-lib-devel systemd-devel \
  libxkbcommon-devel wayland-devel libX11-devel mesa-libGL-devel
```

On Arch Linux:
```sh
sudo pacman -S --needed \
  base-devel pkgconf alsa-lib systemd-libs \
  libxkbcommon wayland libx11 mesa
```

### Docker & Docker Compose
Docker is required to run the local PostgreSQL database, Redis instance, and integration test infrastructure.

---

## 2. Local Setup & Running

### Starting the Development Environment

```sh
# 1. Copy sample environment file
cp .env.example .env

# 2. Start PostgreSQL, Redis, and backend in Docker Compose
make up

# 3. Launch the desktop client
make client
```

### Running Backend on Host (outside Docker)
If you prefer running `nm-servo` directly on the host machine while keeping PostgreSQL and Redis containerized:
```sh
# Start only database and cache containers
make infra

# Export configuration and run backend
set -a; . ./.env; set +a
make server
```

---

## 3. Automated Testing

### Code Quality & Static Analysis
```sh
make check
```
Runs `cargo fmt`, `cargo clippy`, and verifies source file size limits.

### Unit & Integration Tests
```sh
# Launches isolated test containers and runs workspace tests
make test

# Stops the test infrastructure
make test-stop
```

### Graphical UI Smoke Test
NMessenger includes automated UI smoke tests with headless virtual display rendering:
```sh
make smoke
```
*Requires `Xvfb`, `xdotool`, `ffmpeg`, `Python 3`, and PostgreSQL client utilities.*

---

## 4. Android Local Development

To compile and deploy the Android APK locally to an emulator or USB-connected device:

```sh
# Verify SDK, NDK, and build tools
make android-check

# Compile development APK
make android

# Install directly via ADB
adb install -r target/android/nmessenger-arm64-v8a-debug-development.apk
```

