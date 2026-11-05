@echo off
rem M2 step 2: several RAW regions after one shared unpack (baseline LibRaw, nothing installed).
rem Double-click. Log: D:\PhotoWorkflowData\analysis\m2-tiles.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\m2-tiles.log
echo === %DATE% %TIME% === > "%LOG%"
echo Building and measuring (about 5 minutes) - do not use the computer meanwhile...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0raw-decode-bench\run-tiles.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo measurement FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results under D:\PhotoWorkflowData\analysis\raw-tiles-*  (log: %LOG%)
pause
