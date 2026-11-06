@echo off
rem Builds the release app without its installer and puts it in a portable copy's folder: Tsundoku.exe with
rem .portable.txt beside it, which keeps its data in the data folder there. The folder is the first argument, or
rem "portable" beside the Rust build folder (D:\Dev\tsundoku\portable on this PC, see AGENTS.md). Its data folder is
rem kept, so the copy can be rebuilt over and over. A copy running from that folder is quit before its exe is
rem replaced, and started again after.
setlocal
cd /d "%~dp0..\tsundoku" || exit /b 1

call npm run tauri build -- --no-bundle || exit /b 1

rem Where Cargo built it: the build folder can be moved by a .cargo\config.toml.
for /f "usebackq delims=" %%T in (`powershell -NoProfile -Command "(cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory"`) do set "TARGET=%%T"
if not defined TARGET (
  echo Couldn't find Cargo's build folder.
  exit /b 1
)

if "%~1"=="" (
  for %%P in ("%TARGET%\..\portable") do set "DESTINATION=%%~fP"
) else (
  set "DESTINATION=%~f1"
)

if not exist "%DESTINATION%" mkdir "%DESTINATION%" || exit /b 1

rem A running exe can't be replaced, and the copy may be hidden in the tray, so it's quit here: only the copy run
rem from this folder, never the installed copy or the dev build. Its downloads stop with it, tools included, since
rem they run in a job object that closes with the app. PowerShell exits with 2 when it quit one.
set "EXE=%DESTINATION%\Tsundoku.exe"
set "WAS_RUNNING="
powershell -NoProfile -Command "$exe = [IO.Path]::GetFullPath($env:EXE); $running = @(Get-Process -Name Tsundoku -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [string]::Equals($_.Path, $exe, [StringComparison]::OrdinalIgnoreCase) }); if (-not $running.Count) { exit 0 }; Write-Host 'Quitting the portable copy that is running...'; $running | Stop-Process -Force; $running | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue; exit 2"
if errorlevel 2 set "WAS_RUNNING=1"

copy /y "%TARGET%\release\Tsundoku.exe" "%EXE%" >nul || (
  echo Couldn't replace Tsundoku.exe in %DESTINATION%: quit the portable copy, then try again.
  exit /b 1
)
if not exist "%DESTINATION%\.portable.txt" (
  > "%DESTINATION%\.portable.txt" echo Keep this file beside Tsundoku.exe: with it, Tsundoku keeps its settings, tools and history in the data folder here instead of your user folder.
)
echo Portable copy ready in %DESTINATION%
if defined WAS_RUNNING (
  echo Starting it again.
  start "" /d "%DESTINATION%" "%EXE%"
)
