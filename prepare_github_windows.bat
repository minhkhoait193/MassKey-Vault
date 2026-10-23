@echo off
setlocal
cd /d "%~dp0"
title MassKey Vault - Prepare GitHub Release
where node >nul 2>nul || (echo [ERROR] Node.js required. & pause & exit /b 1)
where cargo >nul 2>nul || (echo [ERROR] Rust required. & pause & exit /b 1)
echo [1/4] Resolving npm dependencies and creating package-lock.json...
call npm install --no-audit --no-fund
if errorlevel 1 (echo [ERROR] npm install failed. & pause & exit /b 1)
echo [2/4] Creating src-tauri\Cargo.lock...
cd src-tauri
cargo generate-lockfile
if errorlevel 1 (cd .. & echo [ERROR] Cargo lockfile generation failed. & pause & exit /b 1)
cd ..
echo [3/4] Building frontend...
call npm run build
if errorlevel 1 (echo [ERROR] Frontend build failed. & pause & exit /b 1)
echo [4/4] Running Rust tests with frozen versions...
cargo test --locked --manifest-path src-tauri\Cargo.toml
if errorlevel 1 (echo [ERROR] Rust tests failed. & pause & exit /b 1)
echo.
echo [OK] Commit package-lock.json and src-tauri\Cargo.lock to GitHub.
echo      Use GitHub Actions to verify the Windows installer before tagging a release.
pause
