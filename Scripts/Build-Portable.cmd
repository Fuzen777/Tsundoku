@echo off
rem Builds the release app without its installer and puts it in a portable copy's folder: Tsundoku.exe with
rem .portable.txt beside it, which keeps its data in the data folder there. The folder is the first argument, or
rem "portable" beside the Rust build folder (D:\Dev\tsundoku\portable on this PC, see AGENTS.md). Its data folder is
rem kept, so the copy can be rebuilt over and over. Close the portable copy first: a running exe can't be replaced.
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
copy /y "%TARGET%\release\Tsundoku.exe" "%DESTINATION%\Tsundoku.exe" >nul || (
  echo Couldn't replace Tsundoku.exe in %DESTINATION%: close the portable copy, then try again.
  exit /b 1
)
if not exist "%DESTINATION%\.portable.txt" (
  > "%DESTINATION%\.portable.txt" echo Keep this file beside Tsundoku.exe: with it, Tsundoku keeps its settings, tools and history in the data folder here instead of your user folder.
)
echo Portable copy ready in %DESTINATION%
