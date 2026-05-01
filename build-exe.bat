@echo off
setlocal
cd /d "%~dp0"

where python >nul 2>nul
if errorlevel 1 (
    py -3 scripts\build_exe.py %*
) else (
    python scripts\build_exe.py %*
)

exit /b %errorlevel%
