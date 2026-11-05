@echo off
rem M2 error handling: damaged and unreadable RAW files. Builds, tests and verifies the
rem standalone folder again (copy in D:\PhotoWorkflowData\dist-test, cleaned environment).
rem  1. engine tests (simulated decoder)
rem  2. pw-libraw on the test RAWs: pixel identity, complete files never "damaged", truncated
rem     A7R IVA / RW2 / A1 files refused, unreadable files (prints LibRaw's diagnostics)
rem  3. release build + standalone folder
rem  4. verify-dist: RAW on, RAW off, broken-file folder (incl. rapid switching)
rem Do not use the computer while it runs (about 15 minutes). Results: D:\PhotoWorkflowData\analysis\m2-errors\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\m2-errors
set LOG=D:\PhotoWorkflowData\analysis\m2-errors.log
set PW_TESTDATA=D:\PhotoWorkflowData\RAW
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/4] engine tests...
cargo test --release -p pw-rawview > "%OUT%\engine-tests.txt" 2>&1
if errorlevel 1 (echo engine tests FAILED - see %OUT%\engine-tests.txt & pause & exit /b 1)
echo [2/4] LibRaw on the test RAWs (pixel identity, damaged files)...
cargo test --release -p pw-libraw -- --nocapture --test-threads=1 > "%OUT%\pw-libraw-tests.txt" 2>&1
if errorlevel 1 (echo pw-libraw tests FAILED - see %OUT%\pw-libraw-tests.txt & pause & exit /b 1)
echo [3/4] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [4/4] verification from the copied folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0verify-dist.ps1" -Out "%OUT%" >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
