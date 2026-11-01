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

if not exist "%~dp0..\openstage-author\capture\crates\openstage-capture-render\Cargo.toml" goto no_capture

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Building the program. Please wait...
if exist "installer-log.txt" del "installer-log.txt"
call "%~dp0cargo-run.bat" "installer-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed

echo.
echo Making the setup file. Please wait...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\make-installer.ps1" -OutDir "%~dp0dist" >> "%~dp0installer-log.txt" 2>&1
set "RESULT=%ERRORLEVEL%"
type "%~dp0installer-log.txt" | findstr /i /c:"Made the installer" /c:"SHA-256" /c:"already in" /c:"is missing" /c:"Inno Setup was not found"
if not "%RESULT%"=="0" goto failed

echo.
echo ------------------------------------------------------------
echo   Done. The setup file is in the folder dist.
echo ------------------------------------------------------------
if not defined EXPRESS_NO_OPEN start "" "%~dp0dist"
goto done

:no_capture
echo ------------------------------------------------------------
echo   The OpenStage code was not found next to this folder.
echo ------------------------------------------------------------
echo.
echo Double-click GET OPENSTAGE CODE.bat to get it, then try again.
echo Nothing was changed.
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
