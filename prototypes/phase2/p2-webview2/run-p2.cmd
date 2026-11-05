@echo off
rem ADR 0008 phase 2, P2: presenter B2 (WebView2 shared buffer) against B1. Builds the prototype and
rem writes results to D:\PhotoWorkflowData\analysis\phase2\p2\. Installs nothing; the app is not touched.
rem Takes about 10 minutes. Do not use the computer meanwhile: the latency runs press arrow keys
rem in the prototype window and read the screen.
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p2.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase2\p2\run-p2.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase2\p2
pause
