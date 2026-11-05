@echo off
rem Starts the app in development: the UI reloads live, and Rust changes rebuild and restart the app.
cd /d "%~dp0..\tsundoku" || exit /b 1
call npm run tauri dev
