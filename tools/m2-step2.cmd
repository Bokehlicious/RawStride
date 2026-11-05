@echo off
rem ADR 0006 step 2: the RAW job engine (crates\pw-rawview), tested on its own.
rem  1. engine tests with a simulated decoder, 3 times (priorities, cancellation, concurrency, budget)
rem  2. engine with the real LibRaw decoder on the test RAWs (first tile, memory budget, freeing)
rem The app is not changed in this step. Double-click. Results: D:\PhotoWorkflowData\analysis\step2\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\step2
set LOG=D:\PhotoWorkflowData\analysis\m2-step2.log
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/2] simulated decoder, 3 runs...
for %%r in (1 2 3) do (
  cargo test --release -p pw-rawview > "%OUT%\simulated-run%%r.txt" 2>&1
  if errorlevel 1 (echo simulated run %%r FAILED - see %OUT%\simulated-run%%r.txt & pause & exit /b 1)
)
echo [2/2] real LibRaw decoder (takes about a minute; do not use the computer meanwhile)...
set PW_TESTDATA=D:\PhotoWorkflowData\RAW
cargo test --release -p pw-rawview --features libraw --test libraw_engine -- --nocapture --test-threads=1 > "%OUT%\libraw.txt" 2>&1
if errorlevel 1 (echo LibRaw run FAILED - see %OUT%\libraw.txt & pause & exit /b 1)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
