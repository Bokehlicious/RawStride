@echo off
rem ADR 0008 P6: GPU demosaicing in presenter B2. Builds the prototype and writes results to
rem D:\PhotoWorkflowData\analysis\phase3\p6\. Installs nothing; the app is not touched.
rem Takes about 10 minutes. Do not use the computer meanwhile (the latency part presses keys).
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p6.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase3\p6\run-p6.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase3\p6
pause
