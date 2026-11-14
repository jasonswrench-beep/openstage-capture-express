@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - open
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express
echo ============================================================
echo.
echo This builds the program if it is not up to date (a few seconds
echo when nothing has changed) and opens its window. A full log is
echo saved in this folder as open-log.txt.
echo.

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Getting the program ready. Please wait...
if exist "open-log.txt" del "open-log.txt"
call "%~dp0cargo-run.bat" "open-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed

echo Opening the window...
start "" "%~dp0target\release\openstage-capture-express.exe"
goto finish

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
echo   The program did NOT build.
echo ------------------------------------------------------------
echo.
echo Nothing already on your computer was changed. Please send me the
echo file open-log.txt from this folder. The last lines are shown
echo below.
echo.
powershell -NoProfile -Command "if (Test-Path -LiteralPath 'open-log.txt') { Get-Content -LiteralPath 'open-log.txt' -Tail 25 }"

:done
echo.
pause

:finish
endlocal
