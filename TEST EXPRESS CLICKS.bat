@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - click test
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: the click test
echo ============================================================
echo.
echo This opens a small test window and then clicks and types in it
echo by itself, for about thirty seconds, while Express captures what
echo happens. It checks the steps that come out: their titles, the
echo text read from a field, that nothing typed in a password field
echo is kept, and that Express's own windows stay out of the pictures.
echo.
echo PLEASE LEAVE THE MOUSE AND KEYBOARD ALONE once it starts. It waits
echo until you have been still for a few seconds. It also takes real
echo pictures of your screen, so close anything private first. The
echo pictures stay in the folder clicks-test-output on this computer.
echo A full log is saved in this folder as clicks-test-log.txt.
echo.

if not exist "%~dp0..\openstage-author\capture\crates\openstage-capture-render\Cargo.toml" goto no_capture

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Getting the program ready. Please wait...
if exist "clicks-build-log.txt" del "clicks-build-log.txt"
call "%~dp0cargo-run.bat" "clicks-build-log.txt" build --release -p openstage-express-app
if errorlevel 1 goto failed_build

echo.
echo Starting the test. Hands off the mouse and keyboard now...
echo.
"%~dp0target\release\openstage-capture-express.exe" test-clicks --out "%~dp0clicks-test-output" --wait 120 > "%~dp0clicks-test-log.txt" 2>&1
set "RESULT=%ERRORLEVEL%"
type "%~dp0clicks-test-log.txt"
echo.
if not "%RESULT%"=="0" goto failed
echo ------------------------------------------------------------
echo   PASS. Every check passed.
echo ------------------------------------------------------------
echo.
echo The guide the test captured is in the folder clicks-test-output.
echo Open it to see the pictures if you like.
goto done

:failed
echo ------------------------------------------------------------
echo   Something above says FAIL. Please send me the file
echo   clicks-test-log.txt from this folder.
echo ------------------------------------------------------------
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

:failed_build
echo.
echo ------------------------------------------------------------
echo   The program did NOT build, so the test could not run.
echo ------------------------------------------------------------
echo.
echo Please send me the file clicks-build-log.txt from this folder.
echo The last lines are shown below.
echo.
powershell -NoProfile -Command "if (Test-Path -LiteralPath 'clicks-build-log.txt') { Get-Content -LiteralPath 'clicks-build-log.txt' -Tail 25 }"

:done
echo.
pause
endlocal
