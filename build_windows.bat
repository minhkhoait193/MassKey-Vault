@echo off
setlocal
cd /d "%~dp0"
title MassKey Vault - Windows Build
where node >nul 2>nul || (echo [ERROR] Node.js required. & pause & exit /b 1)
where cargo >nul 2>nul || (echo [ERROR] Rust required. & pause & exit /b 1)
call npm install --no-audit --no-fund
if errorlevel 1 (echo [ERROR] Failed to install Node packages. & pause & exit /b 1)
echo [MassKey Vault] Running Rust unit tests...
cd src-tauri
cargo test
if errorlevel 1 (cd .. & echo [ERROR] Security tests failed. No installer produced. & pause & exit /b 1)
cd ..
echo [MassKey Vault] Building Windows NSIS installer...
call npm run tauri -- build --bundles nsis
if errorlevel 1 (echo [ERROR] Build failed. & pause & exit /b 1)
echo.
echo Installer: src-tauri\target\release\bundle\nsis\
explorer "src-tauri\target\release\bundle\nsis"
pause
