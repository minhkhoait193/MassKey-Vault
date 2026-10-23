@echo off
setlocal
cd /d "%~dp0"
title MassKey Vault - Development
where node >nul 2>nul || (echo [ERROR] Node.js 22+ required. Install from https://nodejs.org & pause & exit /b 1)
where npm >nul 2>nul || (echo [ERROR] npm missing. & pause & exit /b 1)
where cargo >nul 2>nul || (echo [ERROR] Rust missing. Install from https://rustup.rs and restart terminal. & pause & exit /b 1)
echo [MassKey Vault] Installing dependencies...
call npm install --no-audit --no-fund
if errorlevel 1 (echo [ERROR] npm install failed. & pause & exit /b 1)
echo [MassKey Vault] Starting development window...
call npm run tauri -- dev
if errorlevel 1 (echo [ERROR] App did not start. Read the first error above to identify the cause. & pause & exit /b 1)
