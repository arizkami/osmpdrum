@echo off
setlocal

REM Build frontend
call bun run build
if errorlevel 1 exit /b %errorlevel%

REM Kill any running host.exe before overwriting it
taskkill /IM host.exe /F >nul 2>&1

REM Build Rust backend
call cargo build --manifest-path host\Cargo.toml
if errorlevel 1 exit /b %errorlevel%

echo.
echo Build completed. Run with:
echo   host\target\debug\host.exe
echo.

REM Pass --run to also launch the server immediately
if "%1"=="--run" (
  host\target\debug\host.exe
)

exit /b 0
