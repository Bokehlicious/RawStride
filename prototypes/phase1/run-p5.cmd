@echo off
rem ADR 0008 phase 1, P5: memory budget (4 GB, with 2 and 3 GB for comparison). Builds
rem prototypes\phase1 and writes results to D:\PhotoWorkflowData\analysis\phase1\p5\. Installs nothing; the app is not touched.
rem Takes about an hour. Do not use the computer meanwhile; timings are affected otherwise.
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p5.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase1\p5\run-p5.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase1\p5
pause
