@echo off
rem ONE file, double-click it. Works even if you have no Prism folder yet.
rem It installs what is missing, logs you in to GitHub, downloads the Prism repo to
rem Documents\prism, then finishes Chunk 1 (merge, licence files, web page, first release).
setlocal
where winget >nul 2>nul || (echo winget is missing. Update "App Installer" from the Microsoft Store, then run again. & goto done)

where git >nul 2>nul || (echo Installing Git... & winget install -e --id Git.Git --silent --accept-package-agreements --accept-source-agreements & set NEEDRESTART=1)
where gh  >nul 2>nul || (echo Installing GitHub CLI... & winget install -e --id GitHub.cli --silent --accept-package-agreements --accept-source-agreements & set NEEDRESTART=1)
where py >nul 2>nul || where python >nul 2>nul || (echo Installing Python... & winget install -e --id Python.Python.3.12 --silent --accept-package-agreements --accept-source-agreements & set NEEDRESTART=1)
if defined NEEDRESTART (
  echo.
  echo Tools were just installed. Close this window and double-click this file again.
  goto done
)

gh auth status >nul 2>nul || (echo Log in to GitHub - follow the prompts in your browser. & gh auth login --web --git-protocol https)
gh auth status >nul 2>nul || (echo Login did not work. Run this file again. & goto done)

set "DEST=%USERPROFILE%\Documents\prism"
if exist "%DEST%\.git" (
  echo Using the existing folder %DEST%
  git -C "%DEST%" pull --ff-only origin main
) else (
  echo Downloading Prism to %DEST%
  gh repo clone thriple/prism "%DEST%"
)
if not exist "%DEST%\scripts\finish_chunk1.cmd" (echo Could not find the script in %DEST%. & goto done)

call "%DEST%\scripts\finish_chunk1.cmd" --tag v0.0.1 --open
exit /b

:done
echo.
pause
