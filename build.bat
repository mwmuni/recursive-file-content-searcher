@echo off
echo Building Filesystem Searcher...

echo.
echo Building Rust binary...
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo Failed to build Rust binary
    exit /b %ERRORLEVEL%
)

echo.
echo Building Electron UI...
cd electron-ui
if %ERRORLEVEL% neq 0 (
    echo Failed to change directory
    exit /b %ERRORLEVEL%
)

echo Installing npm dependencies...
call npm install
if %ERRORLEVEL% neq 0 (
    echo Failed to install dependencies
    exit /b %ERRORLEVEL%
)

echo Building Electron app...
call npm run build
if %ERRORLEVEL% neq 0 (
    echo Failed to build Electron app
    exit /b %ERRORLEVEL%
)

echo.
echo Build complete!
echo The application can be found in: electron-ui\dist\win-unpacked\Filesystem Searcher.exe
echo. 