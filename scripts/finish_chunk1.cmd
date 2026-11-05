@echo off
rem Double-click me. Finishes Chunk 1: checks CI, merges, adds licence files, turns on Pages.
rem Installs Git, GitHub CLI and Python with winget if they are missing, and logs you in to GitHub.
rem Options pass through, for example:  scripts\finish_chunk1.cmd --dry-run
setlocal
cd /d "%~dp0.."

where winget >nul 2>nul || (echo winget is missing. Update "App Installer" from the Microsoft Store, then run again. & goto done)

where git >nul 2>nul || (echo Installing Git... & winget install -e --id Git.Git --silent --accept-package-agreements --accept-source-agreements & set NEEDRESTART=1)
where gh  >nul 2>nul || (echo Installing GitHub CLI... & winget install -e --id GitHub.cli --silent --accept-package-agreements --accept-source-agreements & set NEEDRESTART=1)
where py >nul 2>nul || where python >nul 2>nul || (echo Installing Python... & winget install -e --id Python.Python.3.12 --silent --accept-package-agreements --accept-source-agreements & set NEEDRESTART=1)

if defined NEEDRESTART (
  echo.
  echo Tools were just installed. Close this window and double-click the file again so Windows can find them.
  goto done
)

gh auth status >nul 2>nul || (echo Log in to GitHub - follow the prompts. & gh auth login --web --git-protocol https)

where py >nul 2>nul && (py -3 scripts\finish_chunk1.py %* & goto done)
python scripts\finish_chunk1.py %*

:done
echo.
pause
