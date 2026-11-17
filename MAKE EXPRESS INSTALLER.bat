@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - make the installer
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: make the installer
echo ============================================================
echo.
echo This builds the program and makes one setup file in the folder
echo dist, using Inno Setup (a free program). The setup file uses the
echo product icon. Nothing there is ever replaced. A full log is saved
echo here as installer-log.txt.
echo.

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Building the program. Please wait...
if exist "installer-log.txt" del "installer-log.txt"
rem A release keeps this computer's folder names out of the program, and knows which release it is: see tools\release-flags.ps1.
for /f "usebackq delims=" %%F in (`powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\release-flags.ps1"`) do set "CARGO_ENCODED_RUSTFLAGS=%%F"
set /p OPENSTAGE_RELEASE=<"%~dp0installer\release.txt"
call "%~dp0cargo-run.bat" "installer-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed

echo.
echo Making the setup file. Please wait...
rem MAKE EXPRESS INSTALLER.bat sign signs the program and the setup (tools\sign_windows.py says what signing needs).
rem MAKE SIGNED EXPRESS INSTALLER.bat does that with a double-click, and names its own folder in EXPRESS_OUTDIR.
set "OUTDIR=%~dp0dist"
if defined EXPRESS_OUTDIR set "OUTDIR=%EXPRESS_OUTDIR%"
set "SIGNARG="
if /i "%~1"=="sign" set "SIGNARG=-Sign"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\make-installer.ps1" -OutDir "%OUTDIR%" %SIGNARG% >> "%~dp0installer-log.txt" 2>&1
set "RESULT=%ERRORLEVEL%"
type "%~dp0installer-log.txt" | findstr /i /c:"Made the installer" /c:"SHA-256" /c:"names this computer" /c:"already in" /c:"is missing" /c:"Inno Setup was not found"
if not "%RESULT%"=="0" goto failed

echo.
echo ------------------------------------------------------------
echo   Done. The setup file is in %OUTDIR%
echo ------------------------------------------------------------
if not defined EXPRESS_NO_OPEN start "" "%OUTDIR%"
goto done

:no_rust
echo ------------------------------------------------------------
echo   Rust is not installed on this computer yet.
echo ------------------------------------------------------------
echo.
echo Please double-click BUILD EXPRESS.bat first. It explains how to
echo install Rust, which is free.
goto done

:failed
echo.
echo ------------------------------------------------------------
echo   The setup file was NOT made.
echo ------------------------------------------------------------
echo.
echo If Inno Setup was not found, install it (it is free) from
echo https://jrsoftware.org/isdl.php and try again. Otherwise please
echo send me the file installer-log.txt from this folder. The last
echo lines are shown below.
echo.
powershell -NoProfile -Command "if (Test-Path -LiteralPath 'installer-log.txt') { Get-Content -LiteralPath 'installer-log.txt' -Tail 25 }"

:done
echo.
pause
endlocal
