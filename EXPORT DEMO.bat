@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - export demo
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: try every export
echo ============================================================
echo.
echo This makes a made-up guide (with made-up screenshots, so nothing
echo is recorded) and exports it in every format that is built: a web
echo page, Markdown, JSON, pictures (PNG and JPG), an animated GIF, a PDF,
echo a Word file and a PowerPoint file. Each run goes into a new folder,
echo so nothing is ever replaced. A full log is saved in this folder as
echo export-demo-log.txt.
echo.

if not exist "%~dp0..\openstage-author\capture\crates\openstage-capture-render\Cargo.toml" goto no_capture

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Building the program. Please wait...
if exist "export-demo-log.txt" del "export-demo-log.txt"
call "%~dp0cargo-run.bat" "export-demo-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed

echo.
set "OUT=%~dp0export-demo-output"
"%~dp0target\release\openstage-capture-express.exe" export-demo --out "%OUT%"
if errorlevel 1 goto failed

if defined EXPRESS_NO_OPEN goto opened
echo Opening the folder...
start "" "%OUT%"
:opened
echo.
echo ------------------------------------------------------------
echo   Done. Look in the folder named above.
echo ------------------------------------------------------------
echo.
echo The web page (the html folder) is the one to open first.
echo Tell me what you think of how it looks and reads.
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
echo   The demo did NOT work.
echo ------------------------------------------------------------
echo.
echo Nothing already on your computer was changed. Please send me the
echo file export-demo-log.txt from this folder. The last lines are
echo shown below.
echo.
powershell -NoProfile -Command "if (Test-Path -LiteralPath 'export-demo-log.txt') { Get-Content -LiteralPath 'export-demo-log.txt' -Tail 25 }"

:done
echo.
pause
endlocal
