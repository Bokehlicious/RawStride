@echo off
rem ADR 0008 phase 1, P4: scheduling without UI, with PPG. Builds prototypes\phase1 and writes
rem results to D:\PhotoWorkflowData\analysis\phase1\p4\. Installs nothing; the app is not touched.
rem Takes about an hour. Do not use the computer meanwhile; timings are affected otherwise.
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p4.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase1\p4\run-p4.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase1\p4
pause
