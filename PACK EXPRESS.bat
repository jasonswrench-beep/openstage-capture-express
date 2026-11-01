@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - make the portable zip
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: make the portable zip
echo ============================================================
echo.
echo This builds the program and puts it, with its licence and a short
echo guide, in a folder and a zip inside the folder dist. Nothing there
echo is ever replaced. A full log is saved here as pack-log.txt.
echo.

if not exist "%~dp0..\openstage-author\capture\crates\openstage-capture-render\Cargo.toml" goto no_capture

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Building the program. Please wait...
if exist "pack-log.txt" del "pack-log.txt"
call "%~dp0cargo-run.bat" "pack-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed

echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\pack.ps1" -OutDir "%~dp0dist" >> "%~dp0pack-log.txt" 2>&1
set "RESULT=%ERRORLEVEL%"
type "%~dp0pack-log.txt" | findstr /i /c:"Made the folder" /c:"and the zip" /c:"already in" /c:"is missing"
if not "%RESULT%"=="0" goto failed

echo.
echo ------------------------------------------------------------
echo   Done. The zip is in the folder dist.
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
echo   The zip was NOT made.
echo ------------------------------------------------------------
echo.
echo Please send me the file pack-log.txt from this folder. The last
echo lines are shown below.
echo.
powershell -NoProfile -Command "if (Test-Path -LiteralPath 'pack-log.txt') { Get-Content -LiteralPath 'pack-log.txt' -Tail 25 }"

:done
echo.
pause
endlocal
