@echo off
rem M2 error handling, second round: no tolerance for missing RAW data (measured: complete files
rem lack 0 bytes). pw-libraw tests on the test RAWs, release build + standalone folder, then
rem verify-dist from the copied folder: RAW on (no complete file may be refused) and broken files.
rem Do not use the computer while it runs (about 12 minutes). Results: D:\PhotoWorkflowData\analysis\m2-errors2\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\m2-errors2
set LOG=D:\PhotoWorkflowData\analysis\m2-errors2.log
set PW_TESTDATA=D:\PhotoWorkflowData\RAW
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/3] LibRaw on the test RAWs (pixel identity, damaged files)...
cargo test --release -p pw-libraw -- --nocapture --test-threads=1 > "%OUT%\pw-libraw-tests.txt" 2>&1
if errorlevel 1 (echo pw-libraw tests FAILED - see %OUT%\pw-libraw-tests.txt & pause & exit /b 1)
echo [2/3] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [3/3] verification from the copied folder (RAW on, broken files)...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0verify-dist.ps1" -Out "%OUT%" -Runs on >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0verify-dist.ps1" -Out "%OUT%" -Runs broken >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
