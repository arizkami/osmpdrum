@echo off
setlocal

set "VCVARS=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"

if not exist "%VCVARS%" (
  echo vcvars64.bat not found at: %VCVARS%
  exit /b 1
)

call "%VCVARS%"
if errorlevel 1 exit /b %errorlevel%

call bun run build
if errorlevel 1 exit /b %errorlevel%

REM If host.exe is running, cargo may fail to overwrite it.
REM Attempt to stop it first (ignore errors if not running).
taskkill /IM host.exe /F >nul 2>&1

call cargo build --manifest-path host\Cargo.toml
if errorlevel 1 exit /b %errorlevel%

echo Build completed successfully.
exit /b 0
