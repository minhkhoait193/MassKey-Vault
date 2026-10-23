# MassKey Vault v1.2.0 Beta — Windows

**Development preview — do not use with important or irreplaceable credentials.** This build has not passed an independent security audit or Windows code signing.

## Included

- Windows desktop password vault with offline local encryption.
- Vietnamese / Thai / English language switching, including the login screen.
- Light/dark appearance and polished account manager.
- Custom categories, icons, website links and local password-health guidance.
- Master password, recovery key, configurable auto-lock and optional reauthentication.
- AES-256 encrypted ZIP export/import, including drag-and-drop restore.
- NSIS Windows installer with Desktop and Start Menu shortcuts.

## Install

Download the attached Windows **Setup `.exe`**. Do not download GitHub's auto-generated **Source code (zip)** unless you intend to build the app yourself. No developer toolchain is needed for installation. If Windows SmartScreen warns about an unsigned installer, stop unless you trust the source and have verified its checksum.

## Known limitations

Windows lock/sleep integration, cryptographic design, ZIP parsing, and installer behavior still require independent testing. Password health is heuristic; changing a saved password does not change it on the service's website.

## Verify installer SHA-256

On Windows PowerShell run `Get-FileHash .\NAME_OF_INSTALLER.exe -Algorithm SHA256` and compare the result with the attached `SHA256SUMS.txt`.
