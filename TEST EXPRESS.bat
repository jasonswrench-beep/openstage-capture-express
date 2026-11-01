@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - test
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: run all the tests
echo ============================================================
echo.
echo This runs every test, then the rule checks (licences, file
echo headers, writing style and the script rules). The first time it
echo can take several minutes. You do not need to do anything while it
echo works. Full logs are saved in this folder as test-log.txt and
echo rules-log.txt.
echo.

if not exist "%~dp0..\openstage-author\capture\crates\openstage-capture-render\Cargo.toml" goto no_capture

call "%~dp0rust-setup.bat"
if errorlevel 1 goto no_rust

echo Found Rust:
cargo --version
ver
echo.
echo Running the tests. Please wait...
ver > test-log.txt
cargo --version >> test-log.txt
call "%~dp0cargo-run.bat" test-log.txt test --workspace
set TESTS_RESULT=%errorlevel%

echo.
echo Test summary (each line is one group of tests):
findstr /B /C:"test result" test-log.txt
echo.

if not "%TESTS_RESULT%"=="0" goto tests_failed

set "PY="
set "PYARG="
python --version >nul 2>nul
if not errorlevel 1 set "PY=python"
if defined PY goto have_python
py -3 --version >nul 2>nul
if errorlevel 1 goto no_python
set "PY=py"
set "PYARG=-3"

:have_python
echo Running the rule checks...
echo.
set "TRIES=0"

:rules_try
set /a TRIES+=1
%PY% %PYARG% -I "%~dp0tools\check_rules.py" > rules-log.txt 2>&1
set "RULES_RESULT=%errorlevel%"
if "%RULES_RESULT%"=="0" goto rules_done
if "%RULES_RESULT%"=="1" goto rules_done
if %TRIES% lss 3 goto rules_try

:rules_done
type rules-log.txt
echo.
if "%RULES_RESULT%"=="0" goto pass
if "%RULES_RESULT%"=="1" goto rules_failed
goto rules_crashed

:pass
echo ------------------------------------------------------------
echo   PASS. The tests and the rule checks all passed.
echo ------------------------------------------------------------
echo.
echo The test summary above says how many tests there are. If it says
echo 0 passed, there is no program logic to test yet.
goto done

:rules_failed
echo ------------------------------------------------------------
echo   FAIL. The tests passed but a rule check did not.
echo ------------------------------------------------------------
echo.
echo The lines above that start with FAIL say what to fix. The same
echo text is saved in rules-log.txt.
goto done

:rules_crashed
echo ------------------------------------------------------------
echo   The rule checker stopped unexpectedly three times.
echo ------------------------------------------------------------
echo.
echo Windows gave the error code %RULES_RESULT%. That is not a rule
echo failing: the checker did not get to finish. Run this script again.
echo If it keeps happening, please send me rules-log.txt.
goto done

:no_python
echo ------------------------------------------------------------
echo   The tests passed, but Python was not found.
echo ------------------------------------------------------------
echo.
echo The rule checks need Python 3 (free, from python.org). Install it,
echo then double-click this script again. Nothing was changed.
goto done

:no_rust
echo ------------------------------------------------------------
echo   Rust is not installed on this computer yet.
echo ------------------------------------------------------------
echo.
echo Please double-click BUILD EXPRESS.bat first. It explains how to
echo install Rust, which is free.
goto done

:tests_failed
echo ------------------------------------------------------------
echo   FAIL. At least one test did not pass.
echo ------------------------------------------------------------
echo.
echo That is useful news, not a disaster: it means a problem was found
echo before anyone relied on this. Nothing on your computer was harmed.
echo.
echo HINT: if the error says something was not found, or that a variant is
echo missing, this folder is probably a MIX of old and new files. Fix:
echo extract the ZIP into a NEW, empty folder and run the script from there.
echo.
echo Please send me the file test-log.txt from this folder. The last
echo lines are shown below.
echo.
powershell -NoProfile -Command "Get-Content -LiteralPath 'test-log.txt' -Tail 40"

:no_capture
echo ------------------------------------------------------------
echo   The OpenStage code was not found next to this folder.
echo ------------------------------------------------------------
echo.
echo Express builds on code from OpenStage Capture, which lives in a
echo folder called openstage-author. It must sit next to this folder.
echo Double-click GET OPENSTAGE CODE.bat to get it, then try again.
echo Nothing was changed.
goto done

:done
echo.
pause
endlocal
