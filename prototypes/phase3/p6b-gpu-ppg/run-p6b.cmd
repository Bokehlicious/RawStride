@echo off
rem ADR 0008 P6b: PPG demosaicing on the GPU in presenter B2-res. Builds the prototype and writes
rem results to D:\PhotoWorkflowData\analysis\phase3\p6b\. Installs nothing; the app is not touched.
rem Takes about 15 minutes. Do not use the computer meanwhile (the latency part presses keys).
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p6b.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase3\p6b\run-p6b.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase3\p6b
pause
