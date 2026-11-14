@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - check this computer
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: is this computer ready?
echo ============================================================
echo.
echo This looks at the Windows version, the displays, whether pictures
echo of the screen arrive, whether the names of clicked things can be
echo found, whether the hotkeys are free, and whether there is room
echo for guides. It does not click, type or record anything. A full
echo log is saved in this folder as check-log.txt.
echo.

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Getting the program ready. Please wait...
if exist "check-build-log.txt" del "check-build-log.txt"
call "%~dp0cargo-run.bat" "check-build-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed_build

echo.
"%~dp0target\release\openstage-capture-express.exe" check-computer > "%~dp0check-log.txt" 2>&1
set "RESULT=%ERRORLEVEL%"
type "%~dp0check-log.txt"
echo.
if not "%RESULT%"=="0" goto not_ready
echo ------------------------------------------------------------
echo   Done. Nothing needs fixing.
echo ------------------------------------------------------------
goto done

:not_ready
echo ------------------------------------------------------------
echo   Something above says FAIL. Please send me the file
echo   check-log.txt from this folder.
echo ------------------------------------------------------------
goto done

:no_rust
echo ------------------------------------------------------------
echo   Rust is not installed on this computer yet.
echo ------------------------------------------------------------
echo.
echo Please double-click BUILD EXPRESS.bat first. It explains how to
echo install Rust, which is free.
goto done

:failed_build
echo.
echo ------------------------------------------------------------
echo   The program did NOT build, so the check could not run.
echo ------------------------------------------------------------
echo.
echo Please send me the file check-build-log.txt from this folder.
echo The last lines are shown below.
echo.
powershell -NoProfile -Command "if (Test-Path -LiteralPath 'check-build-log.txt') { Get-Content -LiteralPath 'check-build-log.txt' -Tail 25 }"

:done
echo.
pause
endlocal
