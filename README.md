# MassKey Vault

**MassKey Vault** is a privacy-focused, offline-first password manager for Windows 10/11. It is built with Tauri 2, Rust, React, and TypeScript, with a customizable interface and encrypted local backups.

> [!WARNING]
> **v1.2.0 beta — security review incomplete.** This project has not undergone an independent security audit, full end-to-end Windows verification, or code signing. It is **not recommended for important or irreplaceable passwords** yet. A successful build is not proof of security.

## Download and install (for Windows users)

1. Open the repository's **Releases** page.
2. Select the latest **beta** release and download the attached Windows **Setup `.exe`** (not "Source code ZIP").
3. Run the installer and launch **MassKey Vault** from the desktop or Start Menu.
4. Create a master password (minimum **12 characters**) and **save your recovery key offline**.

**No Rust, Node.js, Visual Studio Build Tools, or Tauri installation is required for end users.** Windows WebView2 Runtime is required; the installer can provision it if absent (an internet connection may be needed for initial installation). The vault itself operates locally without a cloud account.

**Windows SmartScreen:** Unsigned preview installers can trigger a warning. Do not bypass security warnings unless you have independently verified the downloaded file and trust the source. You can compare the SHA-256 checksum against the release's `SHA256SUMS.txt`.

## Features

- Offline encrypted vault with a master password, emergency recovery key, and manual lock.
- Local account management, custom categories, search, website links, and local custom icons.
- Vietnamese, Thai, and English interfaces with a switcher on unlock and in app settings.
- Light, dark, and system appearance modes.
- Configurable inactivity auto-lock and clipboard clearing; optional lock on minimize and password reauthentication.
- Local Password Health checks for weak/reused passwords and password generation.
- Password-protected **AES-256 ZIP** backup/restore with drag-and-drop import and an independently encrypted vault payload.

**Changing a password in MassKey Vault does not update it on the associated website.** Always update the website first, verify the new login, then update the saved credential.

## Encryption and security model

- **Argon2id** derives the master-password wrapping key.
- **XChaCha20-Poly1305** encrypts and authenticates the vault data under a random 256-bit data encryption key.
- The **recovery key** provides a second protected route to recover vault access; there is no secret backdoor. Losing both master password and recovery key means losing access.
- **AES-256 encrypted ZIP** provides an additional, separate password for transfer. Import requires the ZIP password *and* the source vault's master password or recovery key.
- No cloud sync, server-side account, analytics, or tracking is required.

**Limitations:** Malware, keyloggers, memory inspection, clipboard managers, and a compromised unlocked Windows session can still expose secrets. Password Health is heuristic, not a full breached-password database lookup. Windows sleep/lock-event handling, backup adversarial testing, and independent review are not yet complete. Read [SECURITY.md](SECURITY.md) before reporting sensitive issues.

## Build from source (developers only)

On Windows 10/11 (64-bit), install:

- Node.js 22+
- Rust stable (MSVC toolchain)
- Visual Studio 2022 Build Tools with **Desktop development with C++** and Windows SDK
- Microsoft Edge WebView2 Runtime

```powershell
npm install
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build --bundles nsis
```

Or use `run_windows.bat` to run a development build and `build_windows.bat` for tests + a local installer. The NSIS installer is written to:

`src-tauri\target\release\bundle\nsis\`

Use **synthetic test credentials only** until a security audit and full Windows tests have been completed.

### Keep dependency versions reproducible

Before the first GitHub push, run `prepare_github_windows.bat` once. It generates **`package-lock.json` and `src-tauri/Cargo.lock`**, runs the frontend build and Rust tests, and leaves both lockfiles ready to commit. GitHub Actions intentionally requires both lockfiles and uses `npm ci` / `cargo test --locked`.

Do not commit your actual vault, exported ZIP backups, recovery key, master password, `.env` secrets, `node_modules`, Rust `target`, or `dist` directories.

## Update or transfer an existing vault

1. **Close MassKey Vault and back up your current data file** before updating or experimenting.
2. Create and verify an encrypted ZIP backup from the current machine.
3. On the new Windows PC, install the app and import the ZIP directly via drag and drop; provide the ZIP password and source vault secret.
4. Verify the restored accounts using **test-only** data first.

The existing **MassKey** format remains version 1 in this codebase. Migration from older *Nocturne Vault* formats is **not guaranteed** without a compatible converter. Never delete the only working copy.

## GitHub Releases (maintainers)

Release automation builds the unsigned Windows installer on GitHub-hosted Windows runners and creates a **draft prerelease** when a tag matching `v1.2.0-beta.1` is pushed. It is **not publicly published automatically**.

See [docs/RELEASING.md](docs/RELEASING.md) for the precise first-push, tag, review, and publishing steps. Release assets are the NSIS installer and `SHA256SUMS.txt`. Do not upload a personal vault or development `.bat` as end-user downloads.

## Contributing and disclosure

Contributions and bug reports are welcome, but **do not post real passwords, encrypted vaults, recovery keys, or sensitive vulnerability details in public Issues**. See [SECURITY.md](SECURITY.md). Source code is distributed under the [MIT License](LICENSE).
