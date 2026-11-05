@echo off
rem Windows launcher for finish_chunk1.py. Double-click it, or run it in a terminal.
rem Extra options pass through, for example:  scripts\finish_chunk1.cmd --dry-run
cd /d "%~dp0.."
where py >nul 2>nul && (py -3 scripts\finish_chunk1.py %* & goto done)
where python >nul 2>nul && (python scripts\finish_chunk1.py %* & goto done)
echo Python is not installed. Install it with:  winget install Python.Python.3.12
echo Then open a new terminal and run this again.
:done
echo.
pause
