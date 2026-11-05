@echo off
rem ADR 0006 step 3, second round: half-size images scaled to the screen, no unpacking ahead.
rem  1. engine tests (simulated decoder)
rem  2. release build + standalone folder (dist\PhotoWorkflow) with its DLLs
rem  3. start-up and idle memory: M1 reference vs this build
rem  4. built-in measurement runs, twice each: RAW rendering off (= M1 code paths),
rem     on (half-size scaled to the screen, the new default) and on with the full half size.
rem Do not use the computer while it runs (about 12 minutes). Results: D:\PhotoWorkflowData\analysis\step3b\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\step3b
set LOG=D:\PhotoWorkflowData\analysis\m2-step3b.log
set EXE=dist\PhotoWorkflow\photoworkflow.exe
set BENCH=powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0bench-app.ps1" -Exe %EXE%
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
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe %EXE% -Label m2 -Csv "%OUT%\startup.csv" -Runs 3 >> "%LOG%" 2>&1
echo [4/4] measurement runs...
for %%n in (1 2) do (
  echo   round %%n: off
  %BENCH% -Raw off -OutPrefix "%OUT%\bench-off-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement off-%%n FAILED - see %LOG% & pause & exit /b 1)
  echo   round %%n: on, scaled to the screen
  %BENCH% -Raw on -OutPrefix "%OUT%\bench-on-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement on-%%n FAILED - see %LOG% & pause & exit /b 1)
  echo   round %%n: on, full half size
  %BENCH% -Raw on -HalfMax 0 -OutPrefix "%OUT%\bench-full-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement full-%%n FAILED - see %LOG% & pause & exit /b 1)
)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
