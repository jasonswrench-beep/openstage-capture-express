@echo off
setlocal
chcp 65001 >nul
title OpenStage Capture Express - get the OpenStage code
cd /d "%~dp0"

echo.
echo ============================================================
echo   Get the OpenStage code that Express builds on
echo ============================================================
echo.
set "TARGET=%~dp0..\openstage-author"
if exist "%TARGET%\capture\crates\openstage-capture-render\Cargo.toml" goto already

git --version >nul 2>nul
if errorlevel 1 goto no_git

echo Express builds on code from OpenStage Capture. This script copies
echo it from GitHub into a folder called openstage-author, next to
echo this one:
echo.
echo   %TARGET%
echo.
echo It downloads about 70 megabytes. Nothing of yours is sent.
echo.
set "ANSWER="
set /p ANSWER=Type Y and press Enter to download it, or just press Enter to stop: 
if /i not "%ANSWER%"=="Y" goto stopped
echo.
echo Downloading. Please wait...
git clone --branch feature/capture-express https://github.com/jasonswrench-beep/openstage-author.git "%TARGET%"
if errorlevel 1 goto failed
echo.
echo ------------------------------------------------------------
echo   Done. The OpenStage code is in place.
echo ------------------------------------------------------------
echo.
echo Now double-click BUILD EXPRESS.bat.
goto done

:already
echo The OpenStage code is already in place next to this folder:
echo.
echo   %TARGET%
echo.
echo Nothing to do. Double-click BUILD EXPRESS.bat.
goto done

:stopped
echo.
echo Stopped. Nothing was downloaded.
goto done

:no_git
echo ------------------------------------------------------------
echo   Git is not installed on this computer.
echo ------------------------------------------------------------
echo.
echo Git is the free tool that copies the code. Get it from
echo https://git-scm.com, install it with the default choices, then
echo double-click this script again. Nothing was changed.
goto done

:failed
echo.
echo ------------------------------------------------------------
echo   The download did NOT work.
echo ------------------------------------------------------------
echo.
echo Nothing was changed except possibly an unfinished folder at
echo the place named above. Read what Git said above, fix it (a
echo sign-in or a network problem is the usual cause), and run
echo this script again.

:done
echo.
pause
endlocal
