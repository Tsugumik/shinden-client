@echo off
setlocal
cd /d "%~dp0"

where python >nul 2>nul
if errorlevel 1 (
    where py >nul 2>nul
    if errorlevel 1 (
        where winget >nul 2>nul
        if errorlevel 1 (
            echo Python and winget were not found. Install Python 3 manually, then run this file again.
            exit /b 1
        )

        echo Python was not found. Installing Python 3.12 with winget...
        winget install --id Python.Python.3.12 -e --accept-source-agreements --accept-package-agreements
        echo Reopen your terminal, then run bootstrap-exe.bat again.
        exit /b %errorlevel%
    )

    py -3 scripts\build_exe.py --bootstrap --yes %*
) else (
    python scripts\build_exe.py --bootstrap --yes %*
)

exit /b %errorlevel%
