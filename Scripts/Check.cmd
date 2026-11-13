@echo off
rem Checks the Rust code's formatting, runs Clippy with warnings as errors and the Rust tests, then type-checks, lints and
rem tests the UI. Stops at the first failure. Run it before every commit.
rem Passed or not, it then deletes old builds of our own crates from the build folder it used (Prune-Build.ps1), which
rem would otherwise grow by gigabytes a day. Its exit code stays the checks'.
cd /d "%~dp0..\tsundoku" || exit /b 1
call :checks
set "RESULT=%ERRORLEVEL%"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Prune-Build.ps1"
exit /b %RESULT%

:checks
cargo fmt --all --check || exit /b 1
cargo clippy --workspace --all-targets -- -D warnings || exit /b 1
rem The Rust tests also regenerate the UI's src\bindings from the Rust types, so the UI checks below see the current ones.
cargo test --workspace || exit /b 1
call npm run check || exit /b 1
echo All checks passed.
exit /b 0
