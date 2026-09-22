@echo off
setlocal

echo ========================================
echo Building ChatCmd Web Frontend
echo ========================================

cd /d D:\frank\gemini\ChatCmd\web
if errorlevel 1 goto :FAILED

call npm.cmd run build
if errorlevel 1 goto :FAILED

echo.
echo ========================================
echo Building ChatCmd Release
echo ========================================

cd /d D:\frank\gemini\ChatCmd
if errorlevel 1 goto :FAILED

cargo build --release
if errorlevel 1 goto :FAILED

echo.
echo ========================================
echo BUILD SUCCESS
echo ========================================
goto :END

:FAILED
echo.
echo ========================================
echo BUILD FAILED
echo ========================================

:END
echo.
pause
endlocal