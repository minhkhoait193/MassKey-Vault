# Security Policy

## Project status

MassKey Vault v1.4.0 is **unsigned and not independently audited or certified**. It should not be relied on for important or irreplaceable secrets until its cryptographic implementation, ZIP import, migration, OS integration, and threat model have undergone professional review and Windows end-to-end testing.

## Reporting a vulnerability

**Do not disclose vulnerabilities involving credential exposure, encryption, recovery, ZIP parsing, or authentication in a public GitHub Issue.** Use GitHub's **Private vulnerability reporting** feature on the repository's **Security** tab if enabled. Repository maintainers should enable private vulnerability reporting before publishing the repo. If it is not enabled, contact the repository maintainer privately before sharing details or proof-of-concept data.

Never include a real master password, recovery key, vault database, private key, or real user credentials in a report. Use a minimal reproduction with synthetic data.

## Current caveats

- An unlocked vault can be exposed by malware with access to the Windows session or process memory.
- Clipboard clearing is best effort and cannot erase clipboard-manager history.
- Windows screen-lock/sleep handling and the installer need more end-to-end tests.
- An encrypted, authenticated vault does not prevent rollback to a previously valid encrypted file.
- Passing unit tests does not replace independent security assessment or code signing.
