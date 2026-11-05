@echo off
rem ADR 0008 phase 2, P3: presenter A (wgpu). Builds the prototype and writes results to
rem D:\PhotoWorkflowData\analysis\phase2\p3\. Installs nothing; the app is not touched.
rem Takes about 10 minutes. Do not use the computer meanwhile: the latency runs press arrow keys
rem in the prototype window and read the screen.
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p3.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase2\p3\run-p3.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase2\p3
pause
