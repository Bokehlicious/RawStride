@echo off
rem ADR 0008 phase 1 prototypes: P0 (JPEG decoders) and P4b (demosaic methods vs AHD).
rem Builds prototypes\phase1 into target\prototypes and writes results to
rem D:\PhotoWorkflowData\analysis\phase1\. Installs nothing; the app is not touched.
rem Do not use the computer while it runs (about 30-45 minutes); timings are affected otherwise.
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-phase1.ps1"
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase1\run-phase1.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase1
pause
