# Windows GitHub Release Guide (Beta)

**Do not publish a stable release without an independent security audit.** All release automation in this repository is intentionally beta-only and creates a draft.

## One-time setup

1. Create a GitHub repository named `MassKey-Vault`, preferably **private** while preparing and testing.
2. On a Windows development PC run `prepare_github_windows.bat` from the source directory. This produces the npm and Rust lockfiles and runs preliminary tests.
3. Review `.gitignore` and run `git status --short` to ensure that no real vaults, secrets, backup archives, or build artifacts will be committed.
4. In the repository's GitHub Settings, enable **private vulnerability reporting** under Code security.
5. Initialize Git and push the prepared source:

```powershell
git init
git add .
git status --short
git commit -m "Prepare MassKey Vault v1.2 beta source"
git branch -M main
git remote add origin https://github.com/YOUR_USERNAME/MassKey-Vault.git
git push -u origin main
```

Replace `YOUR_USERNAME`; if your project already has a Git repository, **do not reinitialize it**. Use your existing remote and commit. Inspect your **entire Git history**, not only the current working tree, for any accidentally committed credentials. If secret data was ever pushed, removing it in a later commit is not sufficient; rotate exposed credentials and purge the affected history.

## Verify CI before tagging

1. Open GitHub **Actions → Windows CI**.
2. Ensure the frontend build, Rust tests, NSIS installer build and artifact upload all succeed.
3. Download the CI artifact (the unsigned installer) and test it on a clean Windows VM with synthetic passwords, including setup/uninstall, desktop shortcut, recovery, ZIP export/import, and migration.
4. Do not advertise the beta for use with real secrets without an independent audit.

## Prepare a draft beta release

Check that `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` all have `1.3.0` as the version. From the reviewed `main` commit:

```powershell
git tag v1.3.0-beta.1
git push origin v1.3.0-beta.1
```

GitHub Actions → **Windows Beta Release (draft)** will build from that tag, run tests, create a **draft prerelease**, and attach the unsigned NSIS Setup `.exe` plus `SHA256SUMS.txt`. The draft remains private until you explicitly publish it.

In GitHub **Releases**, review assets, checksum, version, release notes, and beta warning; only then click **Publish release**. Do not publish it as a stable release. If the workflow fails, fix the source and use a **new tag** (for example `v1.3.0-beta.2`) rather than silently retargeting a published tag.

## End-user instructions

End users download the **Setup `.exe` from Releases**, install, and launch via desktop or Start Menu. **They do not install Node.js, Rust, Visual Studio or Tauri.** WebView2 is the only relevant runtime and is normally present on Windows 11; the NSIS installer can download its bootstrapper if needed.

## Later stable release prerequisites

- Independent security assessment, remediation and regression tests.
- Windows 10/11 clean-install, upgrade, and uninstall tests.
- Vault integrity, wrong password, recovery, ZIP traversal/bomb/tamper tests.
- Migration compatibility and corruption/recovery tests.
- Signed Windows installer and published checksum.
- Remove beta warnings only after these claims are verified.
