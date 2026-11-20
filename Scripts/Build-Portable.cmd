@echo off
rem Builds the release app without its installer and puts it in a portable copy's folder: Tsundoku.exe with
rem .portable.txt beside it, which keeps its data in the data folder there. The folder is the first argument, or
rem "portable" beside the Rust build folder (D:\Dev\tsundoku\portable on this PC, see AGENTS.md). Its data folder is
rem kept, so the copy can be rebuilt over and over. A copy running from that folder stays open while this builds: it's
rem quit only once the new exe waits beside it, for the moment the swap takes, and started again after. A build that
rem fails leaves it running, and if the exe can't be replaced, the copy that was running is started again.
setlocal
cd /d "%~dp0..\tsundoku" || exit /b 1

rem Where Cargo builds: the build folder can be moved by a .cargo\config.toml.
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
set "EXE=%DESTINATION%\Tsundoku.exe"

rem The copy run from this folder, in PowerShell: never the installed copy or the dev build.
set "RUNNING=@(Get-Process -Name Tsundoku -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [string]::Equals($_.Path, [IO.Path]::GetFullPath($env:EXE), [StringComparison]::OrdinalIgnoreCase) })"
powershell -NoProfile -Command "if (%RUNNING%.Count) { Write-Host 'The portable copy is running. It stays open while this builds, then it is replaced and started again.' }"

call npm run tauri build -- --no-bundle || exit /b 1

if not exist "%DESTINATION%" mkdir "%DESTINATION%" || exit /b 1

rem The new exe is copied beside the old one before anything is quit, so the swap itself is only a rename.
copy /y "%TARGET%\release\Tsundoku.exe" "%EXE%.new" >nul || (
  echo Couldn't copy the new Tsundoku.exe into %DESTINATION%.
  exit /b 1
)

rem A running exe can't be replaced, and the copy may be hidden in the tray, so it's quit here, whether it was running
rem before the build or started during it. Its downloads stop with it, tools included, since they run in a job object
rem that closes with the app. PowerShell exits with 2 when it quit one.
set "WAS_RUNNING="
powershell -NoProfile -Command "$running = %RUNNING%; if (-not $running.Count) { exit 0 }; Write-Host 'Quitting the portable copy that is running...'; $running | Stop-Process -Force; $running | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue; exit 2"
if errorlevel 2 set "WAS_RUNNING=1"

rem Windows can keep a quit program's exe in use for a moment more (an antivirus reading it, say), so the swap is tried
rem for about 15 seconds.
set "REPLACED="
for /l %%N in (1,1,20) do if not defined REPLACED (
  move /y "%EXE%.new" "%EXE%" >nul 2>&1 && set "REPLACED=1"
  if not defined REPLACED powershell -NoProfile -Command "Start-Sleep -Milliseconds 500"
)
if not defined REPLACED (
  del "%EXE%.new" >nul 2>&1
  echo Couldn't replace Tsundoku.exe in %DESTINATION%: something still has it open. The new one is in %TARGET%\release.
  if defined WAS_RUNNING (
    echo Starting the copy that was running again.
    call :start_copy
  )
  exit /b 1
)

if not exist "%DESTINATION%\.portable.txt" (
  > "%DESTINATION%\.portable.txt" echo Keep this file beside Tsundoku.exe: with it, Tsundoku keeps its settings, tools and history in the data folder here instead of your user folder.
)
echo Portable copy ready in %DESTINATION%
if defined WAS_RUNNING (
  echo Starting it again.
  call :start_copy
)
exit /b 0

rem The app is started through PowerShell's Start-Process, since start hands it this script's output even when redirected, which would keep a program reading that output waiting until the app quits.
:start_copy
powershell -NoProfile -Command "Start-Process -FilePath $env:EXE -WorkingDirectory $env:DESTINATION"
exit /b
