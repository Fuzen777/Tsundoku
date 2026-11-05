@echo off
rem Builds the release app and its NSIS installer, in the Rust build folder's release\bundle\nsis (see AGENTS.md).
cd /d "%~dp0..\tsundoku" || exit /b 1
call npm run tauri build
