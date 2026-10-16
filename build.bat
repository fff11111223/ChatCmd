@echo off
setlocal

echo ========================================
echo Closing ChatCMD before build
echo ========================================

taskkill /F /IM ChatCMD.exe >nul 2>&1
taskkill /F /IM chat-cmd-client.exe >nul 2>&1
timeout /t 2 /nobreak >nul

tasklist /FI "IMAGENAME eq ChatCMD.exe" /NH | findstr /I /C:"ChatCMD.exe" >nul
if not errorlevel 1 goto :CLOSE_FAILED
tasklist /FI "IMAGENAME eq chat-cmd-client.exe" /NH | findstr /I /C:"chat-cmd-client.exe" >nul
if not errorlevel 1 goto :CLOSE_FAILED

echo ========================================
echo Building ChatCmd Web Frontend
echo ========================================

#cd /d D:\frank\gemini\ChatCmd\web
cd /d "%~dp0web"
if errorlevel 1 goto :FAILED

call npm.cmd run build
if errorlevel 1 goto :FAILED

echo.
echo ========================================
echo Building ChatCmd Release
echo ========================================

#cd /d D:\frank\gemini\ChatCmd
cd /d "%~dp0"
if errorlevel 1 goto :FAILED

cargo build --release
if errorlevel 1 goto :FAILED

echo.
echo ========================================
echo BUILD SUCCESS
echo ========================================
goto :END

:CLOSE_FAILED
echo.
echo ========================================
echo BUILD FAILED: ChatCMD is still running
echo ========================================
echo Close ChatCMD manually, then run this script again.
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
