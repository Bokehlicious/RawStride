@echo off
rem ADR 0006 step 3: half-size RAW in the loupe. Builds, tests and measures M1 vs M2.
rem  1. engine tests (simulated decoder)
rem  2. release build + standalone folder (dist\PhotoWorkflow) with its DLLs
rem  3. start-up and idle memory: M1 reference vs this build
rem  4. built-in measurement run, RAW rendering off (= M1 code paths) and on, twice each,
rem     with CPU and memory sampling. The app window opens and closes by itself.
rem Do not use the computer while it runs (about 10 minutes). Results: D:\PhotoWorkflowData\analysis\step3\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\step3
set LOG=D:\PhotoWorkflowData\analysis\m2-step3.log
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/4] engine tests...
cargo test --release -p pw-rawview > "%OUT%\engine-tests.txt" 2>&1
if errorlevel 1 (echo engine tests FAILED - see %OUT%\engine-tests.txt & pause & exit /b 1)
echo [2/4] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [3/4] start-up and idle memory, M1 reference vs M2...
if exist "%OUT%\startup.csv" del "%OUT%\startup.csv"
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe D:\PhotoWorkflowData\builds\m1-reference\photoworkflow.exe -Label m1 -Csv "%OUT%\startup.csv" -Runs 3 >> "%LOG%" 2>&1
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe dist\PhotoWorkflow\photoworkflow.exe -Label m2 -Csv "%OUT%\startup.csv" -Runs 3 >> "%LOG%" 2>&1
echo [4/4] measurement runs (off, on, off, on)...
for %%r in (off-1 on-1 off-2 on-2) do (
  for /f "tokens=1 delims=-" %%m in ("%%r") do (
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0bench-app.ps1" -Exe dist\PhotoWorkflow\photoworkflow.exe -Raw %%m -OutPrefix "%OUT%\bench-%%r" >> "%LOG%" 2>&1
    if errorlevel 1 (echo measurement %%r FAILED - see %LOG% & pause & exit /b 1)
  )
)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
