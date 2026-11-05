@echo off
rem Milestone 2: builds LibRaw/libjpeg-turbo (vcpkg, into the project only) and runs the
rem LibRaw decode measurements. Double-click or run from any prompt. Log: D:\PhotoWorkflowData\analysis\m2-measure.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-measure.log
echo === %DATE% %TIME% === > "%LOG%"
echo [1/2] vcpkg install (first time: several minutes)...
"%VCPKG_ROOT%\vcpkg.exe" install --triplet x64-windows-static-md >> "%LOG%" 2>&1
if errorlevel 1 (echo vcpkg install FAILED - see %LOG% & pause & exit /b 1)
echo [2/2] build and measure...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0raw-decode-bench\run.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo measurement FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results under D:\PhotoWorkflowData\analysis\  (log: %LOG%)
pause
