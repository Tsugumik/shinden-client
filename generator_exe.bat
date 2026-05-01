@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "ROOT=%~dp0"
set "LOG_DIR=%ROOT%logs"
set "LOG_FILE=%LOG_DIR%\build-exe.log"
set "BOOTSTRAP_STAMP=%LOG_DIR%\.generator-exe-bootstrap-ok"
set "BUILD_ARGS=%*"
if "%BUILD_ARGS%"=="" set "BUILD_ARGS=--clean"

if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

call :log "Starting generator_exe launcher"
call :refresh_path

call :ensure_python
if "%ERRORLEVEL%"=="10" exit /b 0
if errorlevel 1 goto fail

call :run_python scripts\build_exe.py --preflight
set "PREFLIGHT_EXIT=%ERRORLEVEL%"

set "NEED_BOOTSTRAP=0"
if not "%PREFLIGHT_EXIT%"=="0" set "NEED_BOOTSTRAP=1"
if not exist "%BOOTSTRAP_STAMP%" set "NEED_BOOTSTRAP=1"

if "%NEED_BOOTSTRAP%"=="1" (
    call :log "Installing or checking build requirements with winget"
    where winget >nul 2>nul
    if errorlevel 1 (
        call :log "winget was not found. Skipping automatic system bootstrap."
    ) else (
        call :run_python scripts\build_exe.py --bootstrap --yes
        if errorlevel 1 goto fail
        type nul > "%BOOTSTRAP_STAMP%"
        call :refresh_path
    )
)

call :run_python scripts\build_exe.py --preflight
if errorlevel 1 (
    if not defined GENERATOR_EXE_RESTARTED (
        call :log "Build tools are still not visible in PATH. Opening a refreshed launcher window."
        set "GENERATOR_EXE_RESTARTED=1"
        start "Shinden EXE Generator" cmd /k "cd /d ""%ROOT%"" && set GENERATOR_EXE_RESTARTED=1 && ""%~f0"" %BUILD_ARGS%"
        exit /b 0
    )
    goto fail
)

call :log "Generating EXE"
call :run_python scripts\build_exe.py %BUILD_ARGS%
if errorlevel 1 goto fail

call :log "EXE generation finished"
if exist "%ROOT%dist-exe" start "" "%ROOT%dist-exe"
goto done

:ensure_python
where python >nul 2>nul
if not errorlevel 1 exit /b 0

where py >nul 2>nul
if not errorlevel 1 exit /b 0

where winget >nul 2>nul
if errorlevel 1 (
    call :log "Python and winget were not found. Install Python 3 manually, then run generator_exe.bat again."
    exit /b 1
)

call :log "Python was not found. Installing Python 3.12 with winget."
winget install --id Python.Python.3.12 -e --accept-source-agreements --accept-package-agreements
if errorlevel 1 exit /b %ERRORLEVEL%

call :refresh_path
where python >nul 2>nul
if not errorlevel 1 exit /b 0

where py >nul 2>nul
if not errorlevel 1 exit /b 0

call :log "Python was installed, but this shell cannot see it yet. Opening a refreshed launcher window."
start "Shinden EXE Generator" cmd /k "cd /d ""%ROOT%"" && set GENERATOR_EXE_RESTARTED=1 && ""%~f0"" %BUILD_ARGS%"
exit /b 10

:run_python
where python >nul 2>nul
if not errorlevel 1 (
    python %*
    exit /b %ERRORLEVEL%
)

where py >nul 2>nul
if not errorlevel 1 (
    py -3 %*
    exit /b %ERRORLEVEL%
)

exit /b 1

:refresh_path
for /f "usebackq delims=" %%P in (`powershell -NoProfile -ExecutionPolicy Bypass -Command "$machine=[Environment]::GetEnvironmentVariable('Path','Machine'); $user=[Environment]::GetEnvironmentVariable('Path','User'); Write-Output ($machine + ';' + $user)" 2^>nul`) do set "PATH=%%P;%PATH%"
set "PATH=%PATH%;%ProgramFiles%\nodejs;%USERPROFILE%\.cargo\bin;%LOCALAPPDATA%\Microsoft\WindowsApps;%LOCALAPPDATA%\Programs\Python\Python312;%LOCALAPPDATA%\Programs\Python\Python312\Scripts"
exit /b 0

:log
echo [%DATE% %TIME%] %~1
>> "%LOG_FILE%" echo [%DATE% %TIME%] %~1
exit /b 0

:fail
call :log "Generator failed. Opening build log."
if exist "%LOG_FILE%" start "" notepad "%LOG_FILE%"
exit /b 1

:done
call :log "Done"
pause
exit /b 0
