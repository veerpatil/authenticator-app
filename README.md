# Authenticator

A desktop TOTP/HOTP authenticator app (like Google Authenticator or Authy) built with **Tauri v2**, **Rust**, and vanilla **HTML/CSS/JS**.

![Tauri](https://img.shields.io/badge/Tauri-v2-blue)
![Rust](https://img.shields.io/badge/Rust-2021-orange)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- TOTP (RFC 6238) and HOTP code generation
- SHA-1, SHA-256, SHA-512 algorithm support
- 6 or 8 digit codes
- Manual entry and `otpauth://` URI import (Ente Auth compatible)
- Live countdown timer with auto-refresh
- Click-to-copy with native clipboard
- Persistent local storage (JSON)
- Custom app icon with macOS dock icon support
- Distributable as `.dmg` (macOS), `.msi` (Windows), `.deb`/`.AppImage` (Linux)

---

## Prerequisites

Install these before you begin:

| Tool | Version | Install |
|------|---------|---------|
| **Rust** | stable (1.70+) | [rustup.rs](https://rustup.rs) |
| **Tauri CLI** | v2.x | `cargo install tauri-cli --version "^2"` |
| **Node.js** | 18+ (optional, only if adding a bundler) | [nodejs.org](https://nodejs.org) |

### Platform-specific dependencies

**macOS** — Xcode Command Line Tools:
```bash
xcode-select --install
```

**Linux (Debian/Ubuntu)** — system libraries:
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Windows** — [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

---

## Step-by-Step Build Guide

### Step 1: Clone the repository

```bash
git clone https://github.com/veerpatil/authenticator-app.git
cd authenticator-app
```

### Step 2: Understand the project structure

```
authenticator-app/
├── src/                        # Frontend (served by Tauri)
│   ├── index.html              #   App layout and structure
│   ├── styles.css              #   All styling (dark sidebar, light content)
│   └── main.js                 #   UI logic, event handlers, Tauri IPC
├── src-tauri/                  # Rust backend
│   ├── Cargo.toml              #   Rust dependencies
│   ├── tauri.conf.json         #   Tauri configuration
│   ├── capabilities/
│   │   └── default.json        #   Permission definitions
│   ├── icons/                  #   App icons (all platforms)
│   └── src/
│       ├── main.rs             #   Entry point
│       └── lib.rs              #   Core logic (OTP, storage, commands)
└── README.md
```

### Step 3: Install Rust dependencies

The first build will download and compile all crates. Key dependencies in `src-tauri/Cargo.toml`:

| Crate | Purpose |
|-------|---------|
| `tauri` | Desktop app framework |
| `totp-rs` | TOTP code generation (RFC 6238) |
| `hmac`, `sha1`, `sha2` | HOTP generation via raw HMAC |
| `data-encoding` | Base32 decoding of secrets |
| `url`, `urlencoding` | `otpauth://` URI parsing |
| `arboard` | Native clipboard access |
| `serde`, `serde_json` | JSON serialization for account storage |
| `cocoa`, `objc` | macOS dock icon (macOS only) |

### Step 4: Run in development mode

```bash
cargo tauri dev
```

This will:
1. Compile the Rust backend (~2-3 minutes first time)
2. Open the app window with hot-reload for the frontend
3. Changes to `src/*.html`, `src/*.css`, `src/*.js` reflect instantly
4. Rust changes in `src-tauri/src/` trigger a recompile on save

### Step 5: Build for production

```bash
cargo tauri build
```

The output binaries are in `src-tauri/target/release/bundle/`:

| Platform | Output |
|----------|--------|
| macOS | `dmg/Authenticator_0.1.0_aarch64.dmg` and `macos/Authenticator.app` |
| Windows | `msi/Authenticator_0.1.0_x64_en-US.msi` |
| Linux | `deb/authenticator_0.1.0_amd64.deb` and `appimage/authenticator_0.1.0_amd64.AppImage` |

> **Note:** You can only build for the platform you're running on. Cross-compilation requires CI (e.g., GitHub Actions).

---

## How It Works

### Architecture

```
┌─────────────────┐         invoke()          ┌──────────────────┐
│   Frontend       │  ──────────────────────►  │   Rust Backend    │
│   (HTML/CSS/JS)  │  ◄──────────────────────  │   (Tauri)         │
│                  │       JSON response       │                  │
│ • Sidebar nav    │                           │ • TOTP generation │
│ • Account cards  │                           │ • HOTP generation │
│ • Add form       │                           │ • Account CRUD    │
│ • Countdown      │                           │ • Clipboard       │
│ • Toast alerts   │                           │ • File storage    │
└─────────────────┘                           └──────────────────┘
```

### Tauri Commands (Rust ↔ JS bridge)

| Command | Description |
|---------|-------------|
| `get_all_codes` | Returns all accounts with their current OTP codes |
| `add_account` | Adds a new TOTP or HOTP account |
| `delete_account` | Removes an account by ID |
| `parse_otpauth_uri` | Parses an `otpauth://` URI into account fields |
| `next_hotp_code` | Increments an HOTP counter and returns the new code |
| `copy_to_clipboard` | Copies text to the system clipboard |
| `verify_account` | Returns debug info for an account (secret length, algorithm, etc.) |

### TOTP Flow

1. User adds an account with a Base32-encoded secret
2. Every second, the frontend calls `get_all_codes` via `invoke()`
3. Rust decodes the secret, creates a `TOTP` instance, generates a code using the current Unix timestamp
4. The code and time remaining are returned to the frontend
5. The UI renders the code with a countdown progress bar

### HOTP Flow

1. HOTP accounts show a static code based on a counter value
2. User clicks the "Next" button to advance the counter
3. Rust increments the counter, generates a new code via raw HMAC, and persists the new counter

### Data Storage

Accounts are stored as JSON at the OS-specific app data directory:

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/<your-app-identifier>/accounts.json` |
| Linux | `~/.local/share/<your-app-identifier>/accounts.json` |
| Windows | `%APPDATA%\<your-app-identifier>\accounts.json` |

---

## Adding an Account

### Option A: Manual entry

1. Click **Add** in the sidebar (or the + button)
2. Fill in: Service name, Email/Username, Secret key
3. Choose: Type (TOTP/HOTP), Digits (6/8), Algorithm, Period
4. Click **Add Account**

### Option B: Paste an `otpauth://` URI

1. Click **Add** → switch to the **Paste URI** tab
2. Paste the full `otpauth://totp/...` or `otpauth://hotp/...` URI
3. Click **Add Account**

> URIs exported from **Ente Auth** are supported — extra metadata parameters are automatically stripped.

---

## Running Tests

The Rust backend includes unit tests for TOTP/HOTP correctness:

```bash
cd src-tauri
cargo test
```

Tests verify:
- RFC 6238 test vectors (SHA-1, 8-digit, known timestamp)
- TOTP output matches Python `pyotp` reference at specific timestamps
- TOTP and manual HOTP produce identical codes for the same counter
- Base32 padding works for secrets of any length

---

## Troubleshooting

### "Your app is damaged" on macOS

macOS Gatekeeper blocks unsigned apps. Remove the quarantine flag:

```bash
xattr -cr /Applications/Authenticator.app
```

### Dock icon shows default Tauri icon (macOS dev mode)

The app programmatically sets the dock icon on startup using the `cocoa` crate. If it still shows the default icon, rebuild with:

```bash
cargo tauri dev
```

### OTP codes don't match your service

1. Click the **info (i)** button on the account card to verify the stored details
2. Check that the secret key was entered correctly (Base32, no spaces)
3. Ensure your system clock is synced — TOTP depends on accurate time
4. Confirm the algorithm and digit count match your service's settings

---

## Customizing the App Icon

Replace the source icon and regenerate all sizes:

```bash
cd src-tauri
cargo tauri icon path/to/your-icon.png
```

This generates icons for all platforms (macOS `.icns`, Windows `.ico`, Linux `.png`, iOS, Android) into the `icons/` directory.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri v2](https://v2.tauri.app) |
| Backend | Rust 2021 edition |
| Frontend | Vanilla HTML5 + CSS3 + ES6 JavaScript |
| OTP | [totp-rs](https://crates.io/crates/totp-rs) + raw HMAC |
| Clipboard | [arboard](https://crates.io/crates/arboard) |
| Storage | JSON via `serde_json` |
| Icons | `cargo tauri icon` |
