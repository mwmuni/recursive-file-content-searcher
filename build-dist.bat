@echo off
REM build-dist.bat - produce installer, portable, and zip dist artifacts for Windows.
setlocal enabledelayedexpansion
cd /d %~dp0\electron-ui
echo [BUILD-DIST] Running clean
call npm run clean
if errorlevel 1 (
  echo [BUILD-DIST] Clean failed
  exit /b 1
)

echo [BUILD-DIST] Building NSIS installer
call npx electron-builder --win nsis
if errorlevel 1 (
  echo [BUILD-DIST] NSIS build failed
  exit /b 1
)

echo [BUILD-DIST] Building portable package
call npx electron-builder --win portable
if errorlevel 1 (
  echo [BUILD-DIST] Portable build failed
  exit /b 1
)

echo [BUILD-DIST] Building zip package
call npx electron-builder --win zip
if errorlevel 1 (
  echo [BUILD-DIST] Zip build failed
  exit /b 1
)

echo [BUILD-DIST] Completed: installer, portable, zip artifacts are in electron-ui\dist
endlocal
exit /b 0
