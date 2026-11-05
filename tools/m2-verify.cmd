@echo off
rem Pixel check: baseline vs OpenMP LibRaw, and tile overlap (seams). Installs nothing.
rem Double-click. Log: D:\PhotoWorkflowData\analysis\m2-verify.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-verify.log
echo === %DATE% %TIME% === > "%LOG%"
echo Building and verifying (about 3 minutes) - do not use the computer meanwhile...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0raw-decode-bench\run-verify.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results under D:\PhotoWorkflowData\analysis\raw-verify-*  (log: %LOG%)
pause
