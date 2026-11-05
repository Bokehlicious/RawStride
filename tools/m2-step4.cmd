@echo off
rem ADR 0006 step 4: RAW tiles in the loupe. Builds, tests and measures.
rem  1. engine tests (simulated decoder)
rem  2. pixel identity on the test RAWs: tiles = the same area of a full render, also on a
rem     session reused after the half-size render (A7R IVA, A1, RW2)
rem  3. engine with the real LibRaw decoder: first tile with and without session reuse, memory
rem  4. release build + standalone folder (dist\PhotoWorkflow) with its DLLs
rem  5. start-up and idle memory: M1 reference vs this build
rem  6. built-in measurement runs, twice each: RAW rendering off (= M1 code paths), on, and on
rem     without session reuse. The app window opens and closes by itself.
rem Do not use the computer while it runs (about 20 minutes). Results: D:\PhotoWorkflowData\analysis\step4\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\step4
set LOG=D:\PhotoWorkflowData\analysis\m2-step4.log
set EXE=dist\PhotoWorkflow\photoworkflow.exe
set BENCH=powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0bench-app.ps1" -Exe %EXE%
set PW_TESTDATA=D:\PhotoWorkflowData\RAW
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/6] engine tests...
cargo test --release -p pw-rawview > "%OUT%\engine-tests.txt" 2>&1
if errorlevel 1 (echo engine tests FAILED - see %OUT%\engine-tests.txt & pause & exit /b 1)
echo [2/6] pixel identity on the test RAWs...
cargo test --release -p pw-libraw > "%OUT%\pixel-identity.txt" 2>&1
if errorlevel 1 (echo pixel identity FAILED - see %OUT%\pixel-identity.txt & pause & exit /b 1)
echo [3/6] engine with LibRaw (about a minute)...
cargo test --release -p pw-rawview --features libraw --test libraw_engine -- --nocapture --test-threads=1 > "%OUT%\libraw-engine.txt" 2>&1
if errorlevel 1 (echo LibRaw engine run FAILED - see %OUT%\libraw-engine.txt & pause & exit /b 1)
echo [4/6] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [5/6] start-up and idle memory, M1 reference vs M2...
if exist "%OUT%\startup.csv" del "%OUT%\startup.csv"
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe D:\PhotoWorkflowData\builds\m1-reference\photoworkflow.exe -Label m1 -Csv "%OUT%\startup.csv" -Runs 3 >> "%LOG%" 2>&1
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0measure-app.ps1" -Exe %EXE% -Label m2 -Csv "%OUT%\startup.csv" -Runs 3 >> "%LOG%" 2>&1
echo [6/6] measurement runs...
for %%n in (1 2) do (
  echo   round %%n: off
  %BENCH% -Raw off -OutPrefix "%OUT%\bench-off-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement off-%%n FAILED - see %LOG% & pause & exit /b 1)
  echo   round %%n: on
  %BENCH% -Raw on -OutPrefix "%OUT%\bench-on-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement on-%%n FAILED - see %LOG% & pause & exit /b 1)
  echo   round %%n: on, no session reuse
  %BENCH% -Raw on -Reuse off -OutPrefix "%OUT%\bench-noreuse-%%n" >> "%LOG%" 2>&1
  if errorlevel 1 (echo measurement noreuse-%%n FAILED - see %LOG% & pause & exit /b 1)
)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
