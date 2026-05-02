@echo off
setlocal
cd /d "%~dp0"

call generator_exe.bat --force-bootstrap %*
exit /b %ERRORLEVEL%
