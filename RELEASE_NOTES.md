# MassKey Vault v1.4.0 — Windows

**Unsigned, unaudited release — do not use with important or irreplaceable credentials.** Windows SmartScreen may warn. Verify the installer checksum before running it.

## Included

- Windows desktop password vault with offline local encryption.
- Vietnamese / Thai / English language switching, including the login screen.
- Four themes: light, dark, light pink and dark purple.
- Two-door welcome screen and separate Passwords and Resources spaces.
- Encrypted saved resources for websites, online documents and public GitHub repositories, with search, collections, tags, notes, optional local folder paths, trash and restore.
- Custom categories, icons, website links and local password-health guidance.
- Master password, recovery key, configurable auto-lock and optional reauthentication.
- AES-256 encrypted ZIP export/import, including drag-and-drop restore.
- Windows session lock and suspend/resume vault locking.
- Verified backup preview and explicit confirmation before import.
- Encrypted account and resource trash with manual restore and no automatic purge.
- Faster password reauthentication without decrypting the entire vault again.
- NSIS Windows installer with Desktop and Start Menu shortcuts.

## Upgrade without losing your vault

1. Close MassKey Vault. Back up `%APPDATA%\com.masskey.vault\vault.nvault` and `vault.bak` to a separate folder, then verify the copies.
2. Download the attached `MassKey Vault_1.4.0_x64-setup.exe` and compare its SHA-256 with `SHA256SUMS.txt`.
3. Run Setup as the same Windows user who used the earlier version. The app keeps the same identifier and AppData vault path.
4. Unlock with your existing Master Password and check your accounts, resources and settings before deleting any old backup.

## Install

Download the attached Windows **Setup `.exe`**. Do not download GitHub's auto-generated **Source code (zip)** unless you intend to build the app yourself. No developer toolchain is needed for installation. If Windows SmartScreen warns about an unsigned installer, stop unless you trust the source and have verified its checksum.

## Known limitations

Windows lock/sleep integration, cryptographic design, ZIP parsing and installer behavior still require independent review and broader Windows testing. Password health is heuristic; changing a saved password does not change it on the service's website. Local folder paths for resources may need updating after restoring on another PC.

## Verify installer SHA-256

On Windows PowerShell run `Get-FileHash .\NAME_OF_INSTALLER.exe -Algorithm SHA256` and compare the result with the attached `SHA256SUMS.txt`.
