@echo off
rem Crop alignment check (follow-up to m2-verify). Installs nothing.
rem Double-click. Log: D:\PhotoWorkflowData\analysis\m2-crops.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-crops.log
echo === %DATE% %TIME% === > "%LOG%"
echo Building and checking crops (about 2 minutes) - do not use the computer meanwhile...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0raw-decode-bench\run-crops.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo check FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results under D:\PhotoWorkflowData\analysis\raw-crops-*  (log: %LOG%)
pause
