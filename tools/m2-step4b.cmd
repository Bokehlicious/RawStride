@echo off
rem ADR 0006 step 4, extra measurement: zoom level where tiles start (most of the image visible).
rem Rebuilds the standalone folder and runs the built-in measurement twice with RAW rendering on.
rem Do not use the computer while it runs (about 8 minutes). Results: D:\PhotoWorkflowData\analysis\step4b\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\step4b
set LOG=D:\PhotoWorkflowData\analysis\m2-step4b.log
set EXE=dist\PhotoWorkflow\photoworkflow.exe
set BENCH=powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0bench-app.ps1" -Exe %EXE%
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/2] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
echo [2/2] measurement runs...
for %%n in (1 2) do (
  %BENCH% -Raw on -OutPrefix "%OUT%\bench-on-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement on-%%n FAILED - see %LOG% & pause & exit /b 1)
)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
