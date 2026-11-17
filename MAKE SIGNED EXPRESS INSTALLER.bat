@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - make the signed installer
cd /d "%~dp0"

echo.
echo ============================================================
echo   OpenStage Capture Express: make the SIGNED installer
echo ============================================================
echo.
echo This does what MAKE EXPRESS INSTALLER.bat does, and signs the
echo program, the uninstaller and the setup file (each with a
echo timestamp). The setup file goes in its own folder inside dist,
echo named after the release in installer\release.txt, so nothing
echo from an earlier release is touched or replaced.
echo.

rem Check that signing can work BEFORE the program is built, so a missing sign-in costs seconds and not minutes.
where python >nul 2>nul
if errorlevel 1 goto no_python
python "%~dp0tools\sign_windows.py" --ready
if errorlevel 1 goto not_ready

set /p LABEL=<"%~dp0installer\release.txt"
set "EXPRESS_OUTDIR=%~dp0dist\%LABEL%"
echo.
echo Release %LABEL%. The setup file will be in dist\%LABEL%
echo.
call "%~dp0MAKE EXPRESS INSTALLER.bat" sign
goto done

:no_python
echo ------------------------------------------------------------
echo   Signing needs Python, and it was not found.
echo ------------------------------------------------------------
echo.
echo Python is free (https://www.python.org/downloads/). Install it,
echo then try again.
goto stop

:not_ready
echo.
echo ------------------------------------------------------------
echo   Signing cannot start yet, so nothing was built.
echo ------------------------------------------------------------
echo.
echo The lines above say what is missing. If the sign-in has run out,
echo open a Command Prompt, type  az login  and sign in, then
echo double-click this file again. docs\installer.md (Signing) has
echo the whole list.
goto stop

:stop
echo.
pause

:done
endlocal
