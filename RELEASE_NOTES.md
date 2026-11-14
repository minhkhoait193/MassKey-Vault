# MassKey Vault v1.4.1 — Windows bug fix

**Unsigned, unaudited release.** Windows SmartScreen may warn. Verify the installer checksum before running it. Do not use this app for important or irreplaceable credentials until it has received an independent security review.

## Fixed

- Password reveal and copy now explain when a Master Password is required or incorrect, with the error beside the reauthentication field.
- Thai and English translations now cover the affected success and error messages, resource actions, backup preview, and the Resources breadcrumb.
- The sidebar collapse control is compact, keyboard accessible, and fits the narrow window layout.

## Upgrade and data

This patch keeps the `com.masskey.vault` app identifier, the `%APPDATA%\com.masskey.vault\vault.nvault` data location, and the existing encrypted vault format. It does not migrate or rewrite vault data during installation.

1. Close MassKey Vault. Copy `vault.nvault` and `vault.bak`, if present, to a separate safe location and verify the copies.
2. Download `MassKey.Vault_1.4.1_x64-setup.exe` and verify it against `SHA256SUMS.txt`.
3. Run Setup as the same Windows user. Unlock with your existing Master Password and check accounts, resources, and settings before removing any backup.

The Windows installer is unsigned. The password reauthentication behavior is covered by a Rust regression test. Frontend and Rust builds, legacy vault and ZIP compatibility tests run in CI. Interactive Windows testing is still required on users' own machines.
