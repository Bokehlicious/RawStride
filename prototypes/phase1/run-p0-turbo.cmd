@echo off
rem ADR 0008 phase 1: re-runs only P0 after the libjpeg-turbo link fix (libspng added).
rem Rebuilds prototypes\phase1 and writes D:\PhotoWorkflowData\analysis\phase1\p0-decode-turbo.csv.
rem P4b results are not touched. Takes about 2 minutes; do not use the computer meanwhile.
setlocal
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-phase1.ps1" -Only p0 -P0Csv p0-decode-turbo.csv
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase1\run-phase1.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase1\p0-decode-turbo.csv
pause
