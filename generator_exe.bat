@echo off
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0"

set "ROOT=%~dp0"
set "LOG_DIR=%ROOT%logs"
set "LOG_FILE=%LOG_DIR%\build-exe.log"
set "FORCE_BOOTSTRAP=0"
if /I "%~1"=="--force-bootstrap" (
    set "FORCE_BOOTSTRAP=1"
    shift
)
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
call :log "Preflight exit code: %PREFLIGHT_EXIT%"
if not "%PREFLIGHT_EXIT%"=="0" call :log_tool_lookup

set "NEED_BOOTSTRAP=0"
if not "%PREFLIGHT_EXIT%"=="0" set "NEED_BOOTSTRAP=1"
if "%FORCE_BOOTSTRAP%"=="1" (
    call :log "Bootstrap was forced by --force-bootstrap"
    set "NEED_BOOTSTRAP=1"
)

if "%NEED_BOOTSTRAP%"=="1" (
    call :log "Installing or checking build requirements with winget"
    where winget >nul 2>nul
    if errorlevel 1 (
        set "BOOTSTRAP_UNAVAILABLE=1"
        call :log "winget was not found. Cannot install Node.js/Rust automatically on this machine."
    ) else (
        call :run_python scripts\build_exe.py --bootstrap --yes
        if errorlevel 1 goto fail
        call :refresh_path
    )
)

if defined BOOTSTRAP_UNAVAILABLE goto fail

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
call :has_py3
if not errorlevel 1 exit /b 0

call :has_python
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
call :has_py3
if not errorlevel 1 exit /b 0

call :has_python
if not errorlevel 1 exit /b 0

call :log "Python was installed, but this shell cannot see it yet. Opening a refreshed launcher window."
start "Shinden EXE Generator" cmd /k "cd /d ""%ROOT%"" && set GENERATOR_EXE_RESTARTED=1 && ""%~f0"" %BUILD_ARGS%"
exit /b 10

:run_python
call :has_py3
if not errorlevel 1 (
    py -3 %*
    exit /b !ERRORLEVEL!
)

call :has_python
if not errorlevel 1 (
    python %*
    exit /b !ERRORLEVEL!
)

exit /b 1

:has_py3
where py >nul 2>nul
if errorlevel 1 exit /b 1
py -3 --version >nul 2>nul
exit /b %ERRORLEVEL%

:has_python
where python >nul 2>nul
if errorlevel 1 exit /b 1
python --version >nul 2>nul
exit /b %ERRORLEVEL%

:refresh_path
for /f "usebackq delims=" %%P in (`powershell -NoProfile -ExecutionPolicy Bypass -Command "$machine=[Environment]::GetEnvironmentVariable('Path','Machine'); $user=[Environment]::GetEnvironmentVariable('Path','User'); Write-Output ($machine + ';' + $user)" 2^>nul`) do set "PATH=%%P;%PATH%"
set "PATH=%PATH%;%ProgramFiles%\nodejs;%USERPROFILE%\.cargo\bin;%LOCALAPPDATA%\Microsoft\WindowsApps;%LOCALAPPDATA%\Programs\Python\Python312;%LOCALAPPDATA%\Programs\Python\Python312\Scripts"
exit /b 0

:log_tool_lookup
call :log "Launcher user: %USERNAME%; USERPROFILE=%USERPROFILE%"
call :log "PATHEXT=%PATHEXT%"
call :log_where python
call :log_where py
call :log_where node
call :log_where npm
call :log_where npm.cmd
call :log_where cargo
call :log_where rustc
call :log_where winget
exit /b 0

:log_where
where "%~1" >nul 2>nul
if errorlevel 1 (
    call :log "where %~1: not found"
    exit /b 0
)
for /f "delims=" %%P in ('where "%~1" 2^>nul') do call :log "where %~1: %%P"
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
